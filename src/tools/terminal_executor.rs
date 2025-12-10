use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct JobRecord {
    pub command: String,
    pub started_at: SystemTime,
    pub finished_at: Option<SystemTime>,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub output: String,
    pub full_output: String,
    pub truncated: bool,
    pub canceled: bool,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalExecutionInput {
    /// Command to execute
    pub command: String,
    /// Working directory (default: ".")
    #[serde(default = "default_cwd")]
    pub cwd: String,
    /// Shell to use (default: "sh")
    #[serde(default = "default_shell")]
    pub shell: String,
    /// Output limit in bytes (default: 16384)
    #[serde(default = "default_output_limit")]
    pub output_limit: usize,
    /// Timeout in seconds (default: 60)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
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

#[derive(Debug, Serialize)]
pub struct ExecutionResult {
    pub command: String,
    pub working_directory: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub output: String,
    pub truncated: bool,
    pub timed_out: bool,
}

pub fn execute_command(input: TerminalExecutionInput) -> Result<ExecutionResult> {
    let command = input.command.trim();

    if command.is_empty() {
        return Err(anyhow::anyhow!("Command cannot be empty"));
    }

    // Resolve working directory
    let cwd = if input.cwd == "." || input.cwd.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(&input.cwd)
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
        .map_err(|e| anyhow::anyhow!("Failed to open PTY: {}", e))?;

    // Build command
    let mut cmd = CommandBuilder::new(&input.shell);
    cmd.arg("-c");
    cmd.arg(command);
    cmd.cwd(&cwd);

    // Start the process
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| anyhow::anyhow!("Failed to spawn: {}", e))?;

    drop(pair.slave);

    // Read output with timeout
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| anyhow::anyhow!("Failed to clone reader: {}", e))?;

    let output_limit = input.output_limit;
    let timeout = Duration::from_secs(input.timeout_secs);
    let start_time = std::time::Instant::now();

    let mut output = Vec::new();
    let mut buffer = [0u8; 4096];
    let mut truncated = false;
    let mut timed_out = false;

    loop {
        if start_time.elapsed() > timeout {
            // Timeout - kill process
            let _ = child.kill();
            timed_out = true;
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

    Ok(ExecutionResult {
        command: command.to_string(),
        working_directory: cwd.display().to_string(),
        exit_code,
        success,
        output: output_str,
        truncated,
        timed_out,
    })
}
