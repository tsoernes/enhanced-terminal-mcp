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
    /// If true, return only new output since last check (default: true)
    #[serde(default = "default_incremental")]
    pub incremental: bool,
    /// Offset for pagination (bytes into output, default: 0)
    #[serde(default)]
    pub offset: usize,
    /// Limit for pagination (bytes to return, default: 0 = all)
    #[serde(default)]
    pub limit: usize,
}

fn default_incremental() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobListInput {
    /// Maximum number of jobs to return (default: 50)
    #[serde(default = "default_max_jobs")]
    pub max_jobs: usize,
    /// Filter by job status (e.g., ["Running", "Completed"])
    #[serde(default)]
    pub status_filter: Option<Vec<String>>,
    /// Filter by tag (e.g., "build")
    #[serde(default)]
    pub tag_filter: Option<String>,
    /// Filter by working directory
    #[serde(default)]
    pub cwd_filter: Option<String>,
    /// Sort order: "newest" (default) or "oldest"
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

fn default_max_jobs() -> usize {
    50
}

fn default_sort_order() -> String {
    "newest".to_string()
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
        name = "enhanced_terminal",
        description = "Execute shell commands in a PTY with smart async switching and security.

PARAMETERS:
- command (string, required): The shell command to execute
- cwd (string, default: '.'): Working directory for command execution
- shell (string, default: 'bash'): Shell to use from available shells (see below)
- output_limit (number, default: 16384): Maximum output size in bytes
- timeout_secs (number, default: None): Timeout in seconds (0 or None = no timeout)
- async_threshold_secs (number, default: 50): Auto-switch to background after this many seconds
- env_vars (object, default: {}): Environment variables to set (e.g. {\"PATH\": \"/usr/bin\", \"DEBUG\": \"true\"})
- force_sync (boolean, default: false): Force synchronous execution regardless of duration
- custom_denylist (array, default: []): Additional dangerous patterns to block
- tags (array, default: []): Optional tags for categorizing jobs (e.g., [\"build\", \"ci\"])
- stream (boolean, default: false): Enable streaming mode - immediately switches to background for real-time output polling

AVAILABLE SHELLS:
{shell_list}

BEHAVIOR:
- Commands running longer than async_threshold_secs automatically switch to background
- Returns job_id for tracking via enhanced_terminal_job_status
- Security denylist blocks dangerous commands (rm -rf /, shutdown, fork bombs, etc.)
- PTY support preserves colors and terminal features
- Incremental output captured during background execution

SECURITY:
- 40+ dangerous patterns blocked by default
- Custom patterns via custom_denylist parameter
- No privilege escalation without explicit configuration
- Output size limits prevent memory exhaustion
- Timeout protection prevents runaway processes

RETURNS:
- job_id: Unique identifier for this command execution
- command: The executed command
- working_directory: Resolved working directory path
- exit_code: Exit code (if completed, null if still running)
- success: Boolean indicating success (if completed)
- output: Command output (truncated to output_limit for preview)
- truncated: Boolean indicating if output was truncated
- timed_out: Boolean indicating if command was killed by timeout
- switched_to_async: Boolean indicating if command moved to background
- denied: Boolean indicating if command was blocked
- denial_reason: Reason for denial (if denied)"
    )]
    async fn enhanced_terminal(
        &self,
        Parameters(input): Parameters<TerminalExecutionInput>,
    ) -> Result<CallToolResult, McpError> {
        // Replace {shell_list} placeholder in description with actual shells
        // (Note: This is done at runtime in the description, but we'll handle it in server instructions)
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
            if let Some(duration) = result.duration_secs {
                result_text.push_str(&format!(
                    "Duration: {:.2}s (switched to background)\n",
                    duration
                ));
            }
            if result.streaming {
                result_text.push_str("Status: STREAMING MODE\n");
                result_text
                    .push_str("The command is running in background with streaming enabled.\n");
                result_text.push_str("Use enhanced_terminal_job_status with incremental=true to poll for real-time updates.\n");
                result_text
                    .push_str("Each call will return only new output since the last check.\n");
            } else {
                result_text.push_str("Status: SWITCHED TO BACKGROUND\n");
                result_text.push_str("The command is still running. Use enhanced_terminal_job_status to check progress.\n");
            }
            result_text.push_str("\nPartial Output:\n");
            result_text.push_str(&result.output);
            if result.truncated {
                result_text
                    .push_str("\n\n[Output preview truncated - use enhanced_terminal_job_status to get full output]");
            }
        } else {
            // Show duration first for completed/timed out commands
            if let Some(duration) = result.duration_secs {
                result_text.push_str(&format!("Duration: {:.2}s\n", duration));
            }

            result_text.push_str(&format!("Exit Code: {:?}\n", result.exit_code));
            result_text.push_str(&format!("Success: {}\n", result.success));

            if result.timed_out {
                result_text.push_str("Status: TIMED OUT ⏱️\n");
            } else if result.success {
                result_text.push_str("Status: COMPLETED ✅\n");
            } else {
                result_text.push_str("Status: FAILED ❌\n");
            }

            result_text.push_str("\nOutput:\n");
            result_text.push_str(&result.output);

            if result.truncated {
                result_text.push_str("\n\n[Output truncated due to size limit]");
            }
        }

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(
        name = "enhanced_terminal_job_status",
        description = "Get status and output of a background job.

PARAMETERS:
- job_id (string, required): The job identifier returned by enhanced_terminal
- incremental (boolean, default: true): If true, return only new output since last check (RECOMMENDED)
- offset (number, default: 0): Starting byte position for pagination
- limit (number, default: 0): Maximum bytes to return (0 = all)

BEHAVIOR:
- Returns current status: Running, Completed, Failed, TimedOut, or Canceled
- Full output available (up to output_limit)
- Incremental mode tracks read position per job
- Duration calculated from start time
- Exit code available when completed
- Supports three output modes: incremental, full, and paginated

INCREMENTAL OUTPUT (DEFAULT):
When incremental=true (default, recommended):
- First call returns all output accumulated so far
- Subsequent calls return only new output since last check
- Read position maintained per job_id
- Efficient for polling long-running jobs
- More responsive than full output mode
- Reset position by calling with incremental=false to get all output again

PAGINATION MODE:
When offset > 0 or limit > 0:
- Returns specific byte range of output
- offset: Starting position in bytes
- limit: Number of bytes to return (0 = all remaining)
- Returns has_more flag indicating if more data available
- Returns total_length for overall output size
- Useful for seeking into very long logs
- Can re-read specific segments without full retrieval

RETURNS:
- job_id: Job identifier
- command: The executed command
- summary: First 100 characters of command
- shell: Shell used for execution
- cwd: Working directory
- status: Current job status (Running, Completed, Failed, TimedOut, Canceled)
- exit_code: Exit code (if completed)
- pid: Process ID (if available)
- duration: Time elapsed since job start
- tags: Optional tags assigned to job
- output: Command output (full, incremental, or paginated based on parameters)
- truncated: Boolean indicating if output preview was truncated
- (pagination only) has_more: Boolean indicating if more data available
- (pagination only) total_length: Total output size in bytes"
    )]
    async fn job_status(
        &self,
        Parameters(input): Parameters<JobStatusInput>,
    ) -> Result<CallToolResult, McpError> {
        // Determine if pagination is requested
        let use_pagination = input.offset > 0 || input.limit > 0;

        let (output_to_show, has_more, total_length) = if use_pagination {
            // Use pagination
            let limit = if input.limit == 0 {
                usize::MAX
            } else {
                input.limit
            };

            let (output, has_more, total) = self
                .job_manager
                .get_output_range(&input.job_id, input.offset, limit)
                .ok_or_else(|| {
                    McpError::invalid_params("Job not found", None::<serde_json::Value>)
                })?;

            (output, Some(has_more), Some(total))
        } else if input.incremental {
            // Get incremental output
            let (new_output, is_running) = self
                .job_manager
                .get_incremental_output(&input.job_id)
                .ok_or_else(|| {
                McpError::invalid_params("Job not found", None::<serde_json::Value>)
            })?;

            if new_output.is_empty() && !is_running {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Job {} has no new output. Status: Completed.\nUse incremental=false to see all output.",
                    input.job_id
                ))]));
            }

            (new_output, None, None)
        } else {
            // Get full output
            let job = self.job_manager.get_job(&input.job_id).ok_or_else(|| {
                McpError::invalid_params("Job not found", None::<serde_json::Value>)
            })?;
            (job.output.clone(), None, None)
        };

        // Always get current job info for metadata
        let job = self
            .job_manager
            .get_job(&input.job_id)
            .ok_or_else(|| McpError::invalid_params("Job not found", None::<serde_json::Value>))?;

        let mut result_text = format!("Job ID: {}\n", job.job_id);
        result_text.push_str(&format!("Command: {}\n", job.command));
        result_text.push_str(&format!("Summary: {}\n", job.summary));
        result_text.push_str(&format!("Shell: {}\n", job.shell));
        result_text.push_str(&format!("Working Directory: {}\n", job.cwd));
        result_text.push_str(&format!("Status: {:?}\n", job.status));

        if !job.tags.is_empty() {
            result_text.push_str(&format!("Tags: {}\n", job.tags.join(", ")));
        }

        // Use the duration helper method
        result_text.push_str(&format!("Duration: {}\n", job.duration_string()));

        if let Some(exit_code) = job.exit_code {
            result_text.push_str(&format!("Exit Code: {}\n", exit_code));
        }

        if let Some(pid) = job.pid {
            result_text.push_str(&format!("PID: {}\n", pid));
        }

        if use_pagination {
            result_text.push_str(&format!(
                "Output Mode: Paginated (offset: {}, limit: {})\n",
                input.offset,
                if input.limit == 0 {
                    "all".to_string()
                } else {
                    input.limit.to_string()
                }
            ));
            if let Some(total) = total_length {
                result_text.push_str(&format!("Total Output Length: {} bytes\n", total));
            }
            if let Some(more) = has_more {
                result_text.push_str(&format!("Has More: {}\n", more));
            }
        } else if input.incremental {
            result_text.push_str(&format!(
                "Output Mode: Incremental (new since last check)\n"
            ));
        } else {
            result_text.push_str(&format!("Output Mode: Full\n"));
        }

        result_text.push_str("\nOutput:\n");
        result_text.push_str(&output_to_show);

        if job.truncated && !input.incremental && !use_pagination {
            result_text.push_str("\n\n[Output truncated - showing first part only]");
        }

        if use_pagination {
            if let (Some(more), Some(_total)) = (has_more, total_length) {
                if more {
                    let next_offset = input.offset + output_to_show.len();
                    result_text.push_str(&format!(
                        "\n\n[More output available. Next offset: {}]",
                        next_offset
                    ));
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(
        name = "enhanced_terminal_job_list",
        description = "List all background jobs with status and output previews.

PARAMETERS:
- max_jobs (number, default: 50): Maximum number of jobs to return
- status_filter (array, optional): Filter by status (e.g., [\"Running\", \"Completed\"])
- tag_filter (string, optional): Filter by tag (e.g., \"build\")
- cwd_filter (string, optional): Filter by working directory
- sort_order (string, default: \"newest\"): Sort order (\"newest\" or \"oldest\")

BEHAVIOR:
- Jobs sorted by start time (newest first by default)
- Shows running and completed jobs
- Output preview limited to first 100 characters
- Includes duration, exit codes, tags, and summary

FILTERING:
- status_filter: Match any of the provided statuses
  - Valid values: \"Running\", \"Completed\", \"Failed\", \"TimedOut\", \"Canceled\"
- tag_filter: Show only jobs with the specified tag
- cwd_filter: Show only jobs from a specific directory
- Filters are combined with AND logic

RETURNS: List of jobs with:
- job_id: Unique identifier
- command: Full executed command
- summary: First 100 characters of command
- status: Current status (Running, Completed, Failed, TimedOut, Canceled)
- exit_code: Exit code if completed
- duration: Time elapsed since start
- tags: Optional tags assigned to this job
- cwd: Working directory
- shell: Shell used
- output_preview: First 100 characters of output"
    )]
    async fn job_list(
        &self,
        Parameters(input): Parameters<JobListInput>,
    ) -> Result<CallToolResult, McpError> {
        use crate::tools::JobStatus;

        // Parse status filter if provided
        let status_filter: Option<Vec<JobStatus>> = input.status_filter.as_ref().map(|filters| {
            filters
                .iter()
                .filter_map(|s| match s.as_str() {
                    "Running" => Some(JobStatus::Running),
                    "Completed" => Some(JobStatus::Completed),
                    "Failed" => Some(JobStatus::Failed),
                    "TimedOut" => Some(JobStatus::TimedOut),
                    "Canceled" => Some(JobStatus::Canceled),
                    _ => None,
                })
                .collect()
        });

        // Get filtered jobs
        let mut jobs = if input.status_filter.is_some()
            || input.tag_filter.is_some()
            || input.cwd_filter.is_some()
        {
            self.job_manager.list_jobs_filtered(
                status_filter.as_deref(),
                input.tag_filter.as_deref(),
                input.cwd_filter.as_deref(),
            )
        } else {
            self.job_manager.list_jobs()
        };

        // Apply sort order
        if input.sort_order == "oldest" {
            jobs.reverse();
        }

        let jobs_to_show = jobs.into_iter().take(input.max_jobs).collect::<Vec<_>>();

        if jobs_to_show.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No jobs found.".to_string(),
            )]));
        }

        let mut result_text = format!("Found {} job(s):\n\n", jobs_to_show.len());

        for job in jobs_to_show {
            result_text.push_str(&format!("Job ID: {}\n", job.job_id));
            result_text.push_str(&format!("  Summary: {}\n", job.summary));
            result_text.push_str(&format!("  Status: {:?}\n", job.status));
            result_text.push_str(&format!("  CWD: {}\n", job.cwd));
            result_text.push_str(&format!("  Shell: {}\n", job.shell));

            if !job.tags.is_empty() {
                result_text.push_str(&format!("  Tags: {}\n", job.tags.join(", ")));
            }

            if let Some(exit_code) = job.exit_code {
                result_text.push_str(&format!("  Exit Code: {}\n", exit_code));
            }

            // Use the duration helper method
            result_text.push_str(&format!("  Duration: {}\n", job.duration_string()));

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

    #[tool(
        name = "enhanced_terminal_job_cancel",
        description = "Cancel a running background job by sending SIGTERM (Unix only).

PARAMETERS:
- job_id (string, required): The job identifier to cancel

BEHAVIOR:
- Sends SIGTERM signal to the process (Unix only)
- Graceful termination attempt
- Updates job status to Canceled
- Works with process groups

PLATFORM SUPPORT:
- Unix/Linux/macOS: Full support with SIGTERM
- Windows: Limited support (status update only, no signal)

RETURNS:
- Confirmation message with job_id
- Instructions to use job_status to verify cancellation"
    )]
    async fn job_cancel(
        &self,
        Parameters(input): Parameters<JobCancelInput>,
    ) -> Result<CallToolResult, McpError> {
        self.job_manager
            .cancel_job(&input.job_id)
            .map_err(|e| McpError::internal_error(format!("Failed to cancel job: {}", e), None))?;

        let result_text = format!(
            "Job {} has been canceled. Use enhanced_terminal_job_status to verify.",
            input.job_id
        );

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(
        name = "detect_binaries",
        description = "Detect developer tools and their versions with fast parallel scanning.

PARAMETERS:
- filter_categories (array, optional): Category names to scan (e.g. [\"rust_tools\", \"python_tools\"])
- max_concurrency (number, default: 16): Number of concurrent version checks
- version_timeout_ms (number, default: 1500): Timeout per binary version check in milliseconds
- include_missing (boolean, default: false): Include binaries not found in PATH

CATEGORIES (100+ tools):
- package_managers: npm, pip, cargo, dnf, apt, snap, flatpak, brew
- rust_tools: cargo, rustc, rustfmt, clippy
- python_tools: python, pip, pytest, black, ruff, mypy
- build_systems: make, cmake, ninja, gradle, maven
- c_cpp_tools: gcc, g++, clang, gdb, lldb
- java_jvm_tools: java, javac, kotlin
- node_js_tools: node, deno, bun, npm, yarn
- go_tools: go, gofmt
- editors_dev: vim, nvim, emacs, code, zed
- search_productivity: rg, fd, fzf, jq, bat, tree, exa
- system_perf: htop, ps, top, df, du
- containers: docker, podman, kubectl, helm
- networking: curl, wget, dig, traceroute
- security: openssl, gpg, ssh-keygen
- databases: sqlite3, psql, mysql, redis-cli
- vcs: git, gh

PERFORMANCE:
- 16 concurrent checks by default
- ~2-3 seconds for all categories
- Configurable timeout per binary
- Efficient PATH scanning

RETURNS: JSON array of binaries with:
- name: Binary name
- category: Category identifier
- found: Boolean indicating if binary exists
- path: Full path (or paths separated by ';' if multiple)
- version: Version string (if detected)
- error: Error message (if version detection failed)"
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
            "Enhanced Terminal MCP Server - Production-ready command execution with job management.\n\
            \n\
            CORE FEATURES:\n\
            • Smart async switching: Commands auto-background after 50s (configurable)\n\
            • Job management: Full tracking, status, output streaming, cancellation\n\
            • Security: 40+ dangerous patterns blocked by default\n\
            • Performance: 16 concurrent binary detection\n\
            • Environment: Full env var support, PTY terminal emulation\n\
            \n\
            TOOLS:\n\
            \n\
            1. enhanced_terminal - Execute shell commands\n\
               • Default shell: bash\n\
               • Available shells: {}\n\
               • Smart async: Auto-background after 50s (async_threshold_secs)\n\
               • No timeout by default (timeout_secs: 0 or None)\n\
               • Environment variables: Set via env_vars parameter\n\
               • Security: Denylist blocks rm -rf /, shutdown, fork bombs, etc.\n\
               • Output: 16KB limit (configurable), captured incrementally\n\
               • Returns: job_id for tracking background execution\n\
            \n\
            2. enhanced_terminal_job_status - Monitor background jobs\n\
               • Get current status: Running, Completed, Failed, TimedOut, Canceled\n\
               • Output modes: Incremental (default, new since last check) or Full (all output)\n\
               • Incremental mode is default and recommended for efficiency\n\
               • Set incremental=false to get all output from start\n\
               • Returns: status, exit_code, duration, output, PID\n\
            \n\
            3. enhanced_terminal_job_list - List all jobs\n\
               • Shows recent jobs (newest first)\n\
               • Configurable limit (default: 50)\n\
               • Quick overview with output previews\n\
               • Filter by status if needed\n\
            \n\
            4. job_cancel - Cancel running jobs\n\
               • Sends SIGTERM (Unix/Linux/macOS only)\n\
               • Graceful process termination\n\
               • Updates job status to Canceled\n\
            \n\
            5. detect_binaries - Fast tool detection\n\
               • Scans 100+ developer tools across 16 categories\n\
               • 16 concurrent checks (~2-3 seconds total)\n\
               • Filter by category for targeted detection\n\
               • Returns: paths, versions, availability\n\
            \n\
            SMART ASYNC:\n\
            Commands exceeding async_threshold_secs (default: 50s) automatically move to background.\n\
            Returns immediately with job_id. Use enhanced_terminal_job_status with incremental=true to poll for updates.\n\
            Set force_sync=true to wait for completion regardless of duration.\n\
            \n\
            STREAMING MODE:\n\
            Set stream=true to immediately run in background for real-time output:\n\
            • Command starts in background instantly (async_threshold bypassed)\n\
            • Returns job_id immediately\n\
            • Poll with enhanced_terminal_job_status (incremental=true) for live updates\n\
            • Each poll returns only new output since last check\n\
            • Perfect for long-running commands where you want live feedback\n\
            • Example: stream compilation, test execution, or deployment logs\n\
            \n\
            ENVIRONMENT VARIABLES:\n\
            Set environment variables via env_vars parameter:\n\
            {{\"PATH\": \"/custom/path\", \"DEBUG\": \"true\", \"NODE_ENV\": \"production\"}}\n\
            \n\
            SECURITY DENYLIST:\n\
            Blocks dangerous patterns (40+ default):\n\
            • Destructive: rm -rf /, mkfs, dd if=/dev/zero, > /dev/sda\n\
            • System: shutdown, reboot, halt, chmod 777 /, chown -R root\n\
            • Exhaustion: fork bombs (:(){{:|:&}};:), infinite loops\n\
            • Kernel: rmmod, insmod, modprobe\n\
            • Cron: crontab -r\n\
            • Custom patterns: Add via custom_denylist parameter\n\
            \n\
            INCREMENTAL OUTPUT (DEFAULT):\n\
            enhanced_terminal_job_status uses incremental mode by default (recommended):\n\
            • First call: Returns all output accumulated so far\n\
            • Subsequent calls: Only new output since last check\n\
            • Efficient polling for long-running jobs\n\
            • More responsive than full output mode\n\
            • Set incremental=false to get full output from start\n\
            \n\
            AVAILABLE SHELLS:\n\
            {}\n\
            \n\
            BINARY CATEGORIES:\n\
            package_managers, rust_tools, python_tools, build_systems, c_cpp_tools,\n\
            java_jvm_tools, node_js_tools, go_tools, editors_dev, search_productivity,\n\
            system_perf, containers, networking, security, databases, vcs\n\
            \n\
            EXAMPLES:\n\
            \n\
            Quick command:\n\
            {{\"command\": \"ls -la\", \"cwd\": \".\" }}\n\
            \n\
            Long-running with env vars:\n\
            {{\"command\": \"npm install\", \"env_vars\": {{\"NODE_ENV\": \"production\"}}, \"async_threshold_secs\": 30}}\n\
            \n\
            Monitor job:\n\
            {{\"job_id\": \"job-123\", \"incremental\": true}}\n\
            \n\
            Detect Python tools:\n\
            {{\"filter_categories\": [\"python_tools\"], \"max_concurrency\": 16}}\n\
            \n\
            Extracted and adapted from the Zed editor project.",
            self.shell_info, self.shell_info
        );

        ServerInfo {
            instructions: Some(instructions),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
