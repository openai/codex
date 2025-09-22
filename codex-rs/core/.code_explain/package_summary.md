# core Package Summary

## Purpose
Central library containing the core business logic and shared functionality for the entire Codex CLI system. Acts as the foundation that all other packages build upon.

## Key Components

### Configuration System
- **Config Loading**: TOML configuration file management
- **Profile Management**: Multiple configuration profiles
- **Environment Variables**: Override configs via environment
- **Defaults**: Sensible default configurations

### Authentication & Authorization
- **AuthManager**: Centralized authentication handling
- **CodexAuth**: Token management and refresh
- **API Key Support**: Multiple authentication methods
- **Session Persistence**: Secure credential storage

### Conversation Management
- **Session Handling**: Create, resume, and manage conversations
- **Message Processing**: Format and process chat messages
- **Context Management**: Maintain conversation context
- **History Tracking**: Conversation history and replay

### Model Integration
- **Provider Abstraction**: Unified interface for AI providers
- **Model Selection**: Dynamic model choice
- **Streaming Support**: Real-time response streaming
- **Token Management**: Usage tracking and limits

### File Operations
- **Git Integration**: Repository operations and diffs
- **File Manipulation**: Safe file read/write operations
- **Path Resolution**: Smart path handling
- **Change Tracking**: Monitor file modifications

### Security & Sandboxing
- **Policy Enforcement**: Apply security policies
- **Sandbox Integration**: Secure execution environments
- **Permission Management**: Fine-grained access control
- **Command Validation**: Pre-execution validation

### MCP Integration
- **Server Discovery**: Find and connect to MCP servers
- **Tool Management**: Register and invoke MCP tools
- **Protocol Handling**: MCP message processing
- **Resource Access**: MCP resource management

## Main Functionality
1. **Orchestration**: Coordinate between all Codex components
2. **State Management**: Maintain application state
3. **Event System**: Publish/subscribe event architecture
4. **Error Handling**: Centralized error management
5. **Logging**: Structured logging throughout system

## Dependencies
- `reqwest`: HTTP client for API calls
- `tokio`: Async runtime
- `serde`: Serialization/deserialization
- `askama`: Template engine
- `clap`: Command-line parsing
- Many others for specific functionality

## Integration Points
- Foundation for `cli`, `tui`, and `exec`
- Uses `protocol` for type definitions
- Integrates with `execpolicy` for security
- Connects to `mcp-client` for external tools
- Works with `ollama` for local models

## Architecture Patterns
- **Service Layer**: Business logic abstraction
- **Repository Pattern**: Data access abstraction
- **Factory Pattern**: Object creation
- **Observer Pattern**: Event notification
- **Strategy Pattern**: Pluggable behaviors

## Core Services
- Configuration service
- Authentication service
- Conversation service
- File service
- Git service
- Security service
- Model service
- Event service