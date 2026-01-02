# CORA Integration for Codex

## What This Is

CORA (Cognitive Orchestration Runtime Architecture) integrated into OpenAI Codex fork as MCP server.

## Structure
```
cora-mcp/               # MCP server implementation
  src/index.ts          # Server entry point
  bin/mcp-server.js     # Compiled binary
```

## Setup
```bash
# 1. Build CORA MCP
cd cora-mcp
npm install
npm run build

# 2. Configure Codex
mkdir -p ~/.codex
cat >> ~/.codex/config.toml << 'END'

[[mcp.servers]]
name = "cora"
command = "node"
args = ["/full/path/to/cora-mcp/bin/mcp-server.js"]
END

# 3. Build Codex
cd codex-rs
cargo build --release
```

## Usage
```bash
# Launch Codex
./codex-rs/target/release/codex

# Use CORA tool
> Use cora_learn tool with module "0.1"
```

## Status

- ✅ MCP server created
- ⏳ Codex Rust build needed
- ⏳ CORA Python script integration
- ⏳ End-to-end testing

## Next Steps

1. Build Codex binary
2. Copy CORA script to project root
3. Test MCP integration
4. Wire CRT professor UI

---

## Tool Reference

### `cora_learn`

Launch CORA interactive learning session for a specific module.

**Input Schema:**
```json
{
  "module": "string (required)"
}
```

**Parameters:**
- `module`: Module identifier (e.g., "0.1", "1.0", "advanced.2")

**Returns:**
- Interactive session output from CORA Python runtime
- Learning progress and feedback
- Module completion status

**Example:**
```bash
# In Codex CLI
> Use cora_learn with module "0.1"
```

**Internal Behavior:**
1. Spawns Python process: `python3 ./cora <module>`
2. Captures stdout from interactive session
3. Returns formatted output to Codex
4. Terminates cleanly on session completion

---

## Architecture

### Integration Pattern

```
┌─────────────────────────────────────────────────┐
│ Codex CLI (Rust)                                │
│                                                 │
│  ┌───────────────────────────────────────────┐ │
│  │ MCP Client                                │ │
│  │  - Loads ~/.codex/config.toml             │ │
│  │  - Spawns MCP servers                     │ │
│  │  - Dispatches tool calls                  │ │
│  └─────────────────┬─────────────────────────┘ │
└────────────────────┼───────────────────────────┘
                     │
                     │ JSON-RPC over stdio
                     │
┌────────────────────▼───────────────────────────┐
│ CORA MCP Server (Node.js)                      │
│                                                 │
│  ┌───────────────────────────────────────────┐ │
│  │ Tool Handler: cora_learn                  │ │
│  │  - Validates input schema                 │ │
│  │  - Spawns Python subprocess               │ │
│  │  - Streams output back                    │ │
│  └─────────────────┬─────────────────────────┘ │
└────────────────────┼───────────────────────────┘
                     │
                     │ spawn()
                     │
┌────────────────────▼───────────────────────────┐
│ CORA Runtime (Python)                           │
│                                                 │
│  ./cora <module>                                │
│  - Loads module definition                     │
│  - Executes learning interactions              │
│  - Emits progress to stdout                    │
│  - Writes receipts (optional)                  │
└─────────────────────────────────────────────────┘
```

### Data Flow

1. **User Input** → Codex CLI
2. **Tool Call** → MCP Client → MCP Server (JSON-RPC)
3. **Process Spawn** → MCP Server → CORA Python
4. **Output Stream** → CORA Python → MCP Server
5. **Response** → MCP Server → Codex CLI → User

### File Locations

| Component | Path | Purpose |
|-----------|------|---------|
| MCP Server Source | `cora-mcp/src/index.ts` | Tool definitions, handlers |
| MCP Server Binary | `cora-mcp/bin/mcp-server.js` | Compiled executable |
| Codex Config | `~/.codex/config.toml` | MCP server registration |
| CORA Runtime | `./cora` (project root) | Python learning script |

---

## Configuration

### Basic Configuration

Minimal setup for local development:

```toml
[[mcp.servers]]
name = "cora"
command = "node"
args = ["/home/user/codex/cora-mcp/bin/mcp-server.js"]
```

### Production Configuration

With environment variables and logging:

```toml
[[mcp.servers]]
name = "cora"
command = "node"
args = ["/home/user/codex/cora-mcp/bin/mcp-server.js"]
env = { CORA_LOG_LEVEL = "info", CORA_RECEIPTS_DIR = "/var/log/cora" }
```

### Development Configuration

With hot-reload support:

