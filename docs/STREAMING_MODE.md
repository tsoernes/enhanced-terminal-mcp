# Streaming Mode Guide

## Overview

Streaming mode enables real-time output monitoring for long-running commands by immediately switching to background execution and allowing continuous polling for incremental updates.

## What is Streaming Mode?

Streaming mode is a special execution mode in the `enhanced_terminal` tool that:
- **Immediately** runs commands in background (bypasses async_threshold)
- Returns a **job_id** instantly
- Allows **real-time polling** with `enhanced_terminal_job_status`
- Returns **incremental output** (only new data since last check)
- Perfect for commands where you need **live feedback**

## How to Enable

Set `stream: true` in your `enhanced_terminal` call:

```json
{
  "command": "cargo build --release",
  "stream": true
}
```

## How It Works

### 1. Command Starts Immediately in Background

Unlike normal execution which waits up to 50 seconds before switching to async:
- Stream mode sets async_threshold to 1ms
- Command switches to background almost instantly
- Returns job_id immediately

### 2. Poll for Incremental Updates

Use `enhanced_terminal_job_status` with `incremental=true` (the default):

```json
{
  "job_id": "job-123",
  "incremental": true
}
```

Each call returns:
- Only **new output** since last check
- Current **status** (Running, Completed, Failed, etc.)
- **Exit code** when complete
- **Duration** information

### 3. Continue Until Complete

Keep polling until status is no longer "Running":
- Completed: exit_code = 0
- Failed: exit_code != 0
- TimedOut: timeout exceeded
- Canceled: manually terminated

## Usage Examples

### Example 1: Live Build Output

```javascript
// Start streaming build
const result = await enhanced_terminal({
  command: "cargo build --release",
  stream: true,
  tags: ["build", "release"]
});

// Poll for live updates
while (true) {
  const status = await enhanced_terminal_job_status({
    job_id: result.job_id,
    incremental: true
  });
  
  // Display new output
  if (status.output) {
    console.log(status.output);
  }
  
  // Check if complete
  if (status.status !== "Running") {
    console.log(`Build ${status.status}: exit code ${status.exit_code}`);
    break;
  }
  
  // Wait before next poll
  await sleep(500);
}
```

### Example 2: Test Execution with Live Results

```javascript
// Start streaming tests
const result = await enhanced_terminal({
  command: "npm test",
  stream: true,
  tags: ["test", "ci"]
});

let allOutput = "";

// Poll for test results
const pollInterval = setInterval(async () => {
  const status = await enhanced_terminal_job_status({
    job_id: result.job_id,
    incremental: true
  });
  
  if (status.output) {
    allOutput += status.output;
    console.log(status.output);
  }
  
  if (status.status !== "Running") {
    clearInterval(pollInterval);
    console.log(`Tests ${status.status}`);
    console.log(`Total output: ${allOutput.length} chars`);
  }
}, 1000);
```

### Example 3: Deployment with Progress Monitoring

```javascript
// Start streaming deployment
const result = await enhanced_terminal({
  command: "./deploy.sh production",
  stream: true,
  tags: ["deploy", "production"]
});

console.log("Deployment started...");

// Monitor deployment progress
while (true) {
  const status = await enhanced_terminal_job_status({
    job_id: result.job_id,
    incremental: true
  });
  
  // Parse and display progress
  if (status.output) {
    const lines = status.output.split('\n');
    lines.forEach(line => {
      if (line.includes('ERROR')) {
        console.error('❌', line);
      } else if (line.includes('SUCCESS')) {
        console.log('✅', line);
      } else {
        console.log('ℹ️', line);
      }
    });
  }
  
  if (status.status !== "Running") {
    if (status.exit_code === 0) {
      console.log('🎉 Deployment completed successfully!');
    } else {
      console.error('💥 Deployment failed!');
    }
    break;
  }
  
  await sleep(1000);
}
```

## Comparison with Other Modes

| Feature | Synchronous | Async | Streaming |
|---------|------------|-------|-----------|
| Initial response | Full output | Full output or job_id | job_id immediately |
| Async threshold | N/A | 50s (default) | 1ms (instant) |
| Live updates | No | No | Yes (via polling) |
| Best for | Quick commands | Background tasks | Live monitoring |
| Polling needed | No | Optional | Yes (for live output) |

## When to Use Streaming Mode

### ✅ Perfect For:

- **Compilation/builds** with live progress
- **Test suites** with real-time results  
- **Deployments** with status updates
- **Log monitoring** and tail-like behavior
- **Long-running scripts** with progress output
- **CI/CD pipelines** with step-by-step feedback
- Any command where you want **immediate live feedback**

### ❌ Not Ideal For:

- **Quick commands** (<5 seconds) - Use synchronous mode
- **Fire-and-forget** tasks - Use async mode
- **Commands you'll check later** - Use async mode
- **Silent background jobs** - Use async mode

## Polling Best Practices

### Poll Interval

Choose based on your needs:
- **High-frequency** (100-250ms): Real-time feel, more overhead
- **Medium** (500-1000ms): Good balance, recommended
- **Low-frequency** (2-5s): Batch updates, less overhead

### Example with Adaptive Polling

