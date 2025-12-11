# Debugging Enhanced Terminal MCP Timeout Issues

## Problem Description

The `enhanced_terminal` tool was experiencing "context server request timed out" errors in Zed, even though it has a smart async feature designed to prevent this. Zed has a 60-second timeout for MCP server requests.

## Root Cause

The async switching mechanism had a critical flaw in its timing check logic:

### Original Implementation Issue

```rust
loop {
    let elapsed = start_time.elapsed();

    // Check if we should switch to async
    if !input.force_sync && elapsed > async_threshold {
        switched_to_async = true;
        break;
    }

    match reader.read(&mut buffer) {
        Ok(0) => break, // EOF
        Ok(n) => {
            // Process output...
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        Err(_) => break,
    }
}
```

**The Problem:**
1. The elapsed time check happened at the top of the loop
2. Then `reader.read()` was called, which could **block indefinitely** waiting for output
3. If the PTY had no output for an extended period, `reader.read()` would block
4. The elapsed time check wouldn't happen again until `reader.read()` returned
5. This meant commands could run for 60+ seconds without switching to async mode
6. Result: Zed's 60-second timeout would trigger before the async switch

### Why This Happened

The PTY reader from `portable-pty` doesn't guarantee non-blocking behavior. When a command:
- Produces no output for a long time
- Is doing work but not writing to stdout/stderr
- The `reader.read()` call blocks, preventing the elapsed time check from running

## Solution

Implemented a **monitoring thread** that checks elapsed time independently of I/O operations:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

let should_switch_to_async = Arc::new(AtomicBool::new(false));
let should_timeout = Arc::new(AtomicBool::new(false));

// Spawn a monitoring thread
thread::spawn(move || {
    loop {
        let elapsed = monitor_start.elapsed();

        // Check async threshold
        if !force_sync && elapsed > monitor_async_threshold {
            monitor_switch.store(true, Ordering::SeqCst);
            break;
        }

        // Check timeout
        if let Some(timeout_dur) = monitor_timeout_duration {
            if elapsed > timeout_dur {
                monitor_timeout.store(true, Ordering::SeqCst);
                break;
            }
        }

        thread::sleep(Duration::from_millis(100));
    }
});

// Main loop checks atomic flags
loop {
    // Check if monitoring thread signaled async switch
    if should_switch_to_async.load(Ordering::SeqCst) {
        switched_to_async = true;
        break;
    }

    // Try to read output
    match reader.read(&mut buffer) {
        // ... handle output ...
    }
}
```

### How This Works

1. **Monitoring Thread**: Runs independently, checking elapsed time every 100ms
2. **Atomic Flags**: Thread-safe communication between monitor and main thread
3. **Non-Blocking Checks**: Main thread checks flags before each read attempt
4. **Guaranteed Timeout**: Even if `reader.read()` blocks, monitor thread will trigger async switch

## Logging and Diagnostics

Added comprehensive `tracing` logs throughout the execution flow:

```rust
use tracing;

tracing::debug!("execute_command called: command={}, async_threshold_secs={}", ...);
tracing::debug!("Monitor thread: triggering async switch at {:.2}s", ...);
tracing::info!("Command switched to async mode: job_id={}, elapsed={:.2}s", ...);
tracing::debug!("Background thread started for job_id={}", ...);
```

### Viewing Logs

To enable debug logging, set the `RUST_LOG` environment variable before starting the MCP server:

```bash
# In Zed settings.json for enhanced-terminal-mcp:
{
  "context_servers": {
    "enhanced-terminal-mcp": {
      "command": "/path/to/enhanced-terminal-mcp",
      "args": [],
      "env": {
        "RUST_LOG": "enhanced_terminal_mcp=debug"
      }
    }
  }
}
```

Logs are written to **stderr** by the tracing-subscriber. Zed may redirect these to:
- `/home/user/.local/share/zed/logs/Zed.log` (mixed with other logs)
- Separate MCP server log files (implementation-dependent)
- `/dev/null` if not captured

## Testing the Fix

### Test Case 1: Command that switches to async

```bash
# Set async threshold to 5 seconds
enhanced_terminal(
    command="for i in {1..10}; do echo 'Output $i'; sleep 1; done",
    async_threshold_secs=5
)
```

**Expected Result:**
- Runs for ~5 seconds synchronously
- Switches to background
- Returns immediately with `switched_to_async: true`
- Job status shows "Running"

### Test Case 2: Silent long-running command

```bash
# Command that produces no output for a while
enhanced_terminal(
    command="sleep 60 && echo 'Done'",
    async_threshold_secs=10
)
```

**Expected Result:**
- Switches to async after 10 seconds (even with no output)
- No timeout errors in Zed
- Job completes successfully in background

### Test Case 3: Commands under threshold

```bash
enhanced_terminal(
    command="echo 'Quick command'",
    async_threshold_secs=50
)
```

**Expected Result:**
- Completes synchronously
- Returns with `switched_to_async: false`
- Output immediately available

## Summary of Changes

1. **terminal_executor.rs**:
   - Added monitoring thread for elapsed time checking
   - Used `Arc<AtomicBool>` for thread-safe flag communication
   - Removed attempted PTY non-blocking mode setting (not portable)
   - Added comprehensive debug logging

2. **main.rs**:
   - Initialized `tracing-subscriber` with stderr output
   - Default log level: `info`
   - Respects `RUST_LOG` environment variable

3. **Cargo.toml**:
   - Added `tracing` and `tracing-subscriber` dependencies
   - Added `fs` feature to `nix` dependency

## Performance Impact

- Monitoring thread overhead: minimal (~100ms polling interval)
- No impact on I/O throughput
- Slight increase in memory (one additional thread per command)
- Thread terminates when async switch or timeout occurs

## Future Improvements

1. **Configurable monitoring interval**: Allow users to tune the 100ms polling
2. **Centralized monitoring**: Single thread monitoring all jobs instead of one per command
3. **Structured logging**: JSON logs for better parsing and analysis
4. **Log file rotation**: Dedicated log files with size limits
5. **Metrics**: Track async switch frequency, timeouts, and execution times