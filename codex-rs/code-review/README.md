# Codex Code Review - Continuous Local AI Code Review

Continuous code review module for Codex CLI using local AI models.

## Features

✅ **Continuous File Watching** - Monitors your codebase for changes in real-time
✅ **Local AI Integration** - Works with Ollama and other local AI models
✅ **Code Analysis** - Detects bugs, style issues, complexity, and more
✅ **Smart Suggestions** - AI-powered improvement suggestions
✅ **Session Management** - Track review sessions and improvements over time
✅ **Configurable Policies** - Control what gets auto-fixed vs. requiring approval
✅ **Privacy First** - 100% local, no cloud API calls required

## Quick Start

### 1. Start Continuous Review

Watch your current directory and review changes:

```bash
codex review start
```

Run for a specific duration (e.g., 8 hours):

```bash
codex review start --duration 8
```

Watch a specific directory:

```bash
codex review start --dir ./src
```

### 2. Analyze Files

Analyze a single file:

```bash
codex review analyze src/main.rs
```

Get JSON output:

```bash
codex review analyze src/main.rs --output json
```

### 3. View Configuration

```bash
codex review configure --show
```

Initialize a configuration file:

```bash
codex review configure --init
```

### 4. Manage Sessions

List all review sessions:

```bash
codex review sessions
```

Show detailed session info:

```bash
codex review sessions --verbose
```

View statistics:

```bash
codex review stats
```

## Configuration

Create a `codex-review.toml` file or use `~/.codex/config.toml`:

```toml
[continuous_review]
enabled = true
watch_patterns = ["**/*.rs", "**/*.py", "**/*.js"]
ignore_patterns = ["**/target/**", "**/node_modules/**"]

  [continuous_review.triggers]
  on_file_save = true
  on_git_commit = true
  periodic_check_minutes = 30

  [continuous_review.policies]
  auto_fix_style = true
  auto_add_tests = false       # Requires approval
  auto_refactor = false         # Requires approval
  confidence_threshold = 0.85

  [continuous_review.local_ai]
  coding_model = "deepseek-coder:33b"
  review_model = "codellama:13b-instruct"
  quick_check_model = "qwen2.5-coder:7b"
  ollama_base_url = "http://localhost:11434/v1"
  concurrent_requests = 3

  [continuous_review.analysis]
  check_style = true
  detect_bugs = true
  suggest_refactoring = true
  check_complexity = true
  max_complexity = 10
```

## Recommended Local AI Models

| Model | Size | Best For | Speed |
|-------|------|----------|-------|
| **DeepSeek-Coder V2** | 16B-236B | Deep code understanding | Slow |
| **CodeLlama** | 7B-34B | Balanced performance | Medium |
| **Qwen2.5-Coder** | 7B-32B | Quick checks, multilingual | Fast |
| **StarCoder2** | 3B-15B | Lightweight analysis | Very Fast |

### Setup with Ollama

```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull models
ollama pull deepseek-coder:33b
ollama pull codellama:13b-instruct
ollama pull qwen2.5-coder:7b

# Verify
ollama list
```

## Architecture

```
┌─────────────────────────────────────────┐
│      Codex CLI (Enhanced)               │
├─────────────────────────────────────────┤
│                                          │
│  ┌──────────┐  ┌──────────┐            │
│  │ Reviewer │  │ Analyzer │            │
│  │  Agent   │←→│          │            │
│  └──────────┘  └──────────┘            │
│       ↓              ↓                  │
│  ┌──────────┐  ┌──────────┐            │
│  │ Watcher  │  │ Session  │            │
│  │          │  │ Manager  │            │
│  └──────────┘  └──────────┘            │
│                                          │
├─────────────────────────────────────────┤
│       Local AI (Ollama)                 │
│    Models: DeepSeek, CodeLlama, etc.   │
└─────────────────────────────────────────┘
```

## Module Structure

- **`config.rs`** - Configuration types and defaults
- **`watcher.rs`** - File system monitoring with notify
- **`analyzer.rs`** - Code analysis and metrics calculation
- **`reviewer.rs`** - Main continuous review orchestrator
- **`session.rs`** - Session management and persistence
- **`tools.rs`** - Review-specific tools and commands

## Development

### Build

```bash
cargo build -p codex-code-review
```

### Run Tests

```bash
cargo test -p codex-code-review
```

### Lint

```bash
cargo clippy -p codex-code-review
```

## Usage Examples

### Example 1: Basic Continuous Review

```bash
# Start continuous review in current directory
codex review start

# Output:
# 🚀 Starting continuous code review...
#    Watching: /home/user/project
# ✓ Continuous review started
#   Press Ctrl+C to stop
```

### Example 2: Analyze Specific File

```bash
codex review analyze src/lib.rs

# Output:
# 🔍 Analyzing: "src/lib.rs"
#
# 📊 Analysis Results for "src/lib.rs"
#    Duration: 125ms
#
# ⚠  Issues (2):
#    🟡 Style: Line exceeds 100 characters (line 45)
#    🟡 Complexity: Code complexity (15) exceeds threshold (10)
#
# 💡 Suggestions (1):
#    • Extract complex logic into separate functions (confidence: 80%)
#      Rationale: Reducing complexity improves readability
#
# 📈 Metrics:
#    Lines of code: 234
#    Complexity: 15
#    Functions: 12
#    Doc coverage: 65.0%
```

### Example 3: Initialize Configuration

```bash
codex review configure --init

# Creates codex-review.toml with default settings
```

## Comparison with Claude Code

| Feature | Claude Code | Codex + Local AI Review |
|---------|-------------|------------------------|
| **Privacy** | ☁️ Cloud | ✅ 100% Local |
| **Cost** | 💰 API fees | ✅ Free |
| **Speed** | Network-dependent | ⚡ Local inference |
| **Offline** | ❌ No | ✅ Yes |
| **Customization** | Limited | ✅ Full control |
| **Models** | Fixed | ✅ Your choice |
| **Context** | API limits | ✅ Hardware limits |

## Recent Enhancements (Phase 3 & 4)

- ✅ **Linter Integration** - Clippy, ESLint, Pylint support
- ✅ **AST Analysis** - Deep Rust code analysis with syn
- ✅ **AI Response Caching** - Intelligent caching to reduce API calls
- ✅ **Test Generation** - AI-powered unit test creation
- ✅ **Git Integration** - Auto-commit improvements with detailed messages
- ✅ **Enhanced Analyzer** - Multi-source analysis with deduplication

## Future Enhancements

- [ ] More linters (gofmt, black, prettier, Shellcheck)
- [ ] Git hooks integration (pre-commit)
- [ ] Web UI for session viewing
- [ ] IDE plugins (VS Code, IntelliJ)
- [ ] RAG (Retrieval Augmented Generation) for large codebases
- [ ] Model ensembles for better accuracy
- [ ] Distributed analysis across multiple machines

## Contributing

Contributions welcome! Areas for improvement:

1. **AI Model Integration** - Add support for more local models
2. **Analysis Tools** - Integrate with existing linters and static analyzers
3. **UI/UX** - Improve CLI output and add TUI
4. **Performance** - Optimize for large codebases
5. **Documentation** - Expand examples and guides

## License

Same as Codex CLI parent project.

## Support

For issues, questions, or feature requests, please file an issue on the main Codex repository.
