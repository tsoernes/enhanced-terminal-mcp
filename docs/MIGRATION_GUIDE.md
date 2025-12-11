# Migration Guide: Tool Renaming

## Overview

In the latest version, job management tools have been renamed to include the `enhanced_terminal_` prefix for better namespacing and to prevent conflicts with other MCP servers.

## What Changed

### Tool Name Changes

| Old Name | New Name | Status |
|----------|----------|--------|
| `job_status` | `enhanced_terminal_job_status` | ⚠️ RENAMED |
| `job_list` | `enhanced_terminal_job_list` | ⚠️ RENAMED |
| `job_cancel` | `enhanced_terminal_job_cancel` | ⚠️ RENAMED |
| `enhanced_terminal` | `enhanced_terminal` | ✅ UNCHANGED |
| `detect_binaries` | `detect_binaries` | ✅ UNCHANGED |

### Why This Change?

1. **Clear Namespacing**: The `enhanced_terminal_` prefix makes it immediately obvious which MCP server provides these tools
2. **Conflict Prevention**: Prevents naming collisions with other MCP servers that might have generic `job_*` tools
3. **Better Organization**: Groups related functionality together under a common prefix
4. **Improved Discoverability**: Makes it easier to find all tools from this server

## How to Migrate

### For MCP Clients

Update all tool invocations to use the new prefixed names:

#### Before:
```json
{
  "tool": "job_status",
  "arguments": {
    "job_id": "job-123"
  }
}
```

#### After:
```json
{
  "tool": "enhanced_terminal_job_status",
  "arguments": {
    "job_id": "job-123"
  }
}
```

### For Scripts and Automation

If you have scripts that invoke these tools via MCP clients, update the tool names:

**Python Example:**
```python
# Before
client.call_tool("job_status", {"job_id": "job-123"})
client.call_tool("job_list", {"max_jobs": 50})
client.call_tool("job_cancel", {"job_id": "job-123"})

# After
client.call_tool("enhanced_terminal_job_status", {"job_id": "job-123"})
client.call_tool("enhanced_terminal_job_list", {"max_jobs": 50})
client.call_tool("enhanced_terminal_job_cancel", {"job_id": "job-123"})
```

**JavaScript Example:**
```javascript
// Before
await client.callTool("job_status", { job_id: "job-123" });
await client.callTool("job_list", { max_jobs: 50 });
await client.callTool("job_cancel", { job_id: "job-123" });

// After
await client.callTool("enhanced_terminal_job_status", { job_id: "job-123" });
await client.callTool("enhanced_terminal_job_list", { max_jobs: 50 });
await client.callTool("enhanced_terminal_job_cancel", { job_id: "job-123" });
```

### For Documentation

Update any references in your documentation:

- Replace `job_status` with `enhanced_terminal_job_status`
- Replace `job_list` with `enhanced_terminal_job_list`
- Replace `job_cancel` with `enhanced_terminal_job_cancel`

## What Stays the Same

### All Functionality Remains Identical

- ✅ All parameters remain unchanged
- ✅ All return values remain unchanged
- ✅ All behavior remains unchanged
- ✅ Only the tool names have changed

### Main Tools Unchanged

- ✅ `enhanced_terminal` - Still the same name
- ✅ `detect_binaries` - Still the same name

### Example: No Parameter Changes

The `enhanced_terminal_job_status` tool works exactly the same as before:

```json
{
  "tool": "enhanced_terminal_job_status",
  "arguments": {
    "job_id": "job-123",
    "incremental": true,
    "offset": 0,
    "limit": 1000
  }
}
```

All parameters, behavior, and return values are identical to the old `job_status` tool.

## Finding Old Tool Names

Use a simple search and replace in your codebase:

```bash
# Find all references
grep -r "job_status" .
grep -r "job_list" .
grep -r "job_cancel" .

# Or use ripgrep for better results
rg '"job_status"' 
rg '"job_list"'
rg '"job_cancel"'
```

## Backward Compatibility

**There is NO backward compatibility.** The old tool names (`job_status`, `job_list`, `job_cancel`) are no longer available. You must update to the new names.

## Testing Your Migration

After updating your code, test each tool:

### Test enhanced_terminal_job_status
```json
{
  "tool": "enhanced_terminal",
  "arguments": {
    "command": "echo test",
    "tags": ["migration-test"]
  }
}
```

Then check the status with:
```json
{
  "tool": "enhanced_terminal_job_status",
  "arguments": {
    "job_id": "<returned-job-id>"
  }
}
```

### Test enhanced_terminal_job_list
```json
{
  "tool": "enhanced_terminal_job_list",
  "arguments": {
    "max_jobs": 10,
    "tag_filter": "migration-test"
  }
}
```

### Test enhanced_terminal_job_cancel
```json
{
  "tool": "enhanced_terminal",
  "arguments": {
    "command": "sleep 100"
  }
}
```

Then cancel it:
```json
{
  "tool": "enhanced_terminal_job_cancel",
  "arguments": {
    "job_id": "<returned-job-id>"
  }
}
```

## Common Migration Issues

### Issue: "Tool not found" error

**Cause**: Still using old tool names

**Solution**: Update to new prefixed names:
- `job_status` → `enhanced_terminal_job_status`
- `job_list` → `enhanced_terminal_job_list`
- `job_cancel` → `enhanced_terminal_job_cancel`

### Issue: Autocomplete suggesting old names

**Cause**: IDE or client cache

**Solution**: 
1. Restart your MCP client
2. Clear IDE cache
3. Reload tool definitions from server

### Issue: Scripts failing

**Cause**: Hardcoded old tool names

**Solution**: Update all tool name strings in your scripts

## Support

If you encounter issues during migration:

1. Check the [README.md](../README.md) for updated examples
2. Review the [CHANGELOG.md](CHANGELOG.md) for all changes
3. See [BUGFIX_TOOL_NAMING.md](BUGFIX_TOOL_NAMING.md) for technical details
4. Open an issue on GitHub if problems persist

## Timeline

- **Old Names Removed**: Current version
- **New Names Available**: Current version
- **Migration Window**: None - update immediately

## Summary

| Action Required | Difficulty | Time Estimate |
|----------------|------------|---------------|
| Update tool names | Low | 5-10 minutes |
| Test changes | Low | 5 minutes |
| Update documentation | Low | 10 minutes |
| **Total** | **Low** | **20-30 minutes** |

The migration is straightforward: find and replace old tool names with new prefixed names. All functionality remains identical.