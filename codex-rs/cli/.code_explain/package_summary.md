# cli Package Summary

## Purpose
Main CLI entry point that orchestrates all Codex functionality and dispatches to appropriate subcommands. Acts as the unified interface for all Codex operations.

## Key Components

### Command Parser
- **Main Entry Point**: Primary binary executable
- **Subcommand Routing**: Dispatch to appropriate handlers
- **Argument Parsing**: Process command-line arguments
- **Help Generation**: Auto-generated help text

### Subcommand Integration
- **Exec Command**: Headless execution mode
- **TUI Command**: Interactive terminal UI
- **Login Command**: Authentication management
- **MCP Server**: Start MCP server mode
- **Config Commands**: Configuration management

### Shell Integration
- **Completion Scripts**: Bash/Zsh/Fish completions
- **Environment Setup**: Shell environment configuration
- **Alias Management**: Command aliases
- **Path Integration**: Binary path management

### Profile Management
- **Profile Selection**: Choose configuration profiles
- **Profile Creation**: Create new profiles
- **Profile Switching**: Switch between profiles
- **Profile Export**: Export/import profiles

## Main Functionality
1. **Command Orchestration**: Route to appropriate subcommands
2. **Configuration Loading**: Initialize configuration
3. **Environment Setup**: Prepare execution environment
4. **Error Handling**: Top-level error management
5. **Version Management**: Handle version info

## Dependencies
- `clap`: Command-line parsing
- `core`: Core functionality
- `exec`: Headless execution
- `tui`: Terminal UI
- `login`: Authentication
- `mcp-server`: MCP server
- All other workspace packages

## Integration Points
- Entry point for ALL user interactions
- Dispatches to specialized packages
- Manages global configuration
- Handles cross-cutting concerns

## Command Structure

### Top-level Commands
- `codex exec`: Run headless execution
- `codex tui`: Start terminal UI
- `codex login`: Manage authentication
- `codex mcp-server`: Start MCP server
- `codex config`: Manage configuration

### Global Options
- `--profile`: Select configuration profile
- `--config`: Specify config file
- `--verbose`: Increase verbosity
- `--quiet`: Suppress output
- `--version`: Show version

### Hidden Commands
- Debug commands
- Development tools
- Migration utilities
- Maintenance operations

## Usage Patterns
- Direct command execution
- Interactive mode via TUI
- Scripting via exec
- Service mode via MCP
- Configuration management

## Shell Completion
- Dynamic completion generation
- Context-aware suggestions
- Subcommand completions
- Option value completions

## Error Management
- User-friendly error messages
- Debug mode for developers
- Error code standardization
- Help suggestions on errors