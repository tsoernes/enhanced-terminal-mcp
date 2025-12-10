# Enhanced Terminal MCP Server

A standalone Model Context Protocol (MCP) server that provides terminal execution, binary detection, and shell detection capabilities. This server extracts and reimplements key tools from the Zed editor project.

## Features

### Tools

1. **enhanced_terminal** - Execute shell commands with output capture and timeout
   - Run commands in a PTY with configurable working directory
   - Configurable shell, timeout, and output limits
   - Captures stdout/stderr with proper terminal emulation

2. **detect_binaries** - Detect available developer binaries and their versions
   - Scans PATH for common development tools
   - Detects versions concurrently with configurable timeout
   - Supports filtering by category (rust_tools, python_tools, etc.)
   - Categories include: package managers, build systems, programming language tools, editors, containers, and more

**Note:** Shell information is automatically detected at server startup and included in the server instructions, so no separate tool call is needed to discover available shells.

## Installation

### Prerequisites

- Rust 1.70 or later
- Cargo

### Build from Source

```bash
git clone <repository-url>
cd enhanced-terminal-mcp
cargo build --release
```

The binary will be located at `target/release/enhanced-terminal-mcp`.

## Usage

### Running the Server

The server uses stdio transport for MCP communication:

```bash
./enhanced-terminal-mcp
```

### Configuration

Add to your MCP client configuration (e.g., Claude Desktop, Zed):

```json
{
  "mcpServers": {
    "enhanced-terminal": {
      "command": "/path/to/enhanced-terminal-mcp",
      "args": []
    }
  }
}
```

### Tool Examples

#### enhanced_terminal

```json
{
  "command": "ls -la",
  "cwd": ".",
  "shell": "bash",
  "output_limit": 16384,
  "timeout_secs": 60
}
```

#### detect_binaries

```json
{
  "filter_categories": ["rust_tools", "python_tools"],
  "max_concurrency": 12,
  "version_timeout_ms": 1500,
  "include_missing": false
}
```



## Binary Categories

The `detect_binaries` tool supports filtering by these categories:

- `package_managers` - npm, pip, cargo, dnf, apt, etc.
- `rust_tools` - cargo, rustc, rustfmt, clippy
- `python_tools` - python, pip, pytest, black, ruff
- `build_systems` - make, cmake, ninja
- `c_cpp_tools` - gcc, g++, clang, gdb
- `java_jvm_tools` - java, javac
- `node_js_tools` - node, deno, bun
- `go_tools` - go, gofmt
- `editors_dev` - vim, nvim, emacs, code
- `search_productivity` - rg, fd, fzf, jq, bat, tree
- `system_perf` - htop, ps, top, df, du
- `containers` - docker, podman, kubectl
- `networking` - curl, wget, dig
- `security` - openssl, gpg, ssh-keygen
- `databases` - sqlite3, psql, mysql
- `vcs` - git, gh

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Running Locally

```bash
cargo run
```

## Architecture

This server uses a modular structure with Rust 2024 edition:

- `src/main.rs` - Entry point and server initialization
- `src/server.rs` - MCP server implementation with tool handlers
- `src/detection/` - Binary and shell detection logic
- `src/tools/` - Terminal execution implementation

### Dependencies

- **rmcp** - Official Rust SDK for Model Context Protocol
- **tokio** - Async runtime
- **portable-pty** - Cross-platform PTY support for terminal emulation
- **serde/serde_json** - Serialization
- **schemars** - JSON Schema generation for tool inputs
- **anyhow** - Error handling

## License

MIT License - see [LICENSE](LICENSE) file for details

## Credits

Tools extracted and adapted from the [Zed editor](https://github.com/zed-industries/zed) project.