```javascript
// Start fast, slow down if nothing changes
let pollInterval = 500; // Start at 500ms
let noOutputCount = 0;

while (true) {
  const status = await enhanced_terminal_job_status({
    job_id: result.job_id,
    incremental: true
  });
  
  if (status.output) {
    console.log(status.output);
    noOutputCount = 0;
    pollInterval = 500; // Reset to fast polling
  } else {
    noOutputCount++;
    // Slow down if no output
    if (noOutputCount > 3) {
      pollInterval = Math.min(2000, pollInterval * 1.5);
    }
  }
  
  if (status.status !== "Running") break;
  
  await sleep(pollInterval);
}
```

## Combining with Other Features

### With Tags

```json
{
  "command": "npm run build",
  "stream": true,
  "tags": ["build", "frontend", "production"]
}
```

Then filter in job_list:
```json
{
  "tag_filter": "production",
  "status_filter": ["Running"]
}
```

### With Timeout

```json
{
  "command": "npm test",
  "stream": true,
  "timeout_secs": 600
}
```

### With Custom Environment

```json
{
  "command": "npm run deploy",
  "stream": true,
  "env_vars": {
    "NODE_ENV": "production",
    "API_KEY": "secret123"
  }
}
```

## Error Handling

### Handle Connection Issues

```javascript
async function streamWithRetry(jobId, maxRetries = 3) {
  let retries = 0;
  
  while (true) {
    try {
      const status = await enhanced_terminal_job_status({
        job_id: jobId,
        incremental: true
      });
      
      if (status.output) {
        console.log(status.output);
      }
      
      if (status.status !== "Running") {
        return status;
      }
      
      retries = 0; // Reset on success
      
    } catch (error) {
      retries++;
      console.error(`Poll failed (${retries}/${maxRetries}):`, error);
      
      if (retries >= maxRetries) {
        throw new Error("Max retries exceeded");
      }
    }
    
    await sleep(1000);
  }
}
```

### Handle Timeouts

```javascript
const result = await enhanced_terminal({
  command: "long-running-command",
  stream: true,
  timeout_secs: 300 // 5 minutes
});

// Monitor with overall timeout
const startTime = Date.now();
const maxDuration = 310000; // 5m 10s (buffer)

while (true) {
  if (Date.now() - startTime > maxDuration) {
    console.error("Overall timeout exceeded");
    // Try to cancel
    await enhanced_terminal_job_cancel({ job_id: result.job_id });
    break;
  }
  
  const status = await enhanced_terminal_job_status({
    job_id: result.job_id,
    incremental: true
  });
  
  if (status.output) console.log(status.output);
  if (status.status !== "Running") break;
  
  await sleep(1000);
}
```

## Advanced Patterns

### Multiple Streaming Commands

Monitor multiple commands simultaneously:

```javascript
async function monitorMultiple(commands) {
  // Start all commands
  const jobs = await Promise.all(
    commands.map(cmd => 
      enhanced_terminal({ command: cmd, stream: true })
    )
  );
  
  // Poll all jobs
  const promises = jobs.map(job => 
    streamOutput(job.job_id, job.command)
  );
  
  return await Promise.all(promises);
}

async function streamOutput(jobId, label) {
  while (true) {
    const status = await enhanced_terminal_job_status({
      job_id: jobId,
      incremental: true
    });
    
    if (status.output) {
      console.log(`[${label}] ${status.output}`);
    }
    
    if (status.status !== "Running") {
      return status;
    }
    
    await sleep(500);
  }
}
```

### Progress Parsing

Extract progress information from output:

```javascript
let lastProgress = 0;

while (true) {
  const status = await enhanced_terminal_job_status({
    job_id: result.job_id,
    incremental: true
  });
  
  if (status.output) {
    // Look for progress indicators
    const progressMatch = status.output.match(/(\d+)%/);
    if (progressMatch) {
      const progress = parseInt(progressMatch[1]);
      if (progress > lastProgress) {
        console.log(`Progress: ${progress}%`);
        lastProgress = progress;
      }
    }
  }
  
  if (status.status !== "Running") break;
  await sleep(500);
}
```

## Performance Considerations

### Memory Usage

- Incremental polling resets read position each time
- Old output is preserved in `full_output` for pagination
- Use pagination to access historical output if needed

### Network Overhead

- Each poll is a network request
- Balance polling frequency with network costs
- Consider adaptive polling for long-running commands

### Server Load

- Each poll queries the job manager
- Reasonable for dozens of concurrent streaming jobs
- Monitor if using hundreds of simultaneous streams

## Troubleshooting

### No Output Appearing

Check:
1. Is command actually producing output?
2. Is output buffered? (Add `stdbuf -o0` on Linux)
3. Poll interval too slow?
4. Command using stderr instead of stdout?

### Output Delayed

- Some programs buffer output
- Use unbuffered output flags if available
- Reduce poll interval for faster updates

### Command Not Starting

- Check for denied commands (security policy)
- Verify working directory exists
- Check shell availability

## Summary

Streaming mode provides:
- ✅ **Instant background execution**
- ✅ **Real-time output monitoring**
- ✅ **Incremental updates** (only new data)
- ✅ **Live feedback** for long operations
- ✅ **Full control** over polling frequency

Use it when you need live, real-time feedback from long-running commands!