```toml
[[mcp.servers]]
name = "cora-dev"
command = "npx"
args = ["tsx", "watch", "/home/user/codex/cora-mcp/src/index.ts"]
env = { NODE_ENV = "development" }
```

### Multiple Environments

```toml
# Production CORA
[[mcp.servers]]
name = "cora"
command = "node"
args = ["/home/user/codex/cora-mcp/bin/mcp-server.js"]

# Experimental CORA (beta modules)
[[mcp.servers]]
name = "cora-beta"
command = "node"
args = ["/home/user/codex/cora-mcp-beta/bin/mcp-server.js"]
env = { CORA_MODULES_PATH = "/opt/cora/beta-modules" }
```

---

## Troubleshooting

### Issue: MCP Server Not Found

**Symptom:**
```
Error: MCP server 'cora' not found
```

**Solution:**
1. Verify config file exists: `cat ~/.codex/config.toml`
2. Check absolute path in `args`
3. Ensure binary is built: `ls -la cora-mcp/bin/mcp-server.js`
4. Rebuild if missing: `cd cora-mcp && npm run build`

### Issue: Python Script Not Found

**Symptom:**
```
Error: ENOENT: no such file or directory, spawn 'python3'
```

**Solution:**
1. Install Python 3: `which python3`
2. Verify CORA script exists: `ls -la ./cora`
3. Make script executable: `chmod +x ./cora`
4. Update MCP server to use absolute path

### Issue: Module Not Found

**Symptom:**
```
CORA Error: Module "0.1" not found
```

**Solution:**
1. Check available modules: `./cora --list`
2. Verify module file exists in CORA modules directory
3. Check module naming convention
4. Review CORA logs for module loading errors

### Issue: Output Not Displayed

**Symptom:**
Tool call succeeds but no output shown

**Solution:**
1. Check if Python script writes to stdout (not stderr)
2. Verify MCP server stdout handling in `cora-mcp/src/index.ts:26`
3. Enable debug mode: `DEBUG=* node bin/mcp-server.js`
4. Check Codex CLI output formatting

### Issue: Permission Denied

**Symptom:**
```
Error: EACCES: permission denied
```

**Solution:**
1. Make binary executable: `chmod +x cora-mcp/bin/mcp-server.js`
2. Check CORA script permissions: `chmod +x ./cora`
3. Verify file ownership: `ls -la cora-mcp/bin/`
4. Run with appropriate user permissions

---

## Development

### Building from Source

```bash
# Install dependencies
cd cora-mcp
npm install

# Development build (watch mode)
npm run build -- --watch

# Production build
npm run build

# Clean build artifacts
npm run clean
```

### Adding New Tools

Edit `cora-mcp/src/index.ts`:

```typescript
const server = {
  name: 'cora-mcp',
  version: '0.1.0',
  tools: [
    {
      name: 'cora_learn',
      description: 'Launch CORA interactive learning session',
      inputSchema: {
        type: 'object',
        properties: {
          module: { type: 'string', description: 'Module ID' }
        },
        required: ['module']
      }
    },
    // Add new tool here
    {
      name: 'cora_status',
      description: 'Get CORA system status',
      inputSchema: {
        type: 'object',
        properties: {}
      }
    }
  ]
};

// Add handler
async function handleToolCall(name: string, args: any) {
  if (name === 'cora_learn') {
    // existing handler
  } else if (name === 'cora_status') {
    // new handler
    const child = spawn('python3', ['./cora', '--status']);
    // ... implementation
  }
}
```

### Testing Locally

```bash
# Test MCP server directly
node cora-mcp/bin/mcp-server.js

# Test with mock tool call
echo '{"tool":"cora_learn","args":{"module":"0.1"}}' | \
  node cora-mcp/bin/mcp-server.js

# Test CORA script independently
python3 ./cora 0.1
```

### Debugging

Enable verbose logging:

```typescript
// In cora-mcp/src/index.ts
child.stderr.on('data', (data) => {
  console.error('[CORA stderr]', data.toString());
});

console.error('[CORA] Spawning:', 'python3', ['./cora', args.module]);
```

---

## Examples

### Example 1: Basic Learning Session

```bash
$ codex
> Use cora_learn tool with module "0.1"

[CORA] Starting module 0.1: Introduction to Inference
[CORA] Loading lesson plan...
[CORA] Lesson 1: Understanding LMU execution model
[CORA]
[CORA] === Exercise 1 ===
[CORA] Map the following LMU concepts to CUDA equivalents:
[CORA] 1. LMU operation
[CORA] 2. LMU runner
[CORA] 3. LMU lane
[CORA]
[CORA] Session complete. Progress: 1/5 lessons
```

### Example 2: Advanced Module with Custom CORA Path

