# Streaming Output for Enhanced Terminal

## Overview

The `enhanced_terminal` tool now supports **streaming output** in synchronous mode. This feature allows clients to receive real-time updates as command output is generated, rather than waiting for the entire command to complete.

## How It Works

### Architecture

When a command is executed via `enhanced_terminal`:

1. **PTY Reader Thread**: A separate thread continuously reads output from the PTY
2. **Channel Communication**: Output chunks are sent via `mpsc::unbounded_channel` to the main thread
3. **Progress Notifications**: As each chunk arrives, the server sends `LoggingMessageNotification` to the client
4. **Incremental Updates**: The client receives output in real-time as it's produced

### Implementation Details

```rust
// The tool handler extracts the Peer from the context
async fn enhanced_terminal(
    &self,
    Parameters(input): Parameters<TerminalExecutionInput>,
    peer: Peer<RoleServer>,
) -> Result<CallToolResult, McpError>

// The peer is passed to execute_command
execute_command(&input, &self.job_manager, Some(peer))

// During execution, notifications are sent for each output chunk
peer.notify_logging_message(LoggingMessageNotificationParam {
    level: LoggingLevel::Info,
    logger: Some("enhanced_terminal".to_string()),
    data: serde_json::json!({
        "job_id": &job_id,
        "output": &output_str,
        "type": "stream"
    }),
})
```

## Notification Format

### LoggingMessageNotification

Each streaming update is sent as a `LoggingMessageNotification` with the following structure:

```json
{
  "method": "notifications/message",
  "params": {
    "level": "info",
    "logger": "enhanced_terminal",
    "data": {
      "job_id": "job-123",
      "output": "chunk of output text",
      "type": "stream"
    }
  }
}
```

### Fields

- **level**: Always `"info"` for streaming output
- **logger**: Always `"enhanced_terminal"` to identify the source
- **data.job_id**: The unique identifier for this command execution
- **data.output**: The actual output text chunk (UTF-8 decoded)
- **data.type**: Always `"stream"` to distinguish from other notifications

## Client Integration

### Receiving Streaming Output

Clients implementing the MCP `ClientHandler` trait can receive streaming notifications by implementing the `on_logging_message` method:

```rust
impl ClientHandler for MyClient {
    async fn on_logging_message(
        &self,
        notification: LoggingMessageNotificationParam,
        context: NotificationContext<RoleClient>,
    ) {
        if let Some(logger) = &notification.logger {
            if logger == "enhanced_terminal" {
                if let Some(data) = notification.data.as_object() {
                    if data.get("type").and_then(|v| v.as_str()) == Some("stream") {
                        let job_id = data.get("job_id").and_then(|v| v.as_str());
                        let output = data.get("output").and_then(|v| v.as_str());
                        // Handle streaming output here
                        println!("Stream update for {}: {}", job_id.unwrap(), output.unwrap());
                    }
                }
            }
        }
    }
}
```

### Example: Real-time Display

```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

struct StreamingClient {
    buffers: Arc<RwLock<HashMap<String, String>>>,
}

impl ClientHandler for StreamingClient {
    async fn on_logging_message(
        &self,
        notification: LoggingMessageNotificationParam,
        context: NotificationContext<RoleClient>,
    ) {
        if let Some(logger) = &notification.logger {
            if logger == "enhanced_terminal" {
                if let Some(data) = notification.data.as_object() {
                    if data.get("type").and_then(|v| v.as_str()) == Some("stream") {
                        let job_id = data.get("job_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let output = data.get("output")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        
                        // Append to buffer
                        let mut buffers = self.buffers.write().await;
                        buffers.entry(job_id.to_string())
                            .or_insert_with(String::new)
                            .push_str(output);
                        
                        // Update UI in real-time
                        self.update_display(job_id, &buffers[job_id]).await;
                    }
                }
            }
        }
    }
}
```

## Behavior

### Synchronous Mode

- **Streaming Enabled**: Yes, output is streamed as it arrives
- **Final Response**: Contains the complete output when command finishes
- **Use Case**: Commands that complete quickly (< async_threshold_secs)

### Asynchronous Mode (Background)

- **Streaming**: Occurs in background thread after switch
- **Job Status**: Use `enhanced_terminal_job_status` with `incremental=true` to get new output
- **Use Case**: Long-running commands (> async_threshold_secs)

### Output Characteristics

- **Chunk Size**: Variable, depends on PTY buffer (up to 4096 bytes)
- **Frequency**: As fast as output is produced by the command
- **Encoding**: UTF-8 with lossy conversion for invalid sequences
- **Buffering**: Minimal - output is streamed immediately

## Benefits

### For End Users

