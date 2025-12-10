# Enhanced Terminal MCP Server - Project Summary

## Overview

This project provides a standalone Model Context Protocol (MCP) server that exposes terminal execution and developer tool detection capabilities. It was created by extracting and adapting tools from the Zed editor project into a modular, reusable MCP server.

## Repository

- **GitHub**: https://github.com/tsoernes/enhanced-terminal-mcp
- **Language**: Rust (Edition 2024)
- **License**: MIT

## Features

### Tools Provided

1. **enhanced_terminal** - Execute shell commands with PTY support
   - Full terminal emulation via portable-pty
   - Configurable working directory, shell, timeout, and output limits
   - Captures both stdout and stderr
   - Timeout protection with automatic process termination
   - Output size limits to prevent memory issues

2. **detect_binaries** - Detect developer tools and their versions
   - Concurrent scanning of PATH for common development tools
   - Version detection with configurable timeout
   - Organized into 16+ categories (rust_tools, python_tools, containers, etc.)
   - Filter by category for targeted detection
   - Reports binary paths and version strings

### Shell Detection (Built-in)

- Automatically detects available shells at server startup
- Shell information included in server instructions (visible to LLM)
- No separate tool call required - reduces latency and token usage
- Detects common shells: bash, zsh, fish, sh, dash, ksh, tcsh, csh
- Reports current shell from $SHELL environment variable
- Includes version information when available

## Architecture

### Modular Structure

```
src/
├── main.rs                          # Entry point (10 lines)
├── server.rs                        # MCP server implementation
├── detection/
│   ├── mod.rs                       # Module exports
│   └── binary_detector.rs           # Binary and shell detection logic
└── tools/
    ├── mod.rs                       # Module exports
    └── terminal_executor.rs         # Terminal execution with PTY
```

### Key Design Decisions

1. **Modularity**: Separated concerns into distinct modules rather than monolithic file
2. **Rust 2024**: Using latest stable Rust edition for modern features
3. **Shell Detection Integration**: Embedded shell info in server metadata instead of separate tool
4. **Reusable Logic**: Binary detection logic shared for both tools and shells
5. **Concurrent Scanning**: Uses thread pool for fast parallel version detection

### Dependencies

- `rmcp` 0.8 - Official Rust MCP SDK with stdio transport
- `tokio` 1.x - Async runtime
- `portable-pty` 0.8 - Cross-platform PTY for terminal emulation
- `serde` + `serde_json` - Serialization
- `schemars` 1.0 - JSON Schema generation
- `anyhow` 1.x - Error handling

## Binary Categories

The `detect_binaries` tool scans for tools in these categories:

- **package_managers**: npm, pip, cargo, dnf, apt, snap, flatpak, brew
- **rust_tools**: cargo, rustc, rustfmt, clippy
- **python_tools**: python, pip, pytest, black, ruff, mypy
- **build_systems**: make, cmake, ninja, gradle, maven
- **c_cpp_tools**: gcc, g++, clang, gdb, lldb
- **java_jvm_tools**: java, javac, kotlin
- **node_js_tools**: node, deno, bun, npm, yarn
- **go_tools**: go, gofmt
- **editors_dev**: vim, nvim, emacs, code, zed
- **search_productivity**: rg, fd, fzf, jq, bat, tree, exa
- **system_perf**: htop, ps, top, df, du
- **containers**: docker, podman, kubectl, helm
- **networking**: curl, wget, dig, traceroute
- **security**: openssl, gpg, ssh-keygen
- **databases**: sqlite3, psql, mysql, redis-cli
- **vcs**: git, gh

## Usage

### Building

```bash
cargo build --release
```

Binary located at: `target/release/enhanced-terminal-mcp`

### Running

```bash
./enhanced-terminal-mcp
```

The server uses stdio transport for MCP communication.

### Configuration Example (Claude Desktop)

```json
{
  "mcpServers": {
    "enhanced-terminal": {
      "command": "/path/to/enhanced-terminal-mcp"
    }
  }
}
```

### Configuration Example (Zed)

```json
{
  "context_servers": {
    "enhanced-terminal": {
      "command": "/path/to/enhanced-terminal-mcp",
      "args": []
    }
  }
}
```

## Development Timeline

1. **Initial Creation**: Extracted tools from Zed editor codebase
2. **MCP Integration**: Adapted to rmcp SDK with tool macros
3. **Modularization**: Refactored into clean module structure
4. **Shell Integration**: Moved shell detection into server metadata
5. **Edition Update**: Upgraded to Rust 2024

## Future Enhancements

Potential improvements:

- [ ] Add resource support for reading file contents
- [ ] Implement prompt support for common terminal tasks
- [ ] Add progress notifications for long-running commands
- [ ] Support for interactive commands (stdin input)
- [ ] Job management (background tasks, cancellation)
- [ ] Configurable binary detection groups via config file
- [ ] Windows-specific shell detection (PowerShell, cmd)
- [ ] Environment variable management
- [ ] Command history tracking
- [ ] Security features (command allowlist/denylist)

## Credits

Tools extracted and adapted from:
- [Zed Editor](https://github.com/zed-industries/zed)

Built with:
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [rmcp - Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)

## License

MIT License - See LICENSE file for details