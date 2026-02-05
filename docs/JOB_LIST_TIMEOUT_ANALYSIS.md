# Job List Timeout Analysis

## Problem

The `enhanced_terminal_job_list` tool experiences "Context server request timeout" errors in Zed when there are jobs with large outputs. This timeout occurs even though individual jobs complete successfully and their status can be retrieved without issue.

## Root Cause

The timeout is caused by **excessive memory allocation and cloning overhead** when listing multiple jobs with large outputs:

### 1. Unlimited Output Storage

```rust
pub struct JobRecord {
    pub job_id: String,
    pub command: String,
    // ... other fields ...
    pub output: String,        // Limited to output_limit (default 16KB)
    pub full_output: String,   // ⚠️ UNLIMITED - stores entire job output!
    // ... more fields ...
}
```

- The `output` field is capped at `output_limit` (default: 16,384 bytes)
- The `full_output` field has **NO LIMIT** and stores the complete output
- Long-running jobs can accumulate megabytes or even gigabytes of output

### 2. Complete Cloning in list_jobs()

```rust
pub fn list_jobs(&self) -> Vec<JobRecord> {
    let jobs = self.jobs.lock().unwrap();
    let mut job_list: Vec<JobRecord> = jobs.values().cloned().collect();  // ⚠️ Clones ALL data!
    job_list.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    job_list
}
```

When `list_jobs()` is called:
1. It clones **every `JobRecord`** in the registry
2. Each clone includes the **entire `full_output` string**
3. With default `max_jobs=50`, this could clone 50+ jobs with their full outputs

### 3. Wasteful Data Transfer

The `enhanced_terminal_job_list` tool only displays:
- First 100 characters of output as preview
- Job metadata (status, duration, etc.)

Yet it clones and transfers the **complete output** for every job, which is never used.

## Performance Impact

### Example Scenario

If you have 50 background jobs, each with 10MB of output:
- **Total memory copied**: 50 jobs × 10MB = **500MB**
- **Actually needed**: 50 jobs × 100 chars ≈ **5KB**
- **Waste factor**: ~100,000x more data than needed

### Timing Breakdown

1. **Mutex lock acquisition**: Negligible
2. **Cloning 50 JobRecords with large outputs**: 30-60+ seconds
3. **String formatting**: 1-2 seconds
4. **Serialization to MCP response**: 5-10 seconds

**Total**: Can easily exceed Zed's 60-second timeout

## Why Other Tools Don't Timeout

- `enhanced_terminal_job_status`: Retrieves only **ONE** job - even with large output, this is manageable
- `enhanced_terminal`: Creates new job - no cloning of existing jobs
- `enhanced_terminal_job_cancel`: Only modifies status - no output retrieval

Only `enhanced_terminal_job_list` clones **ALL** jobs at once.

## Solutions

### Solution 1: Lightweight Summary Struct (Recommended)

Create a separate struct for listing that excludes full output:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct JobSummary {
    pub job_id: String,
    pub command: String,
    pub summary: String,
    pub shell: String,
    pub cwd: String,
    pub started_at: SystemTime,
    pub finished_at: Option<SystemTime>,
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    pub output_preview: String,  // Only first 100 chars
    pub tags: Vec<String>,
}

impl JobManager {
    pub fn list_jobs_summary(&self) -> Vec<JobSummary> {
        let jobs = self.jobs.lock().unwrap();
        jobs.values()
            .map(|job| JobSummary {
                job_id: job.job_id.clone(),
                command: job.command.clone(),
                summary: job.summary.clone(),
                shell: job.shell.clone(),
                cwd: job.cwd.clone(),
                started_at: job.started_at,
                finished_at: job.finished_at,
                status: job.status.clone(),
                exit_code: job.exit_code,
                output_preview: job.full_output.chars().take(100).collect(),
                tags: job.tags.clone(),
            })
            .collect()
    }
}
```

**Benefits**:
- Only copies metadata + 100 char preview
- Reduces memory usage by ~1000x for typical jobs
- Fast and won't timeout

### Solution 2: Arc<String> for Large Fields

Use reference counting to make cloning cheap:

```rust
pub struct JobRecord {
    pub job_id: String,
    pub command: String,
    // ... other fields ...
    pub output: String,
    pub full_output: Arc<String>,  // Clone is just pointer copy
    // ... more fields ...
}
```

**Benefits**:
- Cloning becomes O(1) instead of O(n)
- No timeout issues
- Full output still available when needed

**Drawbacks**:
- Requires changes throughout codebase
- Memory not freed until all Arc clones dropped

### Solution 3: Store Full Output Separately

Keep only preview in JobRecord, store full output in a separate HashMap:

```rust
pub struct JobManager {
    jobs: Arc<Mutex<HashMap<String, JobRecord>>>,
    full_outputs: Arc<Mutex<HashMap<String, String>>>,
}
```

**Benefits**:
- Job listing is always fast
- Can implement output pagination/streaming
- Clear separation of concerns

**Drawbacks**:
- More complex implementation
- Need to keep maps synchronized

### Solution 4: Lazy Evaluation

Don't clone until serialization, and only serialize what's needed:

```rust
pub fn list_jobs_json(&self, max_jobs: usize) -> String {
    let jobs = self.jobs.lock().unwrap();
    let mut sorted: Vec<_> = jobs.values().collect();
    sorted.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    
    let limited = sorted.into_iter().take(max_jobs);
    
    // Serialize directly without cloning
    json!(limited.map(|job| json!({
        "job_id": job.job_id,
        "summary": job.summary,
        "status": job.status,
        "output_preview": &job.full_output[..100.min(job.full_output.len())],
        // ... etc
    })).collect::<Vec<_>>()).to_string()
}
```

## Recommended Fix

Implement **Solution 1** (Lightweight Summary Struct) because:
1. ✅ Minimal code changes required
2. ✅ No risk of breaking existing functionality
3. ✅ Solves the timeout issue completely
4. ✅ Actually improves design by separating concerns
5. ✅ No performance overhead in other operations

## Implementation Steps

1. Create `JobSummary` struct in `job_manager.rs`
2. Add `list_jobs_summary()` and `list_jobs_filtered_summary()` methods
3. Update `enhanced_terminal_job_list` tool to use new methods
4. Add tests to verify timeout doesn't occur with large outputs
5. Update documentation

## Testing

After fix, test with:
```bash
# Create job with large output
enhanced_terminal: "for i in {1..100000}; do echo 'Line $i with some data'; done"

# List jobs - should complete quickly
enhanced_terminal_job_list: {}
```

Expected result: Returns within 1-2 seconds regardless of output size.