1. **Immediate Feedback**: See output as it's generated
2. **Better UX**: No "black box" waiting periods
3. **Progress Visibility**: Watch long operations progress in real-time
4. **Early Detection**: Spot errors or issues before command completes

### For Applications

1. **Responsive UI**: Update displays progressively
2. **User Engagement**: Keep users informed during execution
3. **Resource Efficiency**: No polling required
4. **Flexible Display**: Stream to logs, terminals, or custom UI components

## Performance Considerations

### Network Overhead

- Each notification adds ~100-200 bytes of JSON overhead
- For high-throughput commands, this is negligible compared to output size
- Benefits of real-time feedback outweigh minimal overhead

### Buffering Strategy

```
PTY → Reader Thread → Channel → Main Thread → MCP Notification → Client
  ↑                     ↑                        ↑
4KB buffer        Unbounded channel        ~100ms delivery
```

### Optimization Tips

1. **Client-Side Buffering**: Accumulate small chunks before UI update
2. **Throttling**: Limit UI refresh rate (e.g., max 10 updates/sec)
3. **Backpressure**: MCP transport handles flow control automatically

## Comparison with Traditional Approaches

### Before (Blocking)
```
Client → Request → Server → [Wait...] → Complete Response → Client
                              ↑
                         No visibility
```

### After (Streaming)
```
Client → Request → Server → [Notification 1] → Client
                         → [Notification 2] → Client
                         → [Notification N] → Client
                         → Complete Response → Client
                              ↑
                    Real-time visibility
```

## Limitations

1. **Sync Mode Only**: Streaming occurs during synchronous execution
   - After async switch, use `job_status` with `incremental=true`
2. **UTF-8 Only**: Binary output may have encoding issues
3. **No Backpressure Control**: Client must handle notification rate
4. **Order Guarantee**: Notifications are ordered but not guaranteed to be processed synchronously

## Future Enhancements

### Potential Improvements

- [ ] Add streaming for async mode via SSE or WebSocket transport
- [ ] Support binary output with base64 encoding
- [ ] Add client-side throttling configuration
- [ ] Implement notification batching for high-throughput scenarios
- [ ] Add progress percentage for commands with known duration
- [ ] Support cancellation signals from client

### API Stability

The streaming notification format is **stable** and follows MCP specification. Changes will be backward compatible.

## Examples

### Example 1: Watching a Build

```bash
# Command takes 10 seconds to complete
$ cargo build --release

# Client receives:
Stream 1: "   Compiling project v1.0.0"
Stream 2: "   Compiling dependency1 v2.3.4"
Stream 3: "   Compiling dependency2 v3.1.0"
...
Final: Complete output + exit code
```

### Example 2: Monitoring Logs

```bash
# Tailing logs for 5 seconds
$ timeout 5 tail -f /var/log/app.log

# Client sees:
Stream 1: "[2025-01-10 10:15:23] INFO: Server started"
Stream 2: "[2025-01-10 10:15:24] INFO: Connection from 192.168.1.10"
Stream 3: "[2025-01-10 10:15:25] WARN: High memory usage detected"
...
Final: Complete log output + timeout indicator
```

### Example 3: Long-running Script

```bash
# Script with progress indicators
$ ./deploy.sh

# Client receives real-time updates:
Stream 1: "Step 1/5: Preparing environment..."
Stream 2: "Step 2/5: Building Docker images..."
Stream 3: "Step 3/5: Pushing to registry..."
Stream 4: "Step 4/5: Deploying to production..."
Stream 5: "Step 5/5: Running health checks..."
Final: "Deployment complete!" + success status
```

## Debugging

### Enable Tracing

Set `RUST_LOG=enhanced_terminal_mcp=debug` to see streaming events:

```bash
export RUST_LOG=enhanced_terminal_mcp=debug
# Start MCP server
```

### Verify Streaming

Check logs for:
- `"Main task: Data received"` - Output chunk received
- `"Sending notification"` - Streaming notification sent
- `"Notification delivered"` - Client acknowledged receipt

### Common Issues

**Issue**: No streaming notifications received
- **Check**: Client implements `on_logging_message` handler
- **Check**: Notifications are not filtered by log level
- **Check**: Transport supports bidirectional communication

**Issue**: Duplicate or missing chunks
- **Check**: Client accumulates chunks correctly
- **Check**: No race conditions in client code
- **Check**: Network connectivity is stable

## Conclusion

Streaming output transforms the `enhanced_terminal` tool from a request-response pattern to a real-time streaming interface, providing immediate feedback and better user experience. The implementation leverages MCP's notification system for efficient, standardized communication between server and client.

For questions or issues, please refer to the main documentation or open an issue on GitHub.