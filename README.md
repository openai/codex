# A Heterogeneous Multi-Agent Communication Framework for Collaborative Code Generation: Implementation of Human-Mediated Inter-Model Dialogue Systems

## Abstract

This paper presents a novel architectural framework for orchestrating collaborative interactions between heterogeneous large language models (LLMs) in software development contexts. We introduce the Mixture-of-Idiots (MoI) paradigm, a human-in-the-loop system that enables structured communication between Anthropic's Claude Code and OpenAI's Codex CLI through an intelligent message routing infrastructure. Our implementation demonstrates how specialized AI agents can be coordinated through formal protocol specifications to achieve emergent collaborative behaviors while maintaining human oversight and control authority.

## 1. Introduction

The proliferation of specialized large language models has created opportunities for leveraging complementary capabilities across different AI architectures. However, existing frameworks lack sophisticated mechanisms for inter-model communication, particularly in domains requiring iterative collaboration such as software development. This work addresses the fundamental challenge of orchestrating dialogue between disparate LLM systems while preserving human agency in the collaborative process.

We present a distributed system architecture that implements formal communication protocols between two distinct AI coding assistants, mediated by an intelligent routing infrastructure that supports both autonomous and directed interaction modes. The system demonstrates novel approaches to multi-agent coordination in computational environments where individual agents possess asymmetric capabilities and operational constraints.

## 2. System Architecture

### 2.1 Architectural Overview

The MoI framework implements a four-component distributed architecture:

**Master Control Interface (MCI)**: A human-operated command interpreter that provides routing directives and conversation management capabilities. The MCI implements a domain-specific language for agent targeting and conversation flow control.

**Smart Bridge Protocol (SBP)**: A stateful message routing engine that manages inter-agent communication, maintains conversation context, and enforces protocol constraints. The SBP functions as both a message broker and conversation orchestrator.

**Agent Interface Adapters (AIA)**: Specialized communication adapters that abstract the underlying API protocols of each LLM system, providing normalized interfaces for message exchange and response handling.

**Persistent State Management (PSM)**: A file-based persistence layer that maintains conversation history, system state, and configuration parameters across execution sessions.

### 2.2 Communication Topology

The system implements a hub-and-spoke topology with the Smart Bridge serving as the central coordination node. All inter-agent communication is mediated through the bridge, which maintains strict ordering guarantees and prevents message collision through atomic file operations.

```
Human Operator ←→ Master Control Interface
                           ↓
                   Smart Bridge Protocol
                    ↙              ↘
        Claude Agent Interface    Codex Agent Interface
                    ↓                      ↓
               Claude Code               Codex CLI
```

## 3. Bridge Protocol Specification

### 3.1 Message Format Specification

The bridge protocol employs JSON-structured messages with mandatory temporal ordering and routing metadata:

```json
{
  "timestamp": "ISO-8601 timestamp",
  "source": "HUMAN|CLAUDE|CODEX",
  "target": "CLAUDE|CODEX|SYSTEM",
  "message_type": "COMMAND|RESPONSE|DIRECTIVE",
  "payload": "message content",
  "context": {
    "turn_number": "integer",
    "conversation_mode": "AI_TO_AI|DIRECTED|PAUSED",
    "routing_hint": "optional routing guidance"
  }
}
```

### 3.2 State Machine Specification

The Smart Bridge implements a finite state automaton with the following states:

- **INITIALIZATION**: System bootstrap and agent registration
- **AI_AUTONOMOUS**: Agents communicate without human intervention
- **HUMAN_DIRECTED**: Human operator controls message routing
- **SINGLE_AGENT**: Communication directed to specific agent
- **PAUSED**: All automated communication suspended
- **SHUTDOWN**: Graceful system termination

State transitions are triggered by command primitives from the Master Control Interface or internal protocol events.

### 3.3 Routing Algorithm

The bridge employs a priority-based routing algorithm with the following precedence hierarchy:

1. **Explicit Human Commands**: Direct routing directives (/claude, /codex)
2. **Context-Aware Inference**: Analysis of message content for appropriate agent selection
3. **Round-Robin Default**: Alternating agent selection for general conversation
4. **Error Recovery**: Fallback routing for failed message delivery

## 4. Implementation Details

### 4.1 Agent Interface Abstraction

Each Agent Interface Adapter implements a standardized interface:

```javascript
class AgentInterface {
  async sendMessage(content, context)
  async receiveMessage()
  async getStatus()
  async initialize()
  async shutdown()
}
```

The Claude adapter interfaces with the Claude Code environment through file-based message exchange, while the Codex adapter spawns subprocess instances of the Codex CLI with appropriate command-line arguments for autonomous operation.

### 4.2 Persistence and Logging

The system implements comprehensive logging at multiple levels:

- **Protocol-level logging**: All bridge operations and state transitions
- **Message-level logging**: Complete conversation transcripts with metadata
- **System-level logging**: Performance metrics and error conditions
- **Agent-level logging**: Individual agent responses and processing times

