# Timeout Parameter Refactoring Summary

## Overview

This refactoring removes the `timeout_secs` parameter from the `enhanced_terminal` tool API and replaces it with an environment variable `ENHANCED_TERMINAL_TIMEOUT_SECS`. This simplifies the API and makes timeout configuration consistent with other configurable behaviors like `ENHANCED_TERMINAL_ASYNC_THRESHOLD_SECS`.

## Changes Made

### Code Changes

#### `src/tools/terminal_executor.rs`

1. **Removed parameter from struct**:
   - Removed `timeout_secs: Option<u64>` field from `TerminalExecutionInput` struct
   - Removed associated serde default and documentation

2. **Added environment variable reader**:
   ```rust
   fn get_timeout_secs() -> Option<u64> {
       std::env::var("ENHANCED_TERMINAL_TIMEOUT_SECS")
           .ok()
           .and_then(|s| s.parse().ok())
   }
   ```

3. **Updated timeout handling**:
   - Replaced parameter-based timeout logic with environment variable
   - Changed from: `let timeout = match input.timeout_secs { ... }`
   - Changed to: `let timeout = get_timeout_secs().map(Duration::from_secs)`
   - Applied in both `execute_command` and `execute_command_inner` functions

#### `src/server.rs`

1. **Updated tool description**:
   - Removed `timeout_secs` from PARAMETERS section
   - Updated BEHAVIOR section to reference environment variable
   - Changed: "No timeout by default - commands run until completion unless timeout_secs is explicitly set"
   - To: "No timeout by default - commands run until completion (configurable via ENHANCED_TERMINAL_TIMEOUT_SECS environment variable)"
   - Updated SECURITY section to reference environment variable

2. **Updated server info**:
   - Changed info message from "No timeout by default (timeout_secs: 0 or None)"
   - To: "No timeout by default (ENHANCED_TERMINAL_TIMEOUT_SECS env var)"

#### `tests/mcp_client.rs`

- Removed `timeout_secs` parameter from all test cases:
  - `enhanced_terminal_echo` test
  - `sudo_prime_then_cached_sudo_n_opt_in` test (both calls)

### Documentation Changes

#### `README.md`

1. **Removed parameter example**:
   - Changed "With timeout and custom denylist:" section
   - To: "With custom denylist:" (removed timeout_secs from example)

2. **Updated configuration documentation**:
   - Performance section: "Set ENHANCED_TERMINAL_TIMEOUT_SECS environment variable to enable"
   - Default Values section: "set via ENHANCED_TERMINAL_TIMEOUT_SECS environment variable"

#### `docs/CHANGELOG.md`

- Updated "Timeout Behavior" change entry to reflect:
  - Timeout now configured via environment variable
  - Removed `timeout_secs` parameter from tool API
  - Set environment variable to desired timeout in seconds

#### `docs/FEATURES.md`

1. **Updated configuration example**:
   - Removed `timeout_secs` from Terminal Execution JSON schema
   - Added note: "Timeout is configured via the `ENHANCED_TERMINAL_TIMEOUT_SECS` environment variable (no timeout by default)"
   - Added missing parameters: `env_vars` and `tags`

2. **Removed from example**:
   - Smart async execution example no longer shows `timeout_secs`

#### `docs/COMMAND_TIMEOUT_DIAGNOSIS.md`

- Updated Solution 2 from parameter-based approach to environment variable:
  - Changed from JSON parameter example
  - To: `export ENHANCED_TERMINAL_TIMEOUT_SECS=300`
  - Updated explanation text accordingly

## Benefits

1. **Simplified API**: One less parameter for users to understand and configure per-call
2. **Consistency**: Matches the pattern of `ENHANCED_TERMINAL_ASYNC_THRESHOLD_SECS`
3. **Global Configuration**: Timeout applies to all commands from the same server instance
4. **Cleaner Tool Calls**: JSON payloads are simpler without timeout_secs
5. **Environment-based Config**: Aligns with common practices for operational settings

## Usage

### Before (Parameter-based)

```json
{
  "command": "docker run myimage",
  "timeout_secs": 600,
  "custom_denylist": ["docker rm"]
}
```

### After (Environment Variable)

Set the environment variable before starting the MCP server:

```bash
export ENHANCED_TERMINAL_TIMEOUT_SECS=600
```

Then use simplified tool calls:

```json
{
  "command": "docker run myimage",
  "custom_denylist": ["docker rm"]
}
```

## Backward Compatibility

This is a **breaking change** for any code that was explicitly setting `timeout_secs` in tool calls. However:

- The default behavior remains the same (no timeout)
- Users who were not setting timeout_secs are unaffected
- Migration is simple: move timeout configuration to environment variable
- Tests have been updated and pass successfully

## Environment Variables Summary

The enhanced terminal tool now supports three environment variables:

1. `ENHANCED_TERMINAL_ASYNC_THRESHOLD_SECS` (default: 50)
   - Controls when commands switch to background execution
   
2. `ENHANCED_TERMINAL_TIMEOUT_SECS` (default: none)
   - Controls maximum execution time before killing command
   
3. Standard environment variables can be set per-command via `env_vars` parameter

## Testing

All tests pass with the new implementation:
- Unit tests for denylist functionality
- Integration tests for MCP client communication
- Test cases updated to remove timeout_secs parameter
