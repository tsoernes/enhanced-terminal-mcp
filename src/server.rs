use crate::detection::{detect_binaries, detect_shells};
use crate::tools::{JobManager, TerminalExecutionInput, execute_command};
use rmcp::{
    ErrorData as McpError, handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DetectBinariesInput {
    /// Optional filter for specific categories
    #[serde(default)]
    filter_categories: Option<Vec<String>>,
    /// Maximum concurrent checks (default: 16)
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
    16
}

fn default_version_timeout() -> u64 {
    1500
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobStatusInput {
    /// Job ID to query
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobListInput {
    /// Maximum number of jobs to return (default: 50)
    #[serde(default = "default_max_jobs")]
    pub max_jobs: usize,
}

fn default_max_jobs() -> usize {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobCancelInput {
    /// Job ID to cancel
    pub job_id: String,
}

#[derive(Clone)]
pub struct EnhancedTerminalServer {
    tool_router: ToolRouter<Self>,
    shell_info: String,
    job_manager: JobManager,
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
            job_manager: JobManager::new(),
        }
    }

    #[tool(
        description = "Execute shell commands with output capture, timeout, and smart async switching. Commands automatically switch to background after async_threshold_secs (default: 5s). Includes denylist protection against dangerous commands."
    )]
    async fn enhanced_terminal(
        &self,
        Parameters(input): Parameters<TerminalExecutionInput>,
    ) -> Result<CallToolResult, McpError> {
        let result = execute_command(input, &self.job_manager).map_err(|e| {
            McpError::internal_error(format!("Command execution failed: {}", e), None)
        })?;

        if result.denied {
            let mut result_text = format!("Command: {}\n", result.command);
            result_text.push_str(&format!(
                "Status: DENIED\n\nReason: {}\n",
                result
                    .denial_reason
                    .unwrap_or_else(|| "Security policy violation".to_string())
            ));
            return Ok(CallToolResult::success(vec![Content::text(result_text)]));
        }

        let mut result_text = format!("Job ID: {}\n", result.job_id);
        result_text.push_str(&format!("Command: {}\n", result.command));
        result_text.push_str(&format!(
            "Working Directory: {}\n",
            result.working_directory
        ));

        if result.switched_to_async {
            result_text.push_str("\nStatus: SWITCHED TO BACKGROUND\n");
            result_text
                .push_str("The command is still running. Use job_status to check progress.\n");
            result_text.push_str("\nPartial Output:\n");
            result_text.push_str(&result.output);
            if result.truncated {
                result_text
                    .push_str("\n\n[Output preview truncated - use job_status to get full output]");
            }
        } else {
            result_text.push_str(&format!("Exit Code: {:?}\n", result.exit_code));
            result_text.push_str(&format!("Success: {}\n", result.success));

            if result.timed_out {
                result_text.push_str("Status: TIMED OUT\n");
            } else {
                result_text.push_str("Status: COMPLETED\n");
            }

            result_text.push_str("\nOutput:\n");
            result_text.push_str(&result.output);

            if result.truncated {
                result_text.push_str("\n\n[Output truncated due to size limit]");
            }
        }

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(description = "Get the status and output of a background job by job ID")]
    async fn job_status(
        &self,
        Parameters(input): Parameters<JobStatusInput>,
    ) -> Result<CallToolResult, McpError> {
        let job = self
            .job_manager
            .get_job(&input.job_id)
            .ok_or_else(|| McpError::invalid_params("Job not found", None::<serde_json::Value>))?;

        let mut result_text = format!("Job ID: {}\n", job.job_id);
        result_text.push_str(&format!("Command: {}\n", job.command));
        result_text.push_str(&format!("Shell: {}\n", job.shell));
        result_text.push_str(&format!("Working Directory: {}\n", job.cwd));
        result_text.push_str(&format!("Status: {:?}\n", job.status));

        if let Some(exit_code) = job.exit_code {
            result_text.push_str(&format!("Exit Code: {}\n", exit_code));
        }

        if let Some(pid) = job.pid {
            result_text.push_str(&format!("PID: {}\n", pid));
        }

        // Calculate duration
        let duration = if let Some(finished) = job.finished_at {
            finished
                .duration_since(job.started_at)
                .map(|d| format!("{:.2}s", d.as_secs_f64()))
                .unwrap_or_else(|_| "unknown".to_string())
        } else {
            std::time::SystemTime::now()
                .duration_since(job.started_at)
                .map(|d| format!("{:.2}s (still running)", d.as_secs_f64()))
                .unwrap_or_else(|_| "unknown".to_string())
        };
        result_text.push_str(&format!("Duration: {}\n", duration));

        result_text.push_str("\nOutput:\n");
        result_text.push_str(&job.output);

        if job.truncated {
            result_text.push_str("\n\n[Output truncated - showing first part only]");
        }

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(description = "List all background jobs (running and completed)")]
    async fn job_list(
        &self,
        Parameters(input): Parameters<JobListInput>,
    ) -> Result<CallToolResult, McpError> {
        let jobs = self.job_manager.list_jobs();
        let jobs_to_show = jobs.into_iter().take(input.max_jobs).collect::<Vec<_>>();

        if jobs_to_show.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No jobs found.".to_string(),
            )]));
        }

        let mut result_text = format!("Found {} job(s):\n\n", jobs_to_show.len());

        for job in jobs_to_show {
            result_text.push_str(&format!("Job ID: {}\n", job.job_id));
            result_text.push_str(&format!("  Command: {}\n", job.command));
            result_text.push_str(&format!("  Status: {:?}\n", job.status));

            if let Some(exit_code) = job.exit_code {
                result_text.push_str(&format!("  Exit Code: {}\n", exit_code));
            }

            // Calculate duration
            let duration = if let Some(finished) = job.finished_at {
                finished
                    .duration_since(job.started_at)
                    .map(|d| format!("{:.2}s", d.as_secs_f64()))
                    .unwrap_or_else(|_| "unknown".to_string())
            } else {
                std::time::SystemTime::now()
                    .duration_since(job.started_at)
                    .map(|d| format!("{:.2}s (running)", d.as_secs_f64()))
                    .unwrap_or_else(|_| "unknown".to_string())
            };
            result_text.push_str(&format!("  Duration: {}\n", duration));

            // Show output preview (first 100 chars)
            let preview = if job.output.len() > 100 {
                format!("{}...", &job.output[..100])
            } else {
                job.output.clone()
            };
            result_text.push_str(&format!("  Output Preview: {}\n", preview.trim()));
            result_text.push_str("\n");
        }

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(description = "Cancel a running background job by job ID (Unix only)")]
    async fn job_cancel(
        &self,
        Parameters(input): Parameters<JobCancelInput>,
    ) -> Result<CallToolResult, McpError> {
        self.job_manager
            .cancel_job(&input.job_id)
            .map_err(|e| McpError::internal_error(format!("Failed to cancel job: {}", e), None))?;

        let result_text = format!(
            "Job {} has been canceled. Use job_status to verify.",
            input.job_id
        );

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(
        description = "Detect available developer binaries and their versions (16 concurrent checks)"
    )]
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
            "Enhanced terminal MCP server with command execution, job management, and binary detection.\n\
            \n\
            Tools:\n\
            - enhanced_terminal: Execute shell commands with PTY support, output capture, and smart async switching\n\
              * Automatically switches to background after 5 seconds (configurable via async_threshold_secs)\n\
              * Set force_sync=true to wait for completion regardless of duration\n\
              * Includes denylist protection against dangerous commands\n\
              * Default timeout: 300 seconds\n\
              * Output limit: 16KB (configurable)\n\
            - job_status: Check status and get output of background jobs\n\
            - job_list: List all jobs (running and completed)\n\
            - job_cancel: Cancel a running background job (Unix only)\n\
            - detect_binaries: Detect developer tools with 16 concurrent checks\n\
            \n\
            Smart Async Behavior:\n\
            Commands that run longer than async_threshold_secs (default: 5s) automatically switch to\n\
            background execution and return immediately with a job ID. Use job_status to check progress.\n\
            \n\
            Security:\n\
            Dangerous commands are blocked by default denylist including:\n\
            - Destructive operations: rm -rf /, mkfs, dd if=/dev/zero\n\
            - System manipulation: shutdown, reboot, chmod 777 /\n\
            - Fork bombs and resource exhaustion\n\
            - Custom patterns can be added via custom_denylist parameter\n\
            \n\
            Binary Categories (16 concurrent checks):\n\
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
