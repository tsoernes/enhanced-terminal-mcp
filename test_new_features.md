# Test Plan for Job Management Enhancements

## Feature 2.1: Job Log & History Metadata

### New Fields Added to JobRecord
- `tags`: Vec<String> - Optional tags for categorizing jobs
- `summary`: String - First 100 characters of command

### New Features
1. **Tags Support**
   - Add tags when creating jobs via `enhanced_terminal` tool
   - Tags displayed in `job_status` and `job_list`
   
2. **Command Summary**
   - Automatically generated (first 100 chars of command)
   - Displayed in job listings for quick reference

3. **Enhanced Job Filtering in `job_list`**
   - Filter by status (e.g., ["Running", "Completed"])
   - Filter by tag (e.g., "build")
   - Filter by working directory
   - Sort order: "newest" (default) or "oldest"

### Test Cases for 2.1

#### Test 1: Create jobs with tags
```json
{
  "command": "cargo build --release",
  "tags": ["build", "release"]
}
```
Expected: Job created with tags, visible in job_status and job_list

#### Test 2: List jobs with status filter
```json
{
  "max_jobs": 50,
  "status_filter": ["Running", "Completed"]
}
```
Expected: Only shows jobs matching the specified statuses

#### Test 3: List jobs with tag filter
```json
{
  "max_jobs": 50,
  "tag_filter": "build"
}
```
Expected: Only shows jobs with "build" tag

#### Test 4: List jobs with cwd filter
```json
{
  "max_jobs": 50,
  "cwd_filter": "/home/user/project"
}
```
Expected: Only shows jobs from that directory

#### Test 5: Sort order (oldest first)
```json
{
  "max_jobs": 50,
  "sort_order": "oldest"
}
```
Expected: Jobs listed from oldest to newest

#### Test 6: Combined filters
```json
{
  "max_jobs": 50,
  "status_filter": ["Completed"],
  "tag_filter": "test",
  "sort_order": "newest"
}
```
Expected: Shows completed jobs with "test" tag, newest first

## Feature 2.2: Job Output Pagination

### New Parameters in `job_status`
- `offset`: usize - Starting byte position (default: 0)
- `limit`: usize - Maximum bytes to return (default: 0 = all)

### Behavior
- When offset > 0 or limit > 0, pagination mode is activated
- Returns specific byte range of output
- Returns `has_more` flag indicating if more data available
- Returns `total_length` for overall output size
- Allows seeking into very long logs
- Can re-read specific segments

### Test Cases for 2.2

#### Test 1: Basic pagination - first chunk
Create a job with long output first:
```bash
enhanced_terminal: seq 1 1000
```

Then paginate:
```json
{
  "job_id": "job-X",
  "offset": 0,
  "limit": 500
}
```
Expected: Returns first 500 bytes, has_more=true, total_length shown

#### Test 2: Pagination - middle chunk
```json
{
  "job_id": "job-X",
  "offset": 500,
  "limit": 500
}
```
Expected: Returns bytes 500-1000, has_more depends on remaining data

#### Test 3: Pagination - last chunk
```json
{
  "job_id": "job-X",
  "offset": 1000,
  "limit": 500
}
```
Expected: Returns remaining bytes, has_more=false

#### Test 4: Pagination with limit=0 (all remaining)
```json
{
  "job_id": "job-X",
  "offset": 500,
  "limit": 0
}
```
Expected: Returns all bytes from offset 500 to end

#### Test 5: Pagination beyond end
```json
{
  "job_id": "job-X",
  "offset": 999999,
  "limit": 100
}
```
Expected: Returns empty output, has_more=false

#### Test 6: Three modes don't conflict
- Mode 1: `incremental=true` (default)
- Mode 2: `incremental=false` (full)
- Mode 3: `offset > 0 or limit > 0` (pagination)

Expected: Each mode works independently

## Integration Tests

### Test 7: Workflow - Tag and filter
1. Create multiple jobs with different tags
2. Use job_list with tag_filter to find specific jobs
3. Use job_status with pagination to read large outputs

### Test 8: Workflow - Long-running job monitoring
1. Start long-running job (e.g., `sleep 100`)
2. Use incremental mode to poll while running
3. Once complete, use pagination to review full logs

### Test 9: Workflow - Build pipeline
1. Run multiple build steps with tags: ["build"], ["test"], ["deploy"]
2. Use filters to review each stage
3. Check summaries in job_list
4. Dive into details with job_status

## Expected Output Examples

### job_status with tags
```
Job ID: job-123
Command: cargo build --release
Summary: cargo build --release
Shell: bash
Working Directory: /home/user/project
Status: Completed
Tags: build, release
Duration: 45.23s
Exit Code: 0
PID: 12345
Output Mode: Full

Output:
[command output here]
```

### job_list with filtering
```
Found 3 job(s):

Job ID: job-123
  Summary: cargo build --release
  Status: Completed
  CWD: /home/user/project
  Shell: bash
  Tags: build, release
  Exit Code: 0
  Duration: 45.23s
  Output Preview: Compiling project v1.0.0...

[more jobs...]
```

### job_status with pagination
```
Job ID: job-456
Command: seq 1 10000
Summary: seq 1 10000
Shell: bash
Working Directory: /home/user/project
Status: Completed
Duration: 0.12s
Exit Code: 0
PID: 67890
Output Mode: Paginated (offset: 0, limit: 1000)
Total Output Length: 48894 bytes
Has More: true

Output:
1
2
3
[... 1000 bytes total ...]

[More output available. Next offset: 1000]
```

## API Documentation Updates

### enhanced_terminal
New parameter:
- `tags` (array, default: []): Optional tags for categorizing jobs

### job_status
New parameters:
- `offset` (number, default: 0): Starting byte position for pagination
- `limit` (number, default: 0): Maximum bytes to return (0 = all)

New output fields (pagination mode):
- `has_more`: Boolean indicating if more data available
- `total_length`: Total output size in bytes

### job_list
New parameters:
- `status_filter` (array, optional): Filter by status
- `tag_filter` (string, optional): Filter by tag
- `cwd_filter` (string, optional): Filter by working directory
- `sort_order` (string, default: "newest"): Sort order

Enhanced output fields:
- `summary`: Command summary
- `tags`: Job tags
- `cwd`: Working directory
- `shell`: Shell used

## Backwards Compatibility

All new parameters are optional with sensible defaults:
- Existing calls without tags still work
- Existing job_status calls default to incremental mode
- Existing job_list calls show all jobs
- No breaking changes to existing APIs