### 4.3 Configuration Management

System configuration is managed through environment-based parameter injection with hierarchical override capabilities:

1. Default configuration constants
2. Environment file (.env) parameters
3. Runtime environment variables
4. Command-line argument overrides

## 5. Operational Semantics

### 5.1 Conversation Flow Control

The system supports three primary operational modes:

**Autonomous Mode**: Agents engage in self-directed dialogue with minimal human intervention. The bridge maintains conversation flow and prevents infinite loops through configurable turn limits and timeout mechanisms.

**Directed Mode**: Human operator explicitly routes messages to specific agents while maintaining conversation context. This mode enables precise control over agent interactions and collaborative workflows.

**Hybrid Mode**: Dynamic switching between autonomous and directed modes based on conversation context and human operator preferences.

### 5.2 Error Handling and Recovery

The framework implements robust error handling with graceful degradation:

- Agent timeout handling with configurable retry mechanisms
- Network failure recovery through message queuing
- State corruption detection and recovery procedures
- Graceful shutdown with conversation state preservation

### 5.3 Concurrency and Synchronization

All inter-component communication employs atomic file operations to prevent race conditions. The system implements a cooperative multitasking model where each component operates in dedicated execution contexts with clearly defined message passing interfaces.

## 6. Experimental Framework

### 6.1 Deployment Procedure

System deployment follows a standardized initialization sequence:

1. Configuration validation and environment setup
2. Bridge infrastructure initialization
3. Agent interface registration and capability negotiation
4. Master Control Interface activation
5. Initial conversation bootstrap

### 6.2 Command Interface Specification

The Master Control Interface supports the following command primitives:

- **Direct Agent Targeting**: `/claude <message>`, `/codex <message>`
- **Flow Control**: `/continue`, `/pause`, `/status`
- **System Management**: `/help`, `/quit`
- **Implicit Continuation**: Undecorated messages continue current conversation mode

### 6.3 Performance Metrics

The system collects the following operational metrics:

- Message round-trip latency
- Agent response generation time
- Bridge routing overhead
- Conversation turn distribution
- Error rate and recovery time

## 7. Quick Start Guide

### 7.1 Prerequisites

```bash
# System requirements
Node.js 18.0+
OpenAI API key with Codex access
WSL2 or Linux environment
```

### 7.2 Installation

```bash
# Clone repository
git clone https://github.com/openai/codex.git
cd codex

# Build Codex CLI
cd codex-cli
npm install
npm run build
cd ..

# Setup bridge system
cd llm_bridge
echo "OPENAI_API_KEY=your_key_here" > .env
chmod +x start_mixture.sh
```

### 7.3 Execution

```bash
# Launch complete system
./start_mixture.sh

# System opens 4 terminals:
# - Smart Bridge (message router)
# - Master Control (your interface)
# - Claude Enhanced (Claude Code adapter)
# - Codex Enhanced (Codex CLI adapter)
```

### 7.4 Operation

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

## 8. Technical Specifications

### 8.1 System Requirements

- Node.js runtime environment (version 18.0+)
- OpenAI API access credentials
- POSIX-compatible file system
- Terminal emulator with multiple session support

### 8.2 Configuration Parameters

```javascript
{
  OPENAI_API_KEY: "API authentication credential",
  CLAUDE_MODEL: "claude-3-sonnet",
  CODEX_MODEL: "o1-mini", 
  AUTO_CONTINUE_CONVERSATION: true,
  LOG_LEVEL: "info|debug|error",
  MAX_CONVERSATION_TURNS: 50,
  AGENT_TIMEOUT_MS: 90000,
  BRIDGE_POLL_INTERVAL_MS: 500
}
```

## 9. Research Applications

This framework enables systematic study of:

- Inter-model collaboration patterns
- Emergent behaviors in multi-agent systems
- Human-mediated AI coordination effectiveness
- Asymmetric capability utilization in AI teams
- Protocol optimization for multi-agent communication

## 10. Conclusion

The Mixture-of-Idiots framework demonstrates a practical approach to coordinating heterogeneous AI agents in collaborative software development contexts. The architecture's emphasis on human oversight, formal protocol specification, and robust error handling provides a foundation for further research into multi-agent AI coordination systems.

The implementation serves as both a functional tool for AI-assisted development and a research platform for studying emergent behaviors in human-mediated multi-agent systems.

---

**Repository Structure:**
```
codex/
├── codex-cli/              # OpenAI Codex CLI implementation
├── llm_bridge/             # Mixture-of-Idiots bridge system
│   ├── mixture_config.js   # Configuration management
│   ├── master_control.js   # Human control interface
│   ├── smart_bridge.js     # Message routing engine
│   ├── claude_enhanced.js  # Claude Code adapter
│   ├── codex_enhanced.js   # Codex CLI adapter
│   └── start_mixture.sh    # System launcher
└── CLAUDE.md              # Claude Code development guide
```

**License:** Apache-2.0
