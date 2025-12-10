use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use rmcp::{
    handler::server::router::tool::ToolRouter, handler::server::wrapper::Parameters, model::*,
    tool, tool_handler, tool_router, transport::stdio, ErrorData as McpError, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::io::Read;
use std::path::Path;
use std::process::Command as StdCommand;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

// ============================================================================
// Job Management for Enhanced Terminal
// ============================================================================

#[derive(Debug, Clone)]
struct JobRecord {
    command: String,
    started_at: SystemTime,
    finished_at: Option<SystemTime>,
    exit_code: Option<i32>,
    success: bool,
    output: String,
    full_output: String,
    truncated: bool,
    canceled: bool,
    pid: Option<u32>,
}

static JOB_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn new_job_id() -> String {
    format!(
        "job-{}",
        JOB_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    )
}

// ============================================================================
// Enhanced Terminal Tool
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct EnhancedTerminalInput {
    /// Command to execute
    command: String,
    /// Working directory (default: ".")
    #[serde(default = "default_cwd")]
    cwd: String,
    /// Shell to use (default: "sh")
    #[serde(default = "default_shell")]
    shell: String,
    /// Output limit in bytes (default: 16384)
    #[serde(default = "default_output_limit")]
    output_limit: usize,
    /// Timeout in seconds (default: 60)
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
}

fn default_cwd() -> String {
    ".".to_string()
}

fn default_shell() -> String {
    "sh".to_string()
}

fn default_output_limit() -> usize {
    16 * 1024
}

fn default_timeout() -> u64 {
    60
}

// ============================================================================
// Detect Binaries Tool
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct DetectBinariesInput {
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

#[derive(Debug, Serialize)]
struct BinaryReport {
    name: String,
    category: String,
    found: bool,
    path: Option<String>,
    version: Option<String>,
    error: Option<String>,
}

// Base candidate groups for binary detection
const BASE_CANDIDATE_GROUPS: &[(&str, &[&str])] = &[
    (
        "package_managers",
        &[
            "npm", "pip", "cargo", "dnf", "apt", "snap", "flatpak", "brew",
        ],
    ),
    (
        "rust_tools",
        &["cargo", "rustc", "rustfmt", "clippy-driver"],
    ),
    (
        "python_tools",
        &[
            "python", "python3", "pip", "pytest", "black", "ruff", "mypy",
        ],
    ),
    (
        "build_systems",
        &["make", "cmake", "ninja", "gradle", "maven"],
    ),
    ("c_cpp_tools", &["gcc", "g++", "clang", "gdb", "lldb"]),
    ("java_jvm_tools", &["java", "javac", "kotlin"]),
    ("node_js_tools", &["node", "deno", "bun", "npm", "yarn"]),
    ("go_tools", &["go", "gofmt"]),
    ("editors_dev", &["vim", "nvim", "emacs", "code", "zed"]),
    (
        "search_productivity",
        &["rg", "fd", "fzf", "jq", "bat", "tree", "exa"],
    ),
    ("system_perf", &["htop", "ps", "top", "df", "du"]),
    ("containers", &["docker", "podman", "kubectl", "helm"]),
    ("networking", &["curl", "wget", "dig", "traceroute"]),
    ("security", &["openssl", "gpg", "ssh-keygen"]),
    ("databases", &["sqlite3", "psql", "mysql", "redis-cli"]),
    ("vcs", &["git", "gh"]),
];

// ============================================================================
// Shell Detector Tool
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ShellDetectorInput {}

#[derive(Debug, Serialize)]
struct ShellInfo {
    name: String,
    path: String,
    exists: bool,
}

// ============================================================================
// Main Server Implementation
// ============================================================================

#[derive(Clone)]
pub struct EnhancedTerminalServer {
    tool_router: ToolRouter<Self>,
    jobs: Arc<Mutex<HashMap<String, JobRecord>>>,
}

#[tool_router]
impl EnhancedTerminalServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[tool(description = "Execute shell commands with output capture and timeout")]
    async fn enhanced_terminal(
        &self,
        Parameters(input): Parameters<EnhancedTerminalInput>,
    ) -> Result<CallToolResult, McpError> {
        let job_id = new_job_id();
        let command = input.command.trim();

        if command.is_empty() {
            return Err(McpError::invalid_params(
                "Command cannot be empty",
                None::<serde_json::Value>,
            ));
        }

        // Resolve working directory
        let cwd = if input.cwd == "." || input.cwd.is_empty() {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        } else {
            std::path::PathBuf::from(&input.cwd)
        };

        // Create PTY system
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| McpError::internal_error(format!("Failed to open PTY: {}", e), None))?;

        // Build command
        let mut cmd = CommandBuilder::new(&input.shell);
        cmd.arg("-c");
        cmd.arg(command);
        cmd.cwd(&cwd);

        // Start the process
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| McpError::internal_error(format!("Failed to spawn: {}", e), None))?;

        let pid = child.process_id();
        drop(pair.slave);

        // Record job start
        let mut jobs = self.jobs.lock().unwrap();
        jobs.insert(
            job_id.clone(),
            JobRecord {
                command: command.to_string(),
                started_at: SystemTime::now(),
                finished_at: None,
                exit_code: None,
                success: false,
                output: String::new(),
                full_output: String::new(),
                truncated: false,
                canceled: false,
                pid,
            },
        );
        drop(jobs);

        // Read output with timeout
        let mut reader = pair.master.try_clone_reader().map_err(|e| {
            McpError::internal_error(format!("Failed to clone reader: {}", e), None)
        })?;

        let output_limit = input.output_limit;
        let timeout = Duration::from_secs(input.timeout_secs);
        let start_time = std::time::Instant::now();

        let mut output = Vec::new();
        let mut buffer = [0u8; 4096];
        let mut truncated = false;

        loop {
            if start_time.elapsed() > timeout {
                // Timeout - kill process
                let _ = child.kill();
                break;
            }

            match reader.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if output.len() + n <= output_limit {
                        output.extend_from_slice(&buffer[..n]);
                    } else {
                        let remaining = output_limit.saturating_sub(output.len());
                        output.extend_from_slice(&buffer[..remaining]);
                        truncated = true;
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(_) => break,
            }
        }

        // Wait for exit
        let exit_status = child.wait().ok();
        let exit_code = exit_status.map(|s| s.exit_code() as i32);
        let success = exit_code.map(|c| c == 0).unwrap_or(false);

        let output_str = String::from_utf8_lossy(&output).to_string();

        // Update job record
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(&job_id) {
            job.finished_at = Some(SystemTime::now());
            job.exit_code = exit_code;
            job.success = success;
            job.output = output_str.clone();
            job.full_output = output_str.clone();
            job.truncated = truncated;
        }

        let mut result_text = format!("Command: {}\n", command);
        result_text.push_str(&format!("Working Directory: {}\n", cwd.display()));
        result_text.push_str(&format!("Exit Code: {:?}\n", exit_code));
        result_text.push_str(&format!("Success: {}\n\n", success));
        result_text.push_str("Output:\n");
        result_text.push_str(&output_str);

        if truncated {
            result_text.push_str("\n\n[Output truncated]");
        }

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(description = "Detect available developer binaries and their versions")]
    async fn detect_binaries(
        &self,
        Parameters(input): Parameters<DetectBinariesInput>,
    ) -> Result<CallToolResult, McpError> {
        let filter_set: Option<BTreeSet<String>> = input
            .filter_categories
            .as_ref()
            .map(|v| v.iter().map(|s| s.to_lowercase()).collect());

        let mut tasks: Vec<(String, String)> = Vec::new();

        for (category, binaries) in BASE_CANDIDATE_GROUPS {
            if let Some(ref filter) = filter_set {
                if !filter.contains(&category.to_lowercase()) {
                    continue;
                }
            }

            for binary in *binaries {
                tasks.push((category.to_string(), binary.to_string()));
            }
        }

        let max_conc = input.max_concurrency.max(1);
        let timeout_ms = input.version_timeout_ms;
        let shared_results: Arc<Mutex<Vec<BinaryReport>>> = Arc::new(Mutex::new(Vec::new()));

        // Process in chunks
        for chunk in tasks.chunks(max_conc) {
            let mut handles = Vec::new();

            for (category, binary) in chunk.iter().cloned() {
                let results = Arc::clone(&shared_results);
                let handle = thread::spawn(move || {
                    let paths = which_all(&binary);

                    if paths.is_empty() {
                        if let Ok(mut vec) = results.lock() {
                            vec.push(BinaryReport {
                                name: binary,
                                category,
                                found: false,
                                path: None,
                                version: None,
                                error: None,
                            });
                        }
                        return;
                    }

                    let path_field = if paths.len() > 1 {
                        Some(paths.join(";"))
                    } else {
                        Some(paths[0].clone())
                    };

                    let version_result = detect_version(&paths[0], timeout_ms);

                    if let Ok(mut vec) = results.lock() {
                        match version_result {
                            Ok(v) => {
                                vec.push(BinaryReport {
                                    name: binary,
                                    category,
                                    found: true,
                                    path: path_field,
                                    version: Some(v),
                                    error: None,
                                });
                            }
                            Err(e) => {
                                vec.push(BinaryReport {
                                    name: binary,
                                    category,
                                    found: true,
                                    path: path_field,
                                    version: None,
                                    error: Some(e.to_string()),
                                });
                            }
                        }
                    }
                });

                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.join();
            }
        }

        let mut reports = match Arc::try_unwrap(shared_results) {
            Ok(mutex) => mutex.into_inner().unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        reports.sort_by(|a, b| {
            (a.category.as_str(), a.name.as_str()).cmp(&(b.category.as_str(), b.name.as_str()))
        });

        if !input.include_missing {
            reports.retain(|r| r.found);
        }

        let json_output = serde_json::json!({
            "binaries": reports
        });

        let json_text = serde_json::to_string_pretty(&json_output)
            .map_err(|e| McpError::internal_error(format!("Serialization error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json_text)]))
    }

    #[tool(description = "Detect available shells on the system")]
    async fn detect_shells(
        &self,
        Parameters(_input): Parameters<ShellDetectorInput>,
    ) -> Result<CallToolResult, McpError> {
        let common_shells = vec![
            ("/bin/bash", "bash"),
            ("/usr/bin/bash", "bash"),
            ("/bin/zsh", "zsh"),
            ("/usr/bin/zsh", "zsh"),
            ("/bin/fish", "fish"),
            ("/usr/bin/fish", "fish"),
            ("/bin/sh", "sh"),
            ("/usr/bin/sh", "sh"),
            ("/bin/dash", "dash"),
            ("/bin/ksh", "ksh"),
        ];

        let mut shells = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for (path, name) in common_shells {
            if Path::new(path).exists() && !seen_names.contains(name) {
                seen_names.insert(name.to_string());
                shells.push(ShellInfo {
                    name: name.to_string(),
                    path: path.to_string(),
                    exists: true,
                });
            }
        }

        if let Ok(user_shell) = std::env::var("SHELL") {
            if !shells.iter().any(|s| s.path == user_shell) {
                let name = Path::new(&user_shell)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                shells.push(ShellInfo {
                    name,
                    path: user_shell,
                    exists: true,
                });
            }
        }

        let json_output = serde_json::json!({
            "shells": shells,
            "current_shell": std::env::var("SHELL").ok()
        });

        let json_text = serde_json::to_string_pretty(&json_output)
            .map_err(|e| McpError::internal_error(format!("Serialization error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json_text)]))
    }
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for EnhancedTerminalServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Enhanced terminal MCP server with command execution, binary detection, and shell detection tools".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn which_all(name: &str) -> Vec<String> {
    let mut matches = Vec::new();
    let path_var = match env::var_os("PATH") {
        Some(p) => p,
        None => return matches,
    };

    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() && is_executable(&candidate) {
            if let Some(s) = candidate.to_str() {
                matches.push(s.to_string());
            }
        }
    }
    matches
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = p.metadata() {
        let mode = meta.permissions().mode();
        mode & 0o111 != 0
    } else {
        false
    }
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

fn detect_version(path: &str, timeout_ms: u64) -> Result<String> {
    let attempts: &[&[&str]] = &[&["--version"], &["version"], &["-V"]];
    let mut last_err: Option<anyhow::Error> = None;

    for args in attempts {
        match probe_version(path, args, timeout_ms) {
            Ok(line) => return Ok(line),
            Err(e) => {
                last_err = Some(e);
                if last_err
                    .as_ref()
                    .map(|er| er.to_string().contains("timeout"))
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no version retrieved")))
}

fn probe_version(path: &str, args: &[&str], timeout_ms: u64) -> Result<String> {
    let (tx, rx) = mpsc::channel();
    let path_string = path.to_string();
    let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();

    thread::spawn(move || {
        let output = StdCommand::new(&path_string).args(&args_vec).output();
        let result = match output {
            Ok(out) => {
                let text = if !out.stdout.is_empty() {
                    String::from_utf8_lossy(&out.stdout).to_string()
                } else {
                    String::from_utf8_lossy(&out.stderr).to_string()
                };
                let first_line = text.lines().next().unwrap_or("").trim();
                if first_line.is_empty() {
                    Err(anyhow::anyhow!("empty version output"))
                } else {
                    Ok(first_line.to_string())
                }
            }
            Err(e) => Err(anyhow::anyhow!("spawn failed: {}", e)),
        };
        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(r) => r,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(anyhow::anyhow!(
            "version probe timeout after {}ms",
            timeout_ms
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(anyhow::anyhow!("version probe worker disconnected"))
        }
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let server = EnhancedTerminalServer::new();
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
