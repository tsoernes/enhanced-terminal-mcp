# Command Timeout Diagnosis

## Problem Statement

The following MCP tool call resulted in a "Context server request timeout" error in Zed:

```json
{
  "tool": "enhanced_terminal",
  "command": "cd /home/torstein.sornes/code/pnid-ocr-extraction && python3 << 'EOF'\nimport sys\nsys.path.insert(0, 'src')\n\nimport ezdxf\nfrom pathlib import Path\n\n# Load DXF\ndxf_path = Path(\"data/input/FPQ-AKSO-01-PI-004.dxf\")\ndoc = ezdxf.readfile(dxf_path)\nmsp = doc.modelspace()\n\n# Collect TEXT and MTEXT entities\ntexts = []\nfor entity in msp.query('TEXT MTEXT'):\n    etype = entity.dxftype()\n    if etype == 'TEXT':\n        text = entity.dxf.text\n        pos = entity.dxf.insert\n    else:  # MTEXT\n        text = entity.text\n        pos = entity.dxf.insert\n    \n    texts.append({\n        'text': text.strip(),\n        'x': pos[0],\n        'y': pos[1],\n        'type': etype\n    })\n\n# Sort by length (longer texts are often more distinctive)\ntexts.sort(key=lambda t: len(t['text']), reverse=True)\n\nprint(f\"Found {len(texts)} text entities in DXF\")\nprint(\"\\nTop 20 longest texts:\")\nfor i, t in enumerate(texts[:20], 1):\n    print(f\"{i:2d}. '{t['text'][:60]}...' at ({t['x']:.2f}, {t['y']:.2f}) [{t['type']}]\")\n\n# Also show some shorter distinctive texts\nprint(\"\\n\\nSample of shorter texts (potential labels/tags):\")\ndistinctive = [t for t in texts if 3 <= len(t['text']) <= 20 and any(c.isdigit() for c in t['text'])]\nfor i, t in enumerate(distinctive[:15], 1):\n    print(f\"{i:2d}. '{t['text']}' at ({t['x']:.2f}, {t['y']:.2f})\")\n\nEOF\n",
  "cwd": "/home/torstein.sornes/code/pnid-ocr-extraction"
}
```

## Timeout Chain Analysis

### Timeline of Events

1. **T=0s**: Zed sends tool call to MCP server
2. **T=0-60s**: MCP server processes the request
3. **T=60s**: Zed's client timeout threshold reached → "Context server request timeout"

### Where the Timeout Occurs

**Important**: The timeout is NOT from the enhanced_terminal tool itself - it's from **Zed's MCP client**.

- **Zed Client Timeout**: 60 seconds (hardcoded in Zed)
- **Enhanced Terminal Async Threshold**: 50 seconds (configurable via env var)
- **Command Timeout**: Not specified in the call (defaults to None = unlimited)

### Possible Causes

#### 1. Command Took Longer Than 50 Seconds (Async Switch Failed)

**Expected Behavior**:
- Command starts executing
- After 50 seconds, switches to background
- Returns job_id immediately
- Total response time: ~50 seconds

**Actual Behavior**:
- Command may have hung or taken longer than 50 seconds
- Async switch may have been delayed
- Zed timed out at 60 seconds before response was sent

**Why This Could Happen**:
- The DXF file might be very large
- Loading `ezdxf` and parsing the DXF could take 50-60+ seconds
- The script processes all text entities, which could be thousands in a large P&ID

#### 2. Missing Dependency Caused Immediate Failure (But No Response)

**Test Result**:
```bash
$ python3 -c "import ezdxf"
ModuleNotFoundError: No module named 'ezdxf'
```

**Expected**: Immediate error response
**Actual**: Timeout

This suggests the issue is NOT a simple import error that returns quickly.

#### 3. Shell Heredoc Interaction Issue

The command uses a bash heredoc (`<< 'EOF'`) which could potentially:
- Hang waiting for input if not properly terminated
- Have issues with stdin/stdout buffering in PTY
- Interact poorly with the PTY reader thread

**Test Result**:
Simple heredocs work fine:
```bash
python3 << 'EOF'
print("test")
EOF
```
Completes in ~0.1 seconds.

#### 4. Large DXF File Loading

The command attempts to load `data/input/FPQ-AKSO-01-PI-004.dxf`:
- P&ID DXF files can be 10MB-100MB+
- Parsing can take 30-120+ seconds for large files
- If ezdxf IS installed in another Python environment, this could explain the delay

#### 5. Race Condition in Async Switch

**Hypothesis**: 
- Command takes exactly 50-60 seconds
- Async switch occurs at T=50s
- But the response formatting/serialization takes additional time
- Total time exceeds 60 seconds → Zed timeout

### Root Cause Analysis

