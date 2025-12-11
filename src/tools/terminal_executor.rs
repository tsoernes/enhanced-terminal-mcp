use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;
use std::time::{Duration, Instant};

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
    /// Timeout in seconds (None/0 = no timeout, default: None)
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Async threshold in seconds - switch to background after this (default: 50)
    #[serde(default = "default_async_threshold")]
    pub async_threshold_secs: u64,
    /// Environment variables to set for the command
    #[serde(default)]
    pub env_vars: std::collections::HashMap<String, String>,
    /// Force synchronous execution (wait for completion)
    #[serde(default)]
    pub force_sync: bool,
    /// Custom denylist patterns (in addition to defaults)
    #[serde(default)]
    pub custom_denylist: Vec<String>,
    /// Optional tags for categorizing jobs (e.g., ["build", "ci"])
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_cwd() -> String {
    ".".to_string()
}

fn default_shell() -> String {
    "bash".to_string()
}

fn default_output_limit() -> usize {
    16 * 1024
}

fn default_async_threshold() -> u64 {
    50
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
    pub duration_secs: Option<f64>,
}

pub fn execute_command(
    input: &TerminalExecutionInput,
    job_manager: &JobManager,
) -> Result<ExecutionResult> {
    let command = input.command.trim();

    tracing::debug!(
        "execute_command called: command={}, async_threshold_secs={}, force_sync={}",
        command,
        input.async_threshold_secs,
        input.force_sync
    );

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
            duration_secs: None,
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

    // Set environment variables
    for (key, value) in &input.env_vars {
        cmd.env(key, value);
    }

    // Start the process
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| anyhow::anyhow!("Failed to spawn: {}", e))?;

    let pid = child.process_id();
    drop(pair.slave);

    // Register job
    let job_id = job_manager.new_job_id();
    job_manager.register_job_with_tags(
        job_id.clone(),
        command.to_string(),
        input.shell.clone(),
        cwd.display().to_string(),
        pid,
        input.tags.clone(),
    );

    // Read output with smart async switching
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| anyhow::anyhow!("Failed to clone reader: {}", e))?;

    let output_limit = input.output_limit;
    let timeout = input.timeout_secs.map(Duration::from_secs);
    let async_threshold = Duration::from_secs(input.async_threshold_secs);
    let start_time = Instant::now();

    let mut output = Vec::new();
    let mut truncated = false;
    let mut timed_out = false;
    let mut switched_to_async = false;

    // Channel for receiving output from reader thread
    enum ReadMsg {
        Data(Vec<u8>),
        Eof,
        Error,
    }

    let (tx, rx): (std::sync::mpsc::Sender<ReadMsg>, Receiver<ReadMsg>) = channel();

    // Spawn reader thread
    let reader_job_id = job_id.clone();
    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    tracing::debug!("Reader thread: EOF, job_id={}", reader_job_id);
                    let _ = tx.send(ReadMsg::Eof);
                    break;
                }
                Ok(n) => {
                    let data = buffer[..n].to_vec();
                    if tx.send(ReadMsg::Data(data)).is_err() {
                        break; // Main thread dropped receiver
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => {
                    tracing::warn!("Reader thread error: {:?}, job_id={}", e, reader_job_id);
                    let _ = tx.send(ReadMsg::Error);
                    break;
                }
            }
        }
    });

    // Main loop: check elapsed time and receive output
    loop {
        let elapsed = start_time.elapsed();

        // Check if we should switch to async
        if !input.force_sync && elapsed > async_threshold {
            tracing::debug!(
                "Main thread: async threshold reached at {:.2}s, job_id={}",
                elapsed.as_secs_f64(),
                job_id
            );
            switched_to_async = true;
            break;
        }

        // Check for overall timeout (if set)
        if let Some(timeout_duration) = timeout {
            if elapsed > timeout_duration {
                tracing::debug!("Main thread: timeout reached, job_id={}", job_id);
                let _ = child.kill();
                timed_out = true;
                break;
            }
        }

        // Try to receive output from reader thread (non-blocking)
        match rx.try_recv() {
            Ok(ReadMsg::Data(data)) => {
                if output.len() + data.len() <= output_limit {
                    output.extend_from_slice(&data);
                } else {
                    let remaining = output_limit.saturating_sub(output.len());
                    output.extend_from_slice(&data[..remaining]);
                    truncated = true;
                }

                // Update job with incremental output
                let output_str = String::from_utf8_lossy(&data).to_string();
                job_manager.append_output(&job_id, &output_str, output_limit);
            }
            Ok(ReadMsg::Eof) => {
                tracing::debug!("Main thread: EOF received, job_id={}", job_id);
                break;
            }
            Ok(ReadMsg::Error) => {
                tracing::warn!("Main thread: read error received, job_id={}", job_id);
                break;
            }
            Err(TryRecvError::Empty) => {
                // No data available, sleep briefly and check elapsed time again
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(TryRecvError::Disconnected) => {
                tracing::warn!("Main thread: reader thread disconnected, job_id={}", job_id);
                break;
            }
        }
    }

    if switched_to_async {
        tracing::info!(
            "Command switched to async mode: job_id={}, elapsed={:.2}s, output_so_far={} bytes",
            job_id,
            start_time.elapsed().as_secs_f64(),
            output.len()
        );

        // Spawn background thread to continue monitoring
        let job_manager_clone = job_manager.clone();
        let job_id_clone = job_id.clone();
        let timeout_remaining = timeout.map(|t| t.saturating_sub(start_time.elapsed()));

        thread::spawn(move || {
            tracing::debug!("Background thread started for job_id={}", job_id_clone);
            let start_bg = Instant::now();

            // Continue receiving from the reader thread channel
            loop {
                // Check for timeout (if set)
                if let Some(timeout_dur) = timeout_remaining {
                    if start_bg.elapsed() > timeout_dur {
                        let _ = child.kill();
                        job_manager_clone.complete_job(&job_id_clone, None, JobStatus::TimedOut);
                        break;
                    }
                }

                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(ReadMsg::Data(data)) => {
                        let output_str = String::from_utf8_lossy(&data).to_string();
                        job_manager_clone.append_output(&job_id_clone, &output_str, output_limit);
                    }
                    Ok(ReadMsg::Eof) => {
                        // Process finished
                        let exit_status = child.wait().ok();
                        let exit_code = exit_status.map(|s| s.exit_code() as i32);
                        let status = if exit_code == Some(0) {
                            JobStatus::Completed
                        } else {
                            JobStatus::Failed
                        };
                        tracing::debug!(
                            "Background job completed: job_id={}, exit_code={:?}, status={:?}",
                            job_id_clone,
                            exit_code,
                            status
                        );
                        job_manager_clone.complete_job(&job_id_clone, exit_code, status);
                        break;
                    }
                    Ok(ReadMsg::Error) => {
                        job_manager_clone.complete_job(&job_id_clone, None, JobStatus::Failed);
                        break;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // No data, continue loop to check timeout
                        continue;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        // Reader thread died unexpectedly
                        job_manager_clone.complete_job(&job_id_clone, None, JobStatus::Failed);
                        break;
                    }
                }
            }
        });

        // Return immediately with current output and duration so far
        let output_str = String::from_utf8_lossy(&output).to_string();
        let duration_secs = start_time.elapsed().as_secs_f64();
        tracing::info!(
            "Returning async result: job_id={}, duration={:.2}s",
            job_id,
            duration_secs
        );
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
            duration_secs: Some(duration_secs),
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

    let duration_secs = start_time.elapsed().as_secs_f64();

    tracing::debug!(
        "Synchronous command completed: job_id={}, exit_code={:?}, duration={:.2}s",
        job_id,
        exit_code,
        duration_secs
    );

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
        duration_secs: Some(duration_secs),
    })
}
