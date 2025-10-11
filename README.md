# Codex

<div align="center">

**AI-powered coding assistant in your terminal**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.47.0--alpha.1-green.svg)](VERSION)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

[Installation](#-installation) • [Features](#-features) • [Usage](#-usage) • [Documentation](#-documentation)

</div>

---

## 🚀 zapabob/codex Enhanced Features

### 🤖 Sub-Agent System (NEW!)

Delegate specialized tasks to AI sub-agents with fine-grained permissions:

```bash
# Single agent execution
codex delegate code-reviewer --scope ./src --budget 40000

# Parallel execution (NEW!)
codex delegate-parallel code-reviewer,test-gen,sec-audit \
  --goals "Review code,Generate tests,Security audit" \
  --scopes ./src,./tests,./package.json \
  --budgets 40000,30000,20000

# Custom agent from prompt (NEW!)
codex agent-create "Find all TODO comments and create a summary report" \
  --budget 50000
```

#### Available Sub-Agents

| Agent | Purpose | Token Budget |
|-------|---------|--------------|
| `code-reviewer` | Security, performance, best practices analysis | 40,000 |
| `test-gen` | Unit/Integration/E2E test generation (80%+ coverage) | 30,000 |
| `sec-audit` | CVE scanning, dependency audit, patch recommendations | 50,000 |
| `researcher` | Deep research with citations and cross-validation | 60,000 |

#### New Capabilities

- **Parallel Execution**: Run multiple agents simultaneously for 3x faster completion
- **Custom Agents**: Create agents from natural language prompts with LLM-powered generation
- **Process Isolation**: Each agent runs in a separate process for enhanced security

**Quick Start**: See [SUBAGENTS_QUICKSTART.md](SUBAGENTS_QUICKSTART.md) and [PARALLEL_CUSTOM_AGENT_GUIDE.md](PARALLEL_CUSTOM_AGENT_GUIDE.md)

---

### 🔍 Deep Research (Enhanced)

Multi-source research with citation and contradiction detection:

```bash
codex research "Rust async programming best practices" --depth 3
```

**Features**:
- DuckDuckGo HTML scraping (no API key required)
- Smart sub-query generation
- Cross-source validation
- Cited reports with confidence scores

---

### 🔧 Codex MCP Integration (In Progress)

**New**: Codex itself as an MCP server for sub-agents!

```yaml
# .codex/agents/my-agent.yaml
tools:
  mcp:
    - codex_read_file       # Full Codex file reading
    - codex_grep            # Full Codex grep
    - codex_codebase_search # Semantic search
```

**Status**: 🚧 Implementation in progress
**Design**: [_docs/2025-10-11_CodexMCP化設計書.md](_docs/2025-10-11_CodexMCP化設計書.md)

---

### 📋 For Contributors

This fork maintains **dual compatibility** with:
- ✅ OpenAI official repository
- ✅ zapabob enhancements

**Development Guide**: [.codex/META_PROMPT_CONTINUOUS_IMPROVEMENT.md](.codex/META_PROMPT_CONTINUOUS_IMPROVEMENT.md)

#### CI/CD Pipeline

- **Continuous Integration**: Automated testing on every PR
  - 3 platforms (Ubuntu, Windows, macOS)
  - Clippy lint + Rustfmt check
  - Agent definition validation
  - Integration tests (Deep Research + Sub-Agent)
  - Security audit (cargo-audit)

- **Continuous Delivery**: Automated releases on tag push
  - Multi-platform binaries (Linux x64, Windows x64, macOS x64/ARM64)
  - npm package generation
  - GitHub Release creation
  - Auto-generated release notes

**CI/CD Guide**: [CI_CD_SETUP_GUIDE.md](CI_CD_SETUP_GUIDE.md)

---

## 🎯 What is Codex?

Codex is an AI-powered coding assistant that runs in your terminal. It helps you write, understand, and improve code through natural conversation.

### ✨ Features

- **Interactive Chat**: Natural language conversations about your code
- **Code Understanding**: Analyze, explain, and refactor existing code
- **File Operations**: Read, write, and modify files with AI assistance
- **Shell Integration**: Execute commands safely with sandboxing
- **MCP Support**: Extensible via Model Context Protocol
- **Multi-Model**: Support for GPT-4, Claude, and local models

---

## 📦 Installation

### npm (Recommended)

```bash
npm install -g @openai/codex
```

### From Source

```bash
git clone https://github.com/zapabob/codex.git
cd codex/codex-rs
cargo build --release -p codex-cli
npm install -g ./codex-cli
```

---

## 🎮 Usage

### Basic Commands

```bash
# Start interactive session
codex

# Non-interactive execution
codex exec "Add error handling to main.rs"

# Deep research
codex research "Topic to research"

# Delegate to sub-agent
codex delegate code-reviewer --scope ./src

# Resume previous session
codex resume
```

### Configuration

```bash
# Login
codex login

# View status
codex login status

# Configure model
codex -c model="gpt-4" "Your prompt"
```

---

## 📚 Documentation

### Official Documentation
- [Getting Started](docs/getting-started.md)
- [Installation Guide](docs/install.md)
- [Configuration](docs/config.md)
- [Advanced Usage](docs/advanced.md)
- [FAQ](docs/faq.md)

### Enhanced Features (zapabob)
- [Sub-Agents Quick Start](SUBAGENTS_QUICKSTART.md)
- [Requirements Specification](docs/REQUIREMENTS_SPECIFICATION.md)
- [Implementation Plan](_docs/2025-10-11_要件定義書に基づく実装計画.md)
- [Codex MCP Design](_docs/2025-10-11_CodexMCP化設計書.md)

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────┐
│           Codex CLI (Node.js)           │
│  codex, codex exec, codex delegate      │
└──────────────────┬──────────────────────┘
                   │
┌──────────────────▼──────────────────────┐
│      Codex Core (Rust)                  │
│  ┌──────────────┐  ┌─────────────────┐ │
│  │ AgentRuntime │  │ ModelClient     │ │
│  │  Sub-Agents  │  │  LLM Interface  │ │
│  └──────────────┘  └─────────────────┘ │
│  ┌──────────────┐  ┌─────────────────┐ │
│  │ Deep Research│  │ MCP Integration │ │
│  └──────────────┘  └─────────────────┘ │
└─────────────────────────────────────────┘
```

---

## 🤝 Contributing

We welcome contributions! Please see:
- [Contributing Guide](docs/contributing.md)
- [Development Workflow](.codex/META_PROMPT_CONTINUOUS_IMPROVEMENT.md)

---

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- **OpenAI** - Original Codex project
- **Anthropic** - Claude model support
- **Contributors** - All contributors to the project

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/zapabob/codex/issues)
- **Discussions**: [GitHub Discussions](https://github.com/zapabob/codex/discussions)
- **Twitter**: [@zapabob](https://twitter.com/zapabob)

---

<div align="center">

**Made with ❤️ by the Codex community**

**Version**: 0.47.0-alpha.1  
**Last Updated**: 2025-10-11

</div>