```toml
[[mcp.servers]]
name = "cora"
command = "node"
args = ["/home/user/codex/cora-mcp/bin/mcp-server.js"]
env = { CORA_SCRIPT = "/opt/cora/bin/cora" }
```

```typescript
// Update index.ts to use env var
const coraScript = process.env.CORA_SCRIPT || './cora';
const child = spawn('python3', [coraScript, args.module]);
```

### Example 3: Integration with Codex Workflow

```bash
# Start Codex session
$ codex

# Check CORA availability
> List available MCP tools
[cora_learn] Launch CORA interactive learning session

# Begin learning path
> Use cora_learn with module "0.1"
> Use cora_learn with module "0.2"
> Use cora_learn with module "1.0"

# Export session receipts
> Export CORA receipts to ./receipts.jsonl
```

### Example 4: Automated Curriculum Run

```python
# scripts/run_curriculum.py
import subprocess
import json

modules = ["0.1", "0.2", "1.0", "1.1", "advanced.1"]

for module in modules:
    print(f"Running module {module}...")

    # Call via Codex CLI
    result = subprocess.run(
        ["codex", "--tool", "cora_learn", "--args", json.dumps({"module": module})],
        capture_output=True,
        text=True
    )

    print(result.stdout)

    if result.returncode != 0:
        print(f"Warning: Module {module} failed")
        continue
```

---

## Integration with Celaya LMU

CORA MCP serves as the interactive learning interface for the Celaya Solutions LMU Curriculum Runtime.

### Relationship to LMU Components

| LMU Component | CORA Role |
|---------------|-----------|
| `celaya/lmu/syllabus/` | Defines module sequence for `cora_learn` |
| `celaya/lmu/runtime/` | Executes CORA sessions, emits receipts |
| `celaya/lmu/grading/` | Scores CORA session outputs |
| `celaya/lmu/artifacts/` | Stores session transcripts, receipts |

### CUDA Analogy (Per LMU Spec)

```
LMU op          → CUDA kernel
LMU runner      → kernel launch
LMU lane        → warp
KV cache        → SRAM/HBM
CORA receipts   → CUDA profiler
cora_learn      → nvprof session
```

### Expected Integration Flow

1. **Syllabus Definition** → `celaya/lmu/syllabus/syllabus.yaml`
2. **Module Generation** → LMU pipeline creates module specs
3. **CORA Execution** → `codex` calls `cora_learn` for each module
4. **Receipt Capture** → CORA emits JSONL events to `celaya/lmu/artifacts/`
5. **Grading** → LMU grader scores session against weights
6. **Summary** → Final metrics in `generation_summary.json`

---

## Performance Considerations

### MCP Server

- **Startup time**: ~50ms (Node.js spawn)
- **Memory**: ~30MB baseline
- **Concurrency**: One CORA session per tool call
- **Streaming**: stdout buffered until completion

### CORA Python Runtime

- **Varies by module**: 0.1 = ~2s, advanced modules = ~30s
- **Dependencies**: Ollama (if using LLM features)
- **Resource usage**: Defined by module complexity

### Optimization Strategies

1. **Keep MCP server running**: Codex maintains persistent connection
2. **Cache module definitions**: Avoid re-parsing syllabus.yaml
3. **Stream large outputs**: Modify MCP server to stream instead of buffer
4. **Parallel execution**: Run independent modules concurrently

---

## Security Notes

### Sandboxing

Currently, CORA MCP spawns arbitrary Python processes. For production:

1. **Validate module IDs**: Whitelist allowed modules
2. **Restrict execution**: Use `chroot` or containers
3. **Limit resources**: Apply `ulimit` to spawned processes
4. **Audit logging**: Track all `cora_learn` invocations

### Input Validation

```typescript
// Add to index.ts
function validateModule(module: string): boolean {
  const allowedPattern = /^[a-zA-Z0-9._-]+$/;
  return allowedPattern.test(module) && module.length < 50;
}

async function handleToolCall(name: string, args: any) {
  if (name === 'cora_learn') {
    if (!validateModule(args.module)) {
      throw new Error('Invalid module ID');
    }
    // ... rest of handler
  }
}
```

---

## References

- [MCP Protocol Specification](https://spec.modelcontextprotocol.io/)
- [Codex MCP Integration](./codex-rs/docs/mcp.md)
- [Celaya LMU Curriculum Spec](./CODEX.md)
- [CORA Module Definitions](./celaya/lmu/syllabus/)

---

## Changelog

### 0.1.0 (2026-01-02)
- Initial CORA MCP server implementation
- Single tool: `cora_learn`
- Basic stdout capture and return
- TypeScript build pipeline with tsup
- Integration documentation
