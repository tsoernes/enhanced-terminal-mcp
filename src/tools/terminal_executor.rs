use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use super::denylist::{find_matched_pattern, is_denied};
use super::job_manager::{JobManager, JobStatus};

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
    /// Timeout in seconds (default: 300)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Async threshold in seconds - switch to background after this (default: 5)
    #[serde(default = "default_async_threshold")]
    pub async_threshold_secs: u64,
    /// Force synchronous execution (wait for completion)
    #[serde(default)]
    pub force_sync: bool,
    /// Custom denylist patterns (in addition to defaults)
    #[serde(default)]
    pub custom_denylist: Vec<String>,
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
    300
}

fn default_async_threshold() -> u64 {
    5
}

#[derive(Debug, Serialize)]
pub struct ExecutionResult {
    pub job_id: String,
    pub command: String,
    pub working_directory: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub output: String,
    pub truncated: bool,
    pub timed_out: bool,
    pub switched_to_async: bool,
    pub denied: bool,
    pub denial_reason: Option<String>,
}

pub fn execute_command(
    input: TerminalExecutionInput,
    job_manager: &JobManager,
) -> Result<ExecutionResult> {
    let command = input.command.trim();

    if command.is_empty() {
        return Err(anyhow::anyhow!("Command cannot be empty"));
    }

    // Check denylist
    if is_denied(command, &input.custom_denylist) {
        let matched_pattern = find_matched_pattern(command, &input.custom_denylist);
        return Ok(ExecutionResult {
            job_id: String::new(),
            command: command.to_string(),
            working_directory: String::new(),
            exit_code: None,
            success: false,
            output: String::new(),
            truncated: false,
            timed_out: false,
            switched_to_async: false,
            denied: true,
            denial_reason: Some(format!(
                "Command denied by security policy. Matched pattern: {}",
                matched_pattern.unwrap_or_else(|| "unknown".to_string())
            )),
        });
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

    let pid = child.process_id();
    drop(pair.slave);

    // Register job
    let job_id = job_manager.new_job_id();
    job_manager.register_job(
        job_id.clone(),
        command.to_string(),
        input.shell.clone(),
        cwd.display().to_string(),
        pid,
    );

    // Read output with smart async switching
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| anyhow::anyhow!("Failed to clone reader: {}", e))?;

    let output_limit = input.output_limit;
    let timeout = Duration::from_secs(input.timeout_secs);
    let async_threshold = Duration::from_secs(input.async_threshold_secs);
    let start_time = Instant::now();

    let mut output = Vec::new();
    let mut buffer = [0u8; 4096];
    let mut truncated = false;
    let mut timed_out = false;
    let mut switched_to_async = false;

    // Initial synchronous phase
    loop {
        let elapsed = start_time.elapsed();

        // Check if we should switch to async
        if !input.force_sync && elapsed > async_threshold {
            switched_to_async = true;
            break;
        }

        // Check for overall timeout
        if elapsed > timeout {
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
                }

                // Update job with incremental output
                let output_str = String::from_utf8_lossy(&buffer[..n]).to_string();
                job_manager.append_output(&job_id, &output_str, output_limit);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(_) => break,
        }
    }

    if switched_to_async {
        // Spawn background thread to continue monitoring
        let job_manager_clone = job_manager.clone();
        let job_id_clone = job_id.clone();
        let timeout_remaining = timeout.saturating_sub(start_time.elapsed());

        thread::spawn(move || {
            let start_bg = Instant::now();
            let mut bg_buffer = [0u8; 4096];

            loop {
                if start_bg.elapsed() > timeout_remaining {
                    let _ = child.kill();
                    job_manager_clone.complete_job(&job_id_clone, None, JobStatus::TimedOut);
                    break;
                }

                match reader.read(&mut bg_buffer) {
                    Ok(0) => {
                        // Process finished
                        let exit_status = child.wait().ok();
                        let exit_code = exit_status.map(|s| s.exit_code() as i32);
                        let status = if exit_code == Some(0) {
                            JobStatus::Completed
                        } else {
                            JobStatus::Failed
                        };
                        job_manager_clone.complete_job(&job_id_clone, exit_code, status);
                        break;
                    }
                    Ok(n) => {
                        let output_str = String::from_utf8_lossy(&bg_buffer[..n]).to_string();
                        job_manager_clone.append_output(&job_id_clone, &output_str, output_limit);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => {
                        job_manager_clone.complete_job(&job_id_clone, None, JobStatus::Failed);
                        break;
                    }
                }
            }
        });

        // Return with current output
        let output_str = String::from_utf8_lossy(&output).to_string();
        return Ok(ExecutionResult {
            job_id,
            command: command.to_string(),
            working_directory: cwd.display().to_string(),
            exit_code: None,
            success: false,
            output: output_str,
            truncated,
            timed_out: false,
            switched_to_async: true,
            denied: false,
            denial_reason: None,
        });
    }

    // Synchronous completion
    let exit_status = child.wait().ok();
    let exit_code = exit_status.map(|s| s.exit_code() as i32);
    let success = exit_code.map(|c| c == 0).unwrap_or(false);

    let output_str = String::from_utf8_lossy(&output).to_string();

    // Complete job
    let status = if timed_out {
        JobStatus::TimedOut
    } else if success {
        JobStatus::Completed
    } else {
        JobStatus::Failed
    };
    job_manager.complete_job(&job_id, exit_code, status);

    Ok(ExecutionResult {
        job_id,
        command: command.to_string(),
        working_directory: cwd.display().to_string(),
        exit_code,
        success,
        output: output_str,
        truncated,
        timed_out,
        switched_to_async: false,
        denied: false,
        denial_reason: None,
    })
}
