# arg0 Package Summary

## Purpose
Binary dispatch system allowing a single executable to behave as multiple tools based on how it's invoked (argv[0]). Enables the creation of multi-tool binaries like busybox.

## Key Components

### Dispatch System
- **argv[0] Detection**: Identify invocation name
- **Tool Routing**: Route to appropriate tool
- **Fallback Handling**: Default behavior
- **Alias Support**: Multiple names per tool

### Environment Setup
- **PATH Management**: Ensure tool availability
- **Environment Variables**: Set up tool environment
- **Working Directory**: Manage CWD
- **Configuration**: Tool-specific configs

### Runtime Management
- **Tokio Runtime**: Async runtime setup
- **Thread Pools**: Configure parallelism
- **Resource Limits**: Set resource constraints
- **Signal Handling**: Process signals

### Tool Registry
- **Tool Registration**: Register available tools
- **Name Mapping**: Map names to implementations
- **Version Info**: Tool version management
- **Help Generation**: Auto-generate help

## Main Functionality
1. **Multi-tool Binary**: Single binary, multiple tools
2. **Name-based Dispatch**: Route by invocation name
3. **Environment Setup**: Prepare execution context
4. **Runtime Management**: Initialize async runtime
5. **Tool Integration**: Seamless tool invocation

## Dependencies
- `tokio`: Async runtime
- Integration with core package
- Tool implementations (apply-patch, etc.)
- Environment utilities

## Integration Points
- Entry point for main binaries
- Routes to `apply-patch` and others
- Manages `tokio` runtime
- Sets up tool environments

## Dispatch Mechanism

### Name Resolution
1. Check argv[0]
2. Match against registry
3. Select tool implementation
4. Set up environment
5. Execute tool

### Tool Registration
- Static registration
- Dynamic discovery
- Alias configuration
- Priority ordering
- Conflict resolution

### Execution Flow
1. Parse invocation
2. Initialize runtime
3. Set up environment
4. Route to tool
5. Handle result

## Use Cases
- **Single Binary Distribution**: One file, many tools
- **Symlink Tools**: Create tool symlinks
- **Shell Integration**: Shell command aliases
- **Container Images**: Minimal container size
- **Embedded Systems**: Reduced storage

## Supported Tools
- `codex`: Main CLI interface
- `apply-patch`: Patch application tool
- Future tools can be added
- Custom tool integration
- Plugin architecture (future)

## Configuration

### Runtime Options
- Thread pool size
- Stack size
- Async runtime flavor
- Blocking thread pool
- Timer resolution

### Environment Setup
- PATH modifications
- Environment variables
- Resource limits
- Signal handlers
- Process attributes

## Benefits
- **Reduced Size**: Single binary vs multiple
- **Atomic Updates**: Update all tools at once
- **Simplified Distribution**: One file to manage
- **Consistent Versioning**: All tools same version
- **Memory Efficiency**: Shared code segments

## Implementation Details
- Zero-cost abstraction
- Compile-time dispatch
- Static linking
- Minimal overhead
- Fast startup

## Platform Support
- Linux
- macOS
- Windows
- BSD variants
- Cross-compilation