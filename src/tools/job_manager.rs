use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Job status for background command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Running,
    Completed,
    Failed,
    TimedOut,
    Canceled,
}

/// Record of a background job
#[derive(Debug, Clone, Serialize)]
pub struct JobRecord {
    pub job_id: String,
    pub command: String,
    pub shell: String,
    pub cwd: String,
    pub started_at: SystemTime,
    pub finished_at: Option<SystemTime>,
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    pub output: String,
    pub full_output: String,
    pub truncated: bool,
    pub pid: Option<u32>,
    pub last_read_position: usize,
}

/// Global job registry
pub struct JobManager {
    jobs: Arc<Mutex<HashMap<String, JobRecord>>>,
    job_counter: Arc<Mutex<u64>>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            job_counter: Arc::new(Mutex::new(1)),
        }
    }

    /// Generate a new unique job ID
    pub fn new_job_id(&self) -> String {
        let mut counter = self.job_counter.lock().unwrap();
        let id = *counter;
        *counter += 1;
        format!("job-{}", id)
    }

    /// Register a new job
    pub fn register_job(
        &self,
        job_id: String,
        command: String,
        shell: String,
        cwd: String,
        pid: Option<u32>,
    ) {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.insert(
            job_id.clone(),
            JobRecord {
                job_id,
                command,
                shell,
                cwd,
                started_at: SystemTime::now(),
                finished_at: None,
                status: JobStatus::Running,
                exit_code: None,
                output: String::new(),
                full_output: String::new(),
                truncated: false,
                pid,
                last_read_position: 0,
            },
        );
    }

    /// Update job with output (incremental)
    pub fn append_output(&self, job_id: &str, output: &str, output_limit: usize) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(job_id) {
            job.full_output.push_str(output);

            if job.output.len() + output.len() <= output_limit {
                job.output.push_str(output);
            } else {
                let remaining = output_limit.saturating_sub(job.output.len());
                if remaining > 0 {
                    job.output.push_str(&output[..remaining.min(output.len())]);
                }
                job.truncated = true;
            }
        }
    }

    /// Complete a job
    pub fn complete_job(&self, job_id: &str, exit_code: Option<i32>, status: JobStatus) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(job_id) {
            job.finished_at = Some(SystemTime::now());
            job.exit_code = exit_code;
            job.status = status;
        }
    }

    /// Get a job by ID
    pub fn get_job(&self, job_id: &str) -> Option<JobRecord> {
        let jobs = self.jobs.lock().unwrap();
        jobs.get(job_id).cloned()
    }

    /// Get incremental output (only new since last read)
    pub fn get_incremental_output(&self, job_id: &str) -> Option<(String, bool)> {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(job_id) {
            let new_output = job.full_output[job.last_read_position..].to_string();
            job.last_read_position = job.full_output.len();
            Some((new_output, matches!(job.status, JobStatus::Running)))
        } else {
            None
        }
    }

    /// Reset read position to get all output again
    pub fn reset_read_position(&self, job_id: &str) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(job_id) {
            job.last_read_position = 0;
        }
    }

    /// List all jobs
    pub fn list_jobs(&self) -> Vec<JobRecord> {
        let jobs = self.jobs.lock().unwrap();
        let mut job_list: Vec<JobRecord> = jobs.values().cloned().collect();
        job_list.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        job_list
    }

    /// Cancel a running job (Unix only)
    #[cfg(unix)]
    pub fn cancel_job(&self, job_id: &str) -> Result<()> {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;

        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(job_id) {
            if matches!(job.status, JobStatus::Running) {
                if let Some(pid) = job.pid {
                    let pid = Pid::from_raw(pid as i32);
                    kill(pid, Signal::SIGTERM)?;
                    job.status = JobStatus::Canceled;
                    job.finished_at = Some(SystemTime::now());
                    return Ok(());
                }
            }
        }
        Err(anyhow::anyhow!("Job not found or not running"))
    }

    #[cfg(not(unix))]
    pub fn cancel_job(&self, job_id: &str) -> Result<()> {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(job_id) {
            if matches!(job.status, JobStatus::Running) {
                job.status = JobStatus::Canceled;
                job.finished_at = Some(SystemTime::now());
                return Ok(());
            }
        }
        Err(anyhow::anyhow!("Job not found or not running"))
    }

    /// Delete a job from history
    pub fn delete_job(&self, job_id: &str) -> Result<()> {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.remove(job_id)
            .ok_or_else(|| anyhow::anyhow!("Job not found"))?;
        Ok(())
    }

    /// Get clone of jobs Arc for sharing
    pub fn jobs_arc(&self) -> Arc<Mutex<HashMap<String, JobRecord>>> {
        Arc::clone(&self.jobs)
    }
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for JobManager {
    fn clone(&self) -> Self {
        Self {
            jobs: Arc::clone(&self.jobs),
            job_counter: Arc::clone(&self.job_counter),
        }
    }
}
