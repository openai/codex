# execpolicy Package Summary

## Purpose
Security policy engine for validating and controlling command execution in Codex. Provides a Starlark-based policy language for defining and enforcing security constraints on system commands.

## Key Components

### Policy Language
- **Starlark-based DSL**: Uses Starlark (Python-like) language for policy definitions
- **Pattern Matching**: Supports regex and glob patterns for command validation
- **Argument Validation**: Fine-grained control over command arguments

### Core Modules
- **Policy Parser**: Parses and evaluates Starlark policy files
- **Command Validator**: Validates commands against defined policies
- **Default Policies**: Pre-defined security policies for common scenarios

## Main Functionality
1. **Policy Evaluation**: Evaluates commands against security policies before execution
2. **Pattern Matching**: Matches command patterns using regex and glob expressions
3. **Argument Filtering**: Validates and filters command arguments based on policy rules
4. **Error Reporting**: Provides detailed feedback on policy violations

## Dependencies
- `starlark`: Policy language interpreter
- `regex`: Pattern matching for command validation
- `serde_json`: JSON serialization for policy data

## Integration Points
- Used by `core` package for command execution validation
- Integrated with sandbox systems for secure execution
- Works with exec and TUI interfaces to enforce policies

## Security Features
- Prevents execution of dangerous commands
- Restricts access to sensitive file paths
- Controls network operations
- Manages process spawning permissions

## Configuration
Policies are defined in Starlark files and can be customized per:
- User preferences
- System requirements
- Application context
- Security level needed