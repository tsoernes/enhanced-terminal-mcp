# Changelog

All notable changes to the Enhanced Terminal MCP Server will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Timeout Issue**: Fixed "Context server request timeout" errors when commands ran longer than 60 seconds
  - Converted `execute_command` from synchronous to async function using Tokio primitives
  - Replaced `std::sync::mpsc` with `tokio::sync::mpsc::unbounded_channel`
  - Replaced `std::thread::spawn` with `tokio::spawn` and `tokio::task::spawn_blocking`
  - Made async threshold check independent of I/O operations (checks every 100ms)
  - Main loop no longer blocks on PTY reads - uses timeout-based receive
  - Ensures function returns job ID before Zed's 60-second MCP timeout
  - Async threshold (default 50s) now reliably triggers regardless of command output patterns
  - See `docs/TIMEOUT_FIX.md` for detailed technical explanation

### Changed
- **BREAKING: Tool Renaming**: Job management tools renamed for better namespacing
  - `job_status` → `enhanced_terminal_job_status`
  - `job_list` → `enhanced_terminal_job_list`
  - `job_cancel` → `enhanced_terminal_job_cancel`
  - Provides clear namespacing and prevents conflicts with other MCP servers
  - Main tool remains `enhanced_terminal` (unchanged)
  - `detect_binaries` remains unchanged (no prefix needed)

### Added
- **Streaming Output**: Real-time output notifications for `enhanced_terminal` in sync mode
  - Clients receive `LoggingMessageNotification` updates as output is generated
  - Provides immediate feedback during command execution
  - Uses MCP's standard notification mechanism
  - Backward compatible - clients that don't handle notifications still work
  - Zero polling required - push-based updates
  - Minimal overhead (~100-200 bytes JSON per chunk)
  - See `docs/STREAMING_OUTPUT.md` for comprehensive guide
  - See `docs/STREAMING_IMPLEMENTATION_SUMMARY.md` for technical details
- **Job Tags and Metadata**: Enhanced job tracking with rich metadata
  - Add custom tags to jobs via `tags` parameter in `enhanced_terminal`
  - Automatic command summary generation (first 100 chars)
  - Tags displayed in `enhanced_terminal_job_status` and `enhanced_terminal_job_list` outputs
  - Example: `{"command": "cargo build", "tags": ["build", "release"]}`
- **Job Filtering**: Advanced filtering in `enhanced_terminal_job_list` tool
  - Filter by status: `status_filter` (e.g., ["Running", "Completed"])
  - Filter by tag: `tag_filter` (e.g., "build")
  - Filter by working directory: `cwd_filter`
  - Sort order: `sort_order` ("newest" or "oldest")
  - All filters can be combined with AND logic
- **Output Pagination**: Seek into specific byte ranges of job output
  - New `offset` parameter: starting byte position (default: 0)
  - New `limit` parameter: maximum bytes to return (default: 0 = all)
  - Returns `has_more` flag indicating if more data available
  - Returns `total_length` for overall output size
  - Useful for very long logs without retrieving full output
  - Three modes: incremental (default), full, and paginated
- **Incremental Output**: `enhanced_terminal_job_status` now supports `incremental` parameter
  - Returns only new output since last check when `incremental: true`
  - First call returns all accumulated output
  - Subsequent calls return only new output
  - Read position tracked per job_id
  - Reset by calling with `incremental: false`
- **Environment Variable Support**: `env_vars` parameter in `enhanced_terminal`
  - Pass custom environment variables as key-value pairs
  - Example: `{"env_vars": {"NODE_ENV": "production", "DEBUG": "true"}}`
- **Enhanced Tool Documentation**: Comprehensive parameter descriptions exposed via MCP
  - Detailed descriptions for all parameters
  - Usage examples in tool descriptions
- **Explicit Tool Names**: All tools now have explicit names to prevent auto-generation issues
  - Prevents tools from being incorrectly named with double prefixes
  - Ensures consistent tool invocation across MCP clients
  - Behavior explanations for each tool
  - Clear return value documentation
