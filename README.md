# A Mixture-of-Idiots: Human in the Loop Console between CODEX and CLAUDE

## Abstract

This paper presents a novel architectural framework for orchestrating collaborative interactions between heterogeneous large language models (LLMs) in software development contexts. We introduce the Mixture-of-Idiots (MoI) paradigm, a human-in-the-loop system that enables structured communication between Anthropic's Claude Code and OpenAI's Codex CLI through an intelligent message routing infrastructure. Our implementation demonstrates how specialized AI agents can be coordinated to achieve emergent collaborative behaviors while maintaining human oversight and control authority. This system can also serve as a research platform for studying multi-agent AI coordination.

## Quick Start Guide

### Prerequisites

```bash
# System requirements
Node.js 18.0+
OpenAI API key with Codex access
WSL2 or Linux environment
```

### Installation

```bash
# Clone this repository
git clone https://github.com/HarleyCoops/Codex-To-Claude.git
cd Codex-To-Claude

# Clone Codex CLI repository
git clone https://github.com/openai/codex.git
cd codex

# Build Codex CLI
cd codex-cli
npm install
npm run build
cd ..

# Setup bridge system (see llm_bridge/README.md for details)
cd llm_bridge
# (Follow instructions in llm_bridge/README.md to configure .env)
chmod +x start_mixture.sh
```

### Execution

```bash
# Launch complete system
./start_mixture.sh

# System opens 4 terminals:
# - Smart Bridge (message router)
# - Master Control (your interface)
# - Claude Enhanced (Claude Code adapter)
# - Codex Enhanced (Codex CLI adapter)
```

### Operation

In the Master Control terminal:

```bash
# Direct agent communication
/claude analyze this codebase structure
/codex implement the suggested architecture

# Autonomous collaboration
Let's build a web application together

# System management
/status    # View system state
/pause     # Halt AI conversation
/continue  # Resume AI conversation
/quit      # Shutdown system
```

## 1. Introduction

The proliferation of specialized large language models (LLMs) like OpenAI's Codex and Anthropic's Claude has created opportunities for leveraging their complementary capabilities. This project addresses the challenge of orchestrating dialogue between these LLMs, particularly in domains like software development, while ensuring human oversight.

We present a "Mixture-of-Idiots" (MoI) system that enables two AI coding assistants to collaborate. This is facilitated by an intelligent message routing infrastructure, allowing for both autonomous AI-to-AI conversation and human-directed interaction. The specifics of this bridge system, its components, and detailed setup are described in `llm_bridge/README.md`.

## 2. Technical Specifications

### 2.1 System Requirements

- Node.js runtime environment (version 18.0+).
- OpenAI API key with access to Codex models.
- A POSIX-compatible file system (common in Linux/WSL2/macOS).
- A terminal emulator that supports multiple sessions/tabs/windows to run the components.

### 2.2 Configuration Parameters

Specific configuration parameters for the LLM Bridge, such as API keys, model names, and operational settings, are detailed in `llm_bridge/mixture_config.js` and its accompanying `llm_bridge/README.md`.

## 3. Conclusion

The Mixture-of-Idiots framework provides a system for coordinating different LLMs like Codex and Claude, enabling them to collaborate on tasks under human supervision. It aims to be a useful tool for AI-assisted development and a platform for exploring multi-agent AI interactions. For detailed information on the bridge components and setup, please refer to the `llm_bridge/README.md`.

---

**Repository Structure:**
```
codex/                  # Main directory for the Codex CLI tools
├── codex-cli/          # OpenAI Codex CLI implementation
llm_bridge/             # Mixture-of-Idiots bridge system
├── README.md           # Detailed documentation for the LLM Bridge
├── mixture_config.js   # Configuration management
├── master_control.js   # Human control interface
├── smart_bridge.js     # Message routing engine
├── claude_enhanced.js  # Claude Code adapter
├── codex_enhanced.js   # Codex CLI adapter
└── start_mixture.sh    # System launcher
CLAUDE.md               # Claude Code development guide (if still relevant)
README.md               # This file - high-level project overview
```

**License:** Apache-2.0
