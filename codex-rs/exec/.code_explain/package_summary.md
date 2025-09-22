# exec Package Summary

## Purpose
Headless execution interface for running Codex tasks programmatically without interactive UI. Enables automation, scripting, and integration with other tools.

## Key Components

### Execution Engine
- **Headless Mode**: Runs conversations without terminal UI
- **Event Processing**: Handles conversation events and outputs
- **Session Management**: Resume and manage existing sessions

### Output Formats
- **Human-readable**: Formatted text output for terminal display
- **JSON**: Structured output for programmatic consumption
- **Streaming**: Real-time event streaming

## Main Functionality
1. **Command Execution**: Process user prompts and execute AI conversations
2. **Session Resumption**: Continue previous conversations from saved state
3. **Image Handling**: Support for image inputs in prompts
4. **OSS Model Integration**: Works with local open-source models via Ollama
5. **Event Streaming**: Real-time processing of conversation events

## Dependencies
- `core`: Core Codex functionality and types
- `arg0`: Binary dispatch system
- `common`: Shared utilities and configurations
- `ollama`: Local model integration
- `protocol`: Message and event definitions

## Integration Points
- Called by `cli` as a subcommand
- Uses `core` for conversation management
- Integrates with `ollama` for OSS models
- Works with authentication from `login`

## Use Cases
- **CI/CD Integration**: Automated code review and generation
- **Batch Processing**: Process multiple prompts programmatically
- **Scripting**: Include in shell scripts and automation
- **Testing**: Automated testing of AI interactions
- **Non-interactive Environments**: Servers, containers, remote systems

## Command Line Options
- Prompt input (direct or from file)
- Session management (new or resume)
- Output format selection
- Model configuration
- Image attachment support

## Output Management
- Structured event handling
- Progress reporting
- Error propagation
- Result formatting based on output mode