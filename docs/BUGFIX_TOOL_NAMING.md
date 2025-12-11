# Bug Fix: Tool Naming Issue

## Problem

Some MCP clients were attempting to invoke tools with incorrect names:
- Incorrect: `enhanced-terminal_enhanced_terminal`
- Correct: `enhanced_terminal`

This was also happening for other tools:
- `enhanced-terminal_job_status` instead of `job_status`
- `enhanced-terminal_job_list` instead of `job_list`
- `enhanced-terminal_job_cancel` instead of `job_cancel`
- `enhanced-terminal_detect_binaries` instead of `detect_binaries`

## Root Cause

The `#[tool]` macro from the `rmcp` framework was deriving tool names by combining:
1. The struct name: `EnhancedTerminalServer`
2. The method name: `enhanced_terminal`

This resulted in tool names like `enhanced-terminal_enhanced_terminal`.

## Solution

Added explicit `name` parameters to all `#[tool]` attributes to override the automatic name derivation:

```rust
#[tool(
    name = "enhanced_terminal",  // Explicit name
    description = "Execute shell commands..."
)]
async fn enhanced_terminal(&self, ...) { ... }
```

## Changes Made

Modified `src/server.rs` to add explicit tool names:

1. `enhanced_terminal` - Main command execution tool
2. `job_status` - Get job status and output
3. `job_list` - List all background jobs
4. `job_cancel` - Cancel running jobs
5. `detect_binaries` - Detect developer tools

## Testing

After the fix:
- ✅ Tools are invoked with correct names
- ✅ No breaking changes to existing functionality
- ✅ Clean compilation with no warnings
- ✅ All tools remain accessible

## Impact

- **User Impact**: Clients that were experiencing tool invocation errors should now work correctly
- **Backwards Compatibility**: Maintained - tools that were working continue to work
- **API Stability**: Tool names are now explicitly controlled and won't change

## Lessons Learned

When using macro-based tool registration frameworks:
1. Always specify explicit tool names for clarity
2. Test tool invocation from multiple clients
3. Document tool naming conventions
4. Avoid relying on automatic name derivation

## Commit

```
fix: Add explicit tool names to prevent naming conflicts

- Add explicit name parameter to all #[tool] attributes
- Prevents tools from being named "enhanced-terminal_<method>"
- Ensures tools are invoked with correct names: enhanced_terminal, job_status, etc.
- No breaking changes - purely fixes tool invocation issues
```
