# Enhanced Terminal MCP Server - Manual Testing Guide

## ✅ Installation Verified

- **Binary**: `/home/torstein.sornes/code/enhanced-terminal-mcp/target/release/enhanced-terminal-mcp`
- **Size**: 3.5M
- **Permissions**: Executable
- **Build**: Release mode, optimized
- **Zed Config**: Added to `~/.config/zed/settings.json`

## 🔧 Zed Configuration

```json
{
  "context_servers": {
    "enhanced-terminal": {
      "source": "custom",
      "command": "/home/torstein.sornes/code/enhanced-terminal-mcp/target/release/enhanced-terminal-mcp",
      "args": [],
      "enabled": true
    }
  }
}
```

## 🧪 Testing Instructions

### Step 1: Restart Zed
The MCP server will be loaded when Zed restarts or the configuration is reloaded.

### Step 2: Verify Server is Loaded
In Zed, check the context server status. The "enhanced-terminal" server should appear.

### Step 3: Test Commands

#### Test 1: Simple Command
```
Ask the AI: "Use enhanced_terminal to run 'echo Hello from MCP!' "
```

Expected:
- Command executes quickly
- Returns "Hello from MCP!"
- Provides job_id
- Shows exit code 0

#### Test 2: List Files
```
Ask the AI: "List files in the current directory using enhanced_terminal"
```

Expected:
- Runs `ls -la` or similar
- Shows file listing
- Completes synchronously (< 50s)

#### Test 3: Long-Running Command (Async Switch)
```
Ask the AI: "Run 'sleep 60' using enhanced_terminal"
```

Expected:
- Command starts
- Automatically switches to background after 50s
- Returns job_id immediately
- Status shows "SWITCHED TO BACKGROUND"

#### Test 4: Job Status (Incremental - Default)
```
Ask the AI: "Check the status of job <job_id>"
```

Expected:
- Shows current status
- Returns incremental output (default mode)
- Shows duration
- Indicates if running or completed

#### Test 5: Job Status (Full Output)
```
Ask the AI: "Get full output for job <job_id> with incremental set to false"
```

Expected:
- Shows all output from start
- Resets read position

#### Test 6: Job List
```
Ask the AI: "List all background jobs"
```

Expected:
- Shows recent jobs
- Output previews (100 chars)
- Status for each job

#### Test 7: Detect Binaries
```
Ask the AI: "Detect Python tools using detect_binaries with filter_categories=['python_tools']"
```

Expected:
- Scans for Python tools
- Returns paths and versions
- Completes in ~2-3 seconds (16 concurrent checks)

#### Test 8: Environment Variables
```
Ask the AI: "Run 'echo $MY_VAR' with env_vars={'MY_VAR': 'test123'}"
```

Expected:
- Command uses custom environment
- Outputs "test123"

#### Test 9: Security (Denied Command)
```
Ask the AI: "Try to run 'rm -rf /' using enhanced_terminal"
```

Expected:
- Command is DENIED
- Shows denial reason
- Matches denylist pattern
- No execution occurs

#### Test 10: Job Cancellation
```
Ask the AI: "Start 'sleep 300' then cancel it using job_cancel"
```

Expected:
- Command starts
- Switches to background
- job_cancel sends SIGTERM
- Job status updates to "Canceled"

## 📊 Expected Tool Documentation

When you ask the AI about the enhanced_terminal tool, it should show:
- All parameters with types and defaults
- Available shells list
- Security denylist information  
- Behavior explanations
- Return value descriptions
- Incremental output mode (default: true)

## ✅ Success Criteria

- [ ] Server loads without errors in Zed
- [ ] All 5 tools are available (enhanced_terminal, job_status, job_list, job_cancel, detect_binaries)
- [ ] Commands execute successfully
- [ ] Async switching works (commands > 50s go to background)
- [ ] Incremental output mode works (default)
- [ ] Job management works (status, list, cancel)
- [ ] Security denylist blocks dangerous commands
- [ ] Environment variables work
- [ ] Binary detection finds tools

## 🐛 Troubleshooting

If the server doesn't load:
1. Check Zed logs for errors
2. Verify binary path is correct
3. Ensure binary is executable
4. Try running binary manually to check for library issues: `./target/release/enhanced-terminal-mcp`

If tools don't appear:
1. Restart Zed completely
2. Check `context_servers` section in settings.json
3. Ensure `enabled: true` is set

## 📝 Notes

- Default shell: **bash** (not sh)
- Default async threshold: **50 seconds** (not 5)
- Default timeout: **None** (no timeout)
- Default incremental: **true** (recommended)
- Max concurrency: **16** (for binary detection)