- **Zed Integration**: Automatically added to Zed editor configuration
  - Server available as `enhanced-terminal` context server
  - Enabled by default in user settings

### Changed
- **Default Shell**: Changed from `sh` to `bash`
  - More feature-rich and commonly expected
  - Still fully configurable via `shell` parameter
- **Async Threshold**: Increased from 5 seconds to 50 seconds
  - Reduces unnecessary backgrounding for medium-duration commands
  - More predictable behavior for typical CLI operations
  - Still fully configurable via `async_threshold_secs`
- **Timeout Behavior**: Changed from 300 seconds default to no timeout
  - Timeout is now configured via `ENHANCED_TERMINAL_TIMEOUT_SECS` environment variable
  - No timeout by default (was: 300 seconds)
  - Set environment variable to desired timeout in seconds
  - Prevents unexpected command termination
  - Removed `timeout_secs` parameter from tool API
- **Binary Detection Concurrency**: Increased from 12 to 16 parallel checks
  - Faster tool detection across all categories
  - Better utilization of modern multi-core systems

### Improved
- **Tool Documentation**: All tools now have comprehensive descriptions
  - Parameter descriptions include types and defaults
  - Behavior sections explain what each tool does
  - Return value documentation lists all fields
  - Examples provided inline
- **Job Manager**: Added read position tracking for incremental output
  - `last_read_position` field in JobRecord
  - `get_incremental_output()` method for efficient polling
  - `reset_read_position()` for full output retrieval
- **Server Instructions**: Enhanced ServerInfo with detailed usage guide
  - Complete feature overview
  - Security information
  - Example usage patterns
  - Shell availability information

## [0.1.0] - 2024-12-10

### Added
- **Initial Release**: Enhanced Terminal MCP Server
- **Smart Async Switching**: Commands auto-switch to background after threshold
- **Job Management**: Full background job tracking and control
  - `job_status`: Get status and output of jobs
  - `job_list`: List all jobs with previews
  - `job_cancel`: Cancel running jobs (Unix only)
- **Security Denylist**: 40+ dangerous command patterns blocked
  - Destructive operations: `rm -rf /`, `mkfs`, `dd`
  - System manipulation: `shutdown`, `reboot`, `chmod 777 /`
  - Fork bombs and resource exhaustion
  - Custom patterns via `custom_denylist` parameter
- **Binary Detection**: Fast parallel scanning of developer tools
  - 16+ categories with 100+ tools
  - Concurrent version detection
  - Configurable timeouts and filters
- **Shell Detection**: Automatic shell discovery at startup
  - Integrated into server metadata
  - Version detection included
  - No separate tool call required
- **PTY Support**: Full terminal emulation
  - Proper terminal sizing
  - ANSI color code support
  - Signal handling
- **Output Management**: Intelligent handling of large outputs
  - Configurable limits (16KB default)
  - Incremental capture during execution
  - Truncation indicators
- **Modular Architecture**: Clean separation of concerns
  - `src/detection/` - Binary and shell detection
  - `src/tools/` - Terminal execution, job management, security
  - `src/server.rs` - MCP server implementation
  - `src/main.rs` - Minimal entry point

### Technical Details
- Rust 2024 edition
- Official rmcp SDK v0.8
- tokio async runtime
- portable-pty for terminal emulation
- nix for Unix signal handling (job cancellation)

### Security
- Default denylist with 40+ dangerous patterns
- Case-insensitive pattern matching
- Custom pattern support
- No privilege escalation by default
- Output size limits
- Timeout protection

### Performance
- 16 concurrent binary checks
- Thread-per-job background execution
- Lock-free job status reads
- Memory-efficient incremental output capture

### Documentation
- Comprehensive README
- Feature documentation (FEATURES.md)
- Project summary (SUMMARY.md)
- MIT License

## Links

- **Repository**: https://github.com/tsoernes/enhanced-terminal-mcp
- **Issues**: https://github.com/tsoernes/enhanced-terminal-mcp/issues
- **License**: MIT