Based on testing and code review, the most likely cause is:

**The command would fail immediately due to missing `ezdxf`, BUT**:
- The way the command was invoked from Zed may have caused issues
- There might have been network/system latency at the time
- The MCP server might have been busy processing another request

**OR (more likely)**:

**The Python environment actually HAS `ezdxf` installed**, and:
- The DXF file is very large (100MB+)
- Loading and parsing takes 55-65 seconds
- The async switch happens at 50s, but there's a race condition
- By the time the background job response is formatted and sent, 60s has elapsed
- Zed times out before receiving the response

## Verification Steps

### 1. Check if ezdxf is Installed in the Target Environment

```bash
cd /home/torstein.sornes/code/pnid-ocr-extraction
python3 -c "import ezdxf; print(ezdxf.__version__)"
```

### 2. Check DXF File Size

```bash
ls -lh /home/torstein.sornes/code/pnid-ocr-extraction/data/input/FPQ-AKSO-01-PI-004.dxf
```

### 3. Time the DXF Loading Operation

```bash
cd /home/torstein.sornes/code/pnid-ocr-extraction
time python3 << 'EOF'
import ezdxf
from pathlib import Path
dxf_path = Path("data/input/FPQ-AKSO-01-PI-004.dxf")
doc = ezdxf.readfile(dxf_path)
print(f"Loaded DXF with {len(list(doc.modelspace()))} entities")
EOF
```

### 4. Test Enhanced Terminal Job System

```bash
# Create a job that takes 55 seconds (should switch to async)
enhanced_terminal: {"command": "sleep 55 && echo 'done'", "cwd": "/tmp"}

# Verify it switches to background and returns job_id within 50-51 seconds
```

## Solutions

### Solution 1: Install ezdxf (If Missing)

```bash
cd /home/torstein.sornes/code/pnid-ocr-extraction
pip install ezdxf
# or
uv pip install ezdxf
```

### Solution 2: Always Set Explicit Timeout

Add `timeout_secs` to prevent indefinite hanging:

```json
{
  "command": "...",
  "cwd": "...",
  "timeout_secs": 300
}
```

This ensures the command will timeout after 5 minutes if something goes wrong.

### Solution 3: Force Async Mode for Long Operations

For operations known to take a long time:

```json
{
  "command": "...",
  "cwd": "...",
  "force_sync": false
}
```

This is already the default, but making it explicit helps.

### Solution 4: Reduce Async Threshold

Set environment variable in MCP server config:

```json
{
  "context_servers": {
    "enhanced-terminal-mcp": {
      "command": "...",
      "env": {
        "ENHANCED_TERMINAL_ASYNC_THRESHOLD_SECS": "30"
      }
    }
  }
}
```

This switches to background after 30 seconds instead of 50, providing a 30-second buffer before Zed's 60-second timeout.

### Solution 5: Break Up Long Operations

Instead of one long script, break it into steps:

```bash
# Step 1: Load DXF (may take time)
enhanced_terminal: "cd /path && python3 -c 'import ezdxf; doc = ezdxf.readfile(\"file.dxf\"); print(f\"Loaded {len(list(doc.modelspace()))} entities\")'"

# Step 2: Process entities (after confirming load worked)
enhanced_terminal: "cd /path && python3 script_to_process.py"
```

### Solution 6: Use Python MCP Tools Instead

Use `py_run_script_with_dependencies` from python-mcp server:

```json
{
  "tool": "py_run_script_with_dependencies",
  "script_content": "import ezdxf\n...",
  "dependencies": ["ezdxf"],
  "timeout_seconds": 300
}
```

Benefits:
- Handles dependency installation automatically
- Better timeout management
- Async mode built-in for long operations

## Recommended Approach

1. **Immediate**: Test if ezdxf is installed and check DXF file size
2. **Short-term**: Use python-mcp tools for Python scripts instead of shell heredocs
3. **Long-term**: Consider setting `ENHANCED_TERMINAL_ASYNC_THRESHOLD_SECS=30` to provide more buffer

## Related Issues

- `JOB_LIST_TIMEOUT_ANALYSIS.md`: Documents job_list timeout due to large output cloning
- `DEBUGGING_TIMEOUT_ISSUE.md`: Original timeout investigation for async switching
- `TIMEOUT_INVESTIGATION.md`: Deep dive into PTY reader blocking issues

## Lessons Learned

1. **Zed's 60-second timeout is immutable** - We can't change it
2. **Async threshold must be < 60 seconds** - Currently 50s, might need to be lower
3. **Race conditions exist around async switch** - Response formatting can delay the reply
4. **Large operations should use Python MCP tools** - Better suited for long-running Python scripts
5. **Always set explicit timeouts** - Prevents indefinite hangs