use crate::detection::{detect_binaries, detect_shells};
use crate::tools::{TerminalExecutionInput, execute_command};
use rmcp::{
    ErrorData as McpError, handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DetectBinariesInput {
    /// Optional filter for specific categories
    #[serde(default)]
    filter_categories: Option<Vec<String>>,
    /// Maximum concurrent checks (default: 12)
    #[serde(default = "default_concurrency")]
    max_concurrency: usize,
    /// Version detection timeout in milliseconds (default: 1500)
    #[serde(default = "default_version_timeout")]
    version_timeout_ms: u64,
    /// Include missing binaries in output (default: false)
    #[serde(default)]
    include_missing: bool,
}

fn default_concurrency() -> usize {
    12
}

fn default_version_timeout() -> u64 {
    1500
}

#[derive(Clone)]
pub struct EnhancedTerminalServer {
    tool_router: ToolRouter<Self>,
    shell_info: String,
}

#[tool_router]
impl EnhancedTerminalServer {
    pub fn new() -> Self {
        // Detect shells at startup
        let shells = detect_shells();
        let shell_info = if shells.is_empty() {
            "No shells detected.".to_string()
        } else {
            let mut info = String::from("Available shells:\n");
            for shell in &shells {
                info.push_str(&format!("  - {} ({})", shell.name, shell.path));
                if let Some(ref version) = shell.version {
                    info.push_str(&format!(" - {}", version));
                }
                info.push('\n');
            }
            if let Ok(current_shell) = std::env::var("SHELL") {
                info.push_str(&format!("\nCurrent shell: {}", current_shell));
            }
            info
        };

        Self {
            tool_router: Self::tool_router(),
            shell_info,
        }
    }

    #[tool(description = "Execute shell commands with output capture and timeout")]
    async fn enhanced_terminal(
        &self,
        Parameters(input): Parameters<TerminalExecutionInput>,
    ) -> Result<CallToolResult, McpError> {
        let result = execute_command(input).map_err(|e| {
            McpError::internal_error(format!("Command execution failed: {}", e), None)
        })?;

        let mut result_text = format!("Command: {}\n", result.command);
        result_text.push_str(&format!(
            "Working Directory: {}\n",
            result.working_directory
        ));
        result_text.push_str(&format!("Exit Code: {:?}\n", result.exit_code));
        result_text.push_str(&format!("Success: {}\n", result.success));

        if result.timed_out {
            result_text.push_str("Status: TIMED OUT\n");
        }

        result_text.push_str("\nOutput:\n");
        result_text.push_str(&result.output);

        if result.truncated {
            result_text.push_str("\n\n[Output truncated due to size limit]");
        }

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(description = "Detect available developer binaries and their versions")]
    async fn detect_binaries(
        &self,
        Parameters(input): Parameters<DetectBinariesInput>,
    ) -> Result<CallToolResult, McpError> {
        let reports = detect_binaries(
            input.filter_categories,
            input.max_concurrency,
            input.version_timeout_ms,
            input.include_missing,
        );

        let json_output = serde_json::json!({
            "binaries": reports
        });

        let json_text = serde_json::to_string_pretty(&json_output)
            .map_err(|e| McpError::internal_error(format!("Serialization error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json_text)]))
    }
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for EnhancedTerminalServer {
    fn get_info(&self) -> ServerInfo {
        let instructions = format!(
            "Enhanced terminal MCP server with command execution and binary detection.\n\
            \n\
            Tools:\n\
            - enhanced_terminal: Execute shell commands with PTY support, output capture, and timeouts\n\
            - detect_binaries: Detect developer tools and their versions across multiple categories\n\
            \n\
            Binary Categories:\n\
            - package_managers (npm, pip, cargo, dnf, apt, snap, flatpak, brew)\n\
            - rust_tools (cargo, rustc, rustfmt, clippy)\n\
            - python_tools (python, pip, pytest, black, ruff, mypy)\n\
            - build_systems (make, cmake, ninja, gradle, maven)\n\
            - c_cpp_tools (gcc, g++, clang, gdb, lldb)\n\
            - java_jvm_tools (java, javac, kotlin)\n\
            - node_js_tools (node, deno, bun, npm, yarn)\n\
            - go_tools (go, gofmt)\n\
            - editors_dev (vim, nvim, emacs, code, zed)\n\
            - search_productivity (rg, fd, fzf, jq, bat, tree, exa)\n\
            - system_perf (htop, ps, top, df, du)\n\
            - containers (docker, podman, kubectl, helm)\n\
            - networking (curl, wget, dig, traceroute)\n\
            - security (openssl, gpg, ssh-keygen)\n\
            - databases (sqlite3, psql, mysql, redis-cli)\n\
            - vcs (git, gh)\n\
            \n\
            {}\n\
            \n\
            Extracted and adapted from the Zed editor project.",
            self.shell_info
        );

        ServerInfo {
            instructions: Some(instructions),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
