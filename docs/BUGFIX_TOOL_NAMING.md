# Tool Naming: Evolution and Fix

## Problem (Historical)

Some MCP clients were attempting to invoke tools with incorrect names:
- Incorrect: `enhanced-terminal_enhanced_terminal`
- Correct: `enhanced_terminal`

This was also happening for other tools:
- `enhanced-terminal_job_status` (incorrectly auto-generated)
- `enhanced-terminal_job_list` (incorrectly auto-generated)
- `enhanced-terminal_job_cancel` (incorrectly auto-generated)
- `enhanced-terminal_detect_binaries` (incorrectly auto-generated)

## Current Tool Names

After fixes and renaming for better namespacing:
- `enhanced_terminal` - Main command execution tool
- `enhanced_terminal_job_status` - Get job status and output
- `enhanced_terminal_job_list` - List all background jobs
- `enhanced_terminal_job_cancel` - Cancel running jobs
- `detect_binaries` - Detect developer tools (no prefix needed)

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

Modified `src/server.rs` to add explicit tool names with proper namespacing:

1. `enhanced_terminal` - Main command execution tool
2. `enhanced_terminal_job_status` - Get job status and output
3. `enhanced_terminal_job_list` - List all background jobs
4. `enhanced_terminal_job_cancel` - Cancel running jobs
5. `detect_binaries` - Detect developer tools

The `enhanced_terminal_` prefix for job management tools:
- Provides clear namespacing
- Prevents conflicts with other MCP servers
- Makes it obvious which server provides these tools
- Groups related functionality together

## Testing

After the fixes and renaming:
- ✅ Tools are invoked with correct, explicit names
- ✅ Better namespacing prevents conflicts
- ✅ Clean compilation with no warnings
- ✅ All tools remain accessible with improved clarity

## Impact

- **User Impact**: Clear, namespaced tool names improve discoverability
- **Breaking Change**: Job management tools renamed from `job_*` to `enhanced_terminal_job_*`
  - Old names: `job_status`, `job_list`, `job_cancel`
  - New names: `enhanced_terminal_job_status`, `enhanced_terminal_job_list`, `enhanced_terminal_job_cancel`
- **API Stability**: Tool names are now explicitly controlled and won't change unexpectedly
- **Namespace Clarity**: Prefix makes it clear which server provides these tools

## Lessons Learned

When using macro-based tool registration frameworks:
1. Always specify explicit tool names for clarity
2. Test tool invocation from multiple clients
3. Document tool naming conventions
4. Avoid relying on automatic name derivation

## Commits

### Initial Fix
```
fix: Add explicit tool names to prevent naming conflicts

- Add explicit name parameter to all #[tool] attributes
- Prevents tools from being named "enhanced-terminal_<method>"
- Ensures tools are invoked with correct names
```

### Renaming for Better Namespacing
```
refactor: Rename job_* tools to enhanced_terminal_job_*

- Rename job_status to enhanced_terminal_job_status
- Rename job_list to enhanced_terminal_job_list
- Rename job_cancel to enhanced_terminal_job_cancel
- Provides clear namespacing and prevents conflicts
- Breaking change: clients must update tool invocations
```
