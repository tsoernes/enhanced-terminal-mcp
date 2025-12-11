# Timeout Investigation Guide

## Overview

This guide helps diagnose and fix "context server request timed out" errors in the enhanced-terminal MCP server. These errors occur when Zed's 60-second timeout is reached before a command completes or switches to async mode.

## Quick Diagnosis Checklist

1. **Is the command actually running?**
   - Check `enhanced_terminal_job_list` to see active jobs
   - Look for the job_id in the error output

2. **Did it switch to async?**
   - Look for `switched_to_async: true` in the response
   - Check if async_threshold_secs was appropriate for the command

3. **Is the async threshold too high?**
   - Default is 50 seconds (safe margin before Zed's 60s timeout)
   - If custom value > 55s, it may timeout before switching

4. **Is force_sync enabled?**
   - `force_sync: true` disables async switching entirely
   - This will timeout for any command taking >60 seconds

## Common Causes

### 1. Blocking I/O Without Output

**Symptom:** Command produces no output for extended periods

**Example:**
```bash
# Database migration with long silent period
pg_restore --verbose large_backup.sql
```

**Why it happens:**
- PTY reader blocks waiting for output
- Elapsed time check can't run until read returns
- Monitor thread should catch this (post-fix)

**Solution:**
- Ensure monitoring thread is working (check logs)
- Lower async_threshold_secs for long-running commands
- Use `--verbose` flags to generate periodic output

### 2. Async Threshold Too Close to Timeout

**Symptom:** Command switches to async but Zed timeout already triggered

**Example:**
```bash
enhanced_terminal(
    command="long_operation",
    async_threshold_secs=58  # Only 2s margin!
)
```

**Solution:**
- Keep async_threshold_secs at least 10 seconds below Zed timeout
- Recommended: 50s or lower
- For very long commands: set to 30s or lower

### 3. Force Sync Enabled

**Symptom:** All long commands timeout

**Check:**
```rust
// In error response, look for:
force_sync: true
```

**Solution:**
- Remove `force_sync: true` parameter
- Only use force_sync for commands that MUST complete synchronously

### 4. Monitor Thread Not Running

**Symptom:** Commands hang at exactly async_threshold_secs with no switch

**Diagnostic:**
- Enable debug logging: `RUST_LOG=enhanced_terminal_mcp=debug`
- Look for: "Monitor thread: triggering async switch"
- If missing, monitor thread crashed or wasn't spawned

**Solution:**
- Check for panics in logs
- Verify threading is supported on platform
- File a bug report with logs

## Investigation Steps

### Step 1: Enable Debug Logging

Add to Zed's `settings.json`:

```json
{
  "context_servers": {
    "enhanced-terminal-mcp": {
      "command": "/path/to/enhanced-terminal-mcp",
      "env": {
        "RUST_LOG": "enhanced_terminal_mcp=debug"
      }
    }
  }
}
```

Restart Zed to apply changes.

### Step 2: Check Logs

```bash
# View recent Zed logs
tail -f ~/.local/share/zed/logs/Zed.log | grep -i "enhanced\|terminal\|timeout"

# Or search for specific patterns
rg "Monitor thread|switched to async|timed out" ~/.local/share/zed/logs/Zed.log
```

Look for:
- `execute_command called:` - Command started
- `Monitor thread: triggering async switch` - Async switch initiated
- `Command switched to async mode:` - Async switch completed
- `Background thread started` - Background monitoring active
- `Background job completed:` - Job finished

### Step 3: Reproduce with Known Command

Test with a simple long-running command:

```bash
# Should switch to async after 5 seconds
enhanced_terminal(
    command="for i in {1..20}; do echo 'tick $i'; sleep 1; done",
    async_threshold_secs=5
)
```

Expected log sequence:
```
execute_command called: command=for i in..., async_threshold_secs=5
Monitor thread: triggering async switch at 5.XX for job_id=...
Main thread: async switch detected, elapsed=5.XX
Command switched to async mode: job_id=..., elapsed=5.XX
Background thread started for job_id=...
```

### Step 4: Check Job Status

```bash
# List all jobs
enhanced_terminal_job_list()

# Check specific job
enhanced_terminal_job_status(job_id="job-123")
```

Look for:
- Status: Running, Completed, Failed, TimedOut
- Duration: How long has it been running?
- Exit code: Did it complete successfully?

### Step 5: Test Monitor Thread Health

Create a test that should definitely trigger async:

```bash
# Monitor thread should trigger after 3 seconds
enhanced_terminal(
    command="sleep 100",  # Long silent command
    async_threshold_secs=3
)
```

If this doesn't switch to async within 4-5 seconds, monitor thread is broken.

## Debugging Code Path

### Normal Flow (No Timeout)

1. `execute_command()` called
2. Command spawned in PTY
3. Monitor thread spawned
4. Main thread reads output in loop
5. Monitor thread signals async switch after threshold
6. Main thread detects flag, breaks loop
7. Background thread spawned
8. Function returns with `switched_to_async: true`

### Timeout Flow (Bug)

1. `execute_command()` called
2. Command spawned in PTY
3. Monitor thread spawned
4. Main thread reads output in loop
5. `reader.read()` **blocks** (no output)
6. Monitor thread signals async switch (flag set)
7. Main thread **still blocked** in read
8. 60 seconds elapse
9. Zed timeout triggers
10. Error: "context server request timed out"

**Fix:** Monitor thread should work even if main thread is blocked. If this happens, it's a regression.

## Log Patterns to Look For

### Successful Async Switch

```
DEBUG execute_command called: command=..., async_threshold_secs=5
DEBUG Monitor thread: triggering async switch at 5.02s for job_id=job-123
DEBUG Main thread: async switch detected, elapsed=5.02s, job_id=job-123
INFO  Command switched to async mode: job_id=job-123, elapsed=5.02s
DEBUG Background thread started for job_id=job-123
```

### Timeout Before Switch

```
DEBUG execute_command called: command=..., async_threshold_secs=58
# ... 60 seconds pass ...
# No "Monitor thread" log appears
# Zed timeout error
```

**Problem:** Monitor thread didn't trigger or couldn't set flag

### Synchronous Completion

```
DEBUG execute_command called: command=..., async_threshold_secs=50
DEBUG EOF reached, command completed: job_id=job-123
DEBUG Synchronous command completed: job_id=job-123, exit_code=0, duration=2.34s
```

**Normal:** Command finished before threshold

## Performance Monitoring

### Monitor Thread Overhead

Expected:
- CPU: <0.1% (sleeps 100ms between checks)
- Memory: ~8KB per thread
- Latency: Max 100ms delay in async switch detection

Actual:
```bash
# Monitor thread activity
rg "Monitor thread" ~/.local/share/zed/logs/Zed.log | wc -l

# Count async switches
rg "switched to async mode" ~/.local/share/zed/logs/Zed.log | wc -l
```

### Job Manager Health

```bash
# List all jobs
enhanced_terminal_job_list()

# Look for:
# - Stuck jobs (running for days)
# - Failed jobs with errors
# - Abnormal exit codes
```

## Testing Scenarios

### Scenario 1: No Output Command

```bash
enhanced_terminal(
    command="sleep 60",
    async_threshold_secs=10
)
```

**Expected:** Switches to async after 10 seconds, completes after 60 seconds

### Scenario 2: Periodic Output

```bash
enhanced_terminal(
    command="for i in {1..30}; do echo $i; sleep 2; done",
    async_threshold_secs=15
)
```

**Expected:** Switches to async after 15 seconds with partial output

### Scenario 3: Instant Completion

```bash
enhanced_terminal(
    command="echo 'quick'",
    async_threshold_secs=50
)
```

**Expected:** Completes synchronously in <1 second

### Scenario 4: Force Sync Short Command

```bash
enhanced_terminal(
    command="sleep 5",
    force_sync=true
)
```

**Expected:** Completes synchronously after 5 seconds

### Scenario 5: Force Sync Long Command (Should Timeout)

```bash
enhanced_terminal(
    command="sleep 70",
    force_sync=true
)
```

**Expected:** Timeout error after 60 seconds (by design)

## Resolution Paths

### Issue: Commands timing out despite low async_threshold_secs

**Solution:**
1. Verify monitor thread is spawning (check logs)
2. Check for Arc/AtomicBool issues in code
3. Ensure `should_switch_to_async.load()` is in main loop
4. Verify no deadlocks or panics

### Issue: Commands switching too early

**Solution:**
1. Increase `async_threshold_secs` parameter
2. Default is 50s, can increase to 55s max

### Issue: Background jobs not completing

**Solution:**
1. Check job status with `enhanced_terminal_job_status`
2. Look for zombie processes: `ps aux | grep <command>`
3. Check stderr for job-specific errors
4. Verify background thread spawned (logs)

### Issue: No logs appearing

**Solution:**
1. Verify `RUST_LOG` environment variable is set
2. Check Zed settings.json syntax
3. Restart Zed completely
4. Check if tracing-subscriber is initialized (code)

## Regression Testing

After any changes to async switching logic, test:

```bash
# Test suite
./test_async_switching.sh

# Or manually:
enhanced_terminal(command="sleep 70", async_threshold_secs=10)  # Should switch
enhanced_terminal(command="echo fast", async_threshold_secs=50)  # Should complete
enhanced_terminal(command="while true; do sleep 1; done", async_threshold_secs=5, timeout_secs=10)  # Should timeout
```

## Contact

If timeout issues persist after following this guide:

1. Collect logs with `RUST_LOG=enhanced_terminal_mcp=debug`
2. Document reproduction steps
3. Note Zed version, OS, and enhanced-terminal-mcp version
4. File issue with logs and reproduction case