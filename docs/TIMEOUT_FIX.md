# Timeout Fix Documentation

## Problem

The Enhanced Terminal MCP server was experiencing "Context server request timeout" errors when commands ran for more than 60 seconds, despite having a smart async switching feature designed to prevent this issue.

### Root Cause

The timeout occurred because:

1. **Synchronous Blocking**: The `execute_command` function was synchronous (`fn` instead of `async fn`), which meant the MCP handler blocked waiting for it to return.

2. **I/O Dependent Threshold Checking**: The async threshold check (default: 50 seconds) only happened in the main loop when processing output from the PTY. If a command produced no output for more than 60 seconds, the loop would block on `reader.read()` and never check the elapsed time.

3. **Thread-based Implementation**: The original implementation used `std::thread` and `std::sync::mpsc`, which don't integrate well with async Rust and Tokio's runtime.

4. **MCP Client Timeout**: Zed's MCP client has a 60-second timeout for tool calls. When the synchronous `execute_command` didn't return within 60 seconds, the client would timeout before the function could detect the async threshold and return a job ID.

### Example Scenario

```
Time 0s:  Command starts, PTY spawned
Time 1-59s: Command produces no output, thread blocks on reader.read()
Time 60s: Zed MCP client times out ❌
Time 65s: Command finally produces output (too late!)
```

## Solution

The fix involved refactoring the code to use proper async/await patterns with Tokio:

### Key Changes

1. **Async Function Signature**: Changed `execute_command` from `fn` to `async fn`
   ```rust
   // Before
   pub fn execute_command(...)
   
   // After
   pub async fn execute_command(...)
   ```

2. **Tokio Channels**: Replaced `std::sync::mpsc` with `tokio::sync::mpsc::unbounded_channel`
   ```rust
   // Before
   let (tx, rx) = std::sync::mpsc::channel();
   
   // After
   let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
   ```

3. **Tokio Tasks**: Replaced `std::thread::spawn` with `tokio::task::spawn_blocking` for the reader and `tokio::spawn` for the background monitor
   ```rust
   // Before
   std::thread::spawn(move || { ... });
   
   // After (blocking I/O)
   tokio::task::spawn_blocking(move || { ... });
   
   // After (async work)
   tokio::spawn(async move { ... });
   ```

4. **Non-blocking Timeout Receive**: Changed from `try_recv()` to `tokio::time::timeout(duration, rx.recv()).await`
   ```rust
   // Before - blocking check
   match rx.try_recv() {
       Ok(data) => { ... }
       Err(TryRecvError::Empty) => {
           thread::sleep(Duration::from_millis(10));
           continue;
       }
   }
   
   // After - async with timeout
   match tokio::time::timeout(check_interval, rx.recv()).await {
       Ok(Some(data)) => { ... }
       Err(_) => continue, // Timeout, check elapsed time
   }
   ```

5. **Independent Time Checking**: The main loop now checks elapsed time independently every 100ms, regardless of whether data is received:
   ```rust
   let check_interval = Duration::from_millis(100);
   loop {
       let elapsed = start_time.elapsed();
       
       // Check async threshold BEFORE waiting for I/O
       if !input.force_sync && elapsed > async_threshold {
           switched_to_async = true;
           break;
       }
       
       // Try to receive with timeout
       match tokio::time::timeout(check_interval, rx.recv()).await {
           // ...
       }
   }
   ```

6. **Awaiting in Handler**: Updated the MCP server handler to await the async function
   ```rust
   // Before
   let result = execute_command(&input, &self.job_manager).map_err(...)?;
   
   // After
   let result = execute_command(&input, &self.job_manager).await.map_err(...)?;
   ```

### Flow After Fix

```
Time 0s:     Command starts, PTY spawned
Time 0-50s:  Main loop checks elapsed time every 100ms
Time 50s:    Async threshold reached, function returns job ID ✅
Time 50+:    Background task continues monitoring command
Client:      Receives job ID in <1 second, no timeout!
```

## Benefits

1. **Reliable Timeout Prevention**: The async threshold check happens every 100ms, independent of I/O operations
2. **Better Integration**: Proper async/await with Tokio runtime
3. **Non-blocking**: The MCP handler never blocks for more than 100ms
4. **Consistent Behavior**: Commands always switch to background mode at the async threshold (default 50s), regardless of output patterns
5. **No Context Server Timeouts**: Function always returns before Zed's 60-second timeout

## Testing

To verify the fix works:

```bash
# Test a command that produces no output for 55 seconds
enhanced_terminal --command "sleep 55 && echo done" --async_threshold_secs 50

# Expected: Returns job ID after ~50 seconds, command continues in background
```

## Configuration

Users can still configure the async threshold:

```json
{
  "command": "long-running-task",
  "async_threshold_secs": 30  // Switch to background after 30 seconds
}
```

## Related Files

- `src/tools/terminal_executor.rs`: Core execution logic with async implementation
- `src/server.rs`: MCP server handler that awaits the async function
- `src/tools/job_manager.rs`: Job tracking and status management
- `docs/DEBUGGING_TIMEOUT_ISSUE.md`: Historical debugging notes
- `docs/BUGFIX_TOOL_NAMING.md`: Previous bug fix documentation

## Conclusion

The fix transforms the Enhanced Terminal MCP server from a synchronous, thread-based implementation to a proper async implementation using Tokio. This ensures that the smart async switching feature works reliably, preventing context server timeouts while maintaining backward compatibility with all existing functionality.