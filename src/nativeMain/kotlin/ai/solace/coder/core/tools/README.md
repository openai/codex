# Tool System Architecture

This document describes the comprehensive tool routing and Model Context Protocol (MCP) integration system implemented in Kotlin Native.

## Overview

The tool system provides a unified framework for executing various types of tool calls from AI model responses, including:

- **Built-in tools**: Shell command execution, container operations
- **MCP tools**: External tools provided by MCP servers
- **Custom tools**: User-defined tool implementations

## Core Components

### 1. ToolHandler Interface (`ToolHandler.kt`)

The base interface for all tool implementations:

```kotlin
interface ToolHandler {
    val kind: ToolKind
    fun matchesKind(payload: ToolPayload): Boolean
    suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput>
    fun getTimeoutMs(): Long
}
```

Key features:
- **Payload matching**: Determines which tool calls each handler can process
- **Timeout support**: Configurable timeouts per tool type
- **Cancellation support**: Proper handling of tool call cancellation
- **Error handling**: Comprehensive error reporting with CodexResult<T>

### 2. ToolRouter (`ToolRouter.kt`)

Central routing component that dispatches tool calls to appropriate handlers:

```kotlin
class ToolRouter(private val registry: ToolRegistry) {
    suspend fun dispatchToolCall(session: Session, turn: TurnContext, call: ToolCall): CodexResult<ResponseInputItem>
    suspend fun dispatchToolCalls(session: Session, turn: TurnContext, calls: List<ToolCall>): CodexResult<List<ResponseInputItem>>
}
```

Features:
- **Parallel execution**: Supports concurrent tool execution when possible
- **Tool discovery**: Automatic tool registration and discovery
- **Error aggregation**: Collects and reports multiple tool call failures
- **MCP integration**: Handles MCP tool naming conventions (mcp__server__tool)

### 3. Built-in Tool Handlers

#### ShellToolHandler (`ShellToolHandler.kt`)
Executes shell commands with proper sandboxing and approval handling:
- Supports both `shell` and `local_shell` payload types
- Command validation and safety checking
- Escalated permission handling
- Working directory resolution

#### ShellCommandToolHandler (`ShellCommandToolHandler.kt`)
Executes single shell commands through the user's shell:
- Automatic shell detection and command construction
- Login shell support
- Environment variable handling

#### ContainerExecToolHandler (`ContainerExecToolHandler.kt`)
Executes commands within Docker containers:
- Docker command construction
- Container isolation and security
- Environment and user configuration

### 4. MCP Integration

#### McpConnectionManager (`McpConnectionManager.kt`)
Manages connections to MCP servers:
- **Transport support**: Stdio and HTTP-based connections
- **Lifecycle management**: Start, stop, restart operations
- **Health monitoring**: Connection status tracking
- **Authentication**: Integration with auth manager

#### McpToolRegistry (`McpToolRegistry.kt`)
Discovers and caches MCP tools:
- **Tool filtering**: Server-specific allow/deny lists
- **Caching**: Efficient tool definition storage
- **Search capabilities**: Tool discovery by name/description
- **Statistics**: Tool usage and server metrics

#### McpAuthManager (`McpAuthManager.kt`)
Handles MCP server authentication:
- **OAuth support**: Token refresh and credential management
- **Bearer tokens**: Environment variable-based authentication
- **API keys**: Simple key-based authentication
- **Storage modes**: Memory, disk, and keychain storage

#### McpToolExecutor (`McpToolExecutor.kt`)
Executes MCP tool calls with proper error handling:
- **Retry logic**: Configurable retry policies
- **Timeout handling**: Per-call timeout management
- **Result formatting**: Structured output processing
- **Parallel execution**: Concurrent tool call support

### 5. Tool Call Processing

#### ToolCallProcessor (`ToolCallProcessor.kt`)
Processes function calls from model responses:
- **Response parsing**: Extracts tool calls from model responses
- **Validation**: Argument validation and type checking
- **Execution coordination**: Manages tool call execution flow
- **Result aggregation**: Combines multiple tool call results

#### ToolResultFormatter (`ToolResultFormatter.kt`)
Formats tool outputs for model consumption:
- **Content formatting**: Text, image, and structured data handling
- **Truncation**: Output size limiting
- **Sanitization**: Control character removal
- **Streaming support**: Chunked output for long-running operations

### 6. System Integration

#### ToolSystem (`ToolSystem.kt`)
Main integration point providing unified access to all tool functionality:
- **Component initialization**: Coordinated setup of all subsystems
- **Configuration management**: Centralized configuration handling
- **Statistics collection**: System-wide metrics and monitoring
- **Lifecycle management**: Proper startup and shutdown procedures

## Usage Examples

### Basic Tool System Setup

```kotlin
// Create process executor
val processExecutor = ProcessExecutor()

// Create tool system with default configuration
val toolSystem = ToolSystem(processExecutor)

// Initialize the system
val initResult = toolSystem.initialize()
if (initResult.isFailure) {
    println("Failed to initialize tool system: ${initResult.errorOrNull()?.message}")
    return
}

// Process model response items
val responseItems = listOf(/* model response items */)
val session = Session(/* session data */)
val turn = TurnContext(/* turn data */)

val results = toolSystem.processResponseItems(session, turn, responseItems)
if (results.isSuccess) {
    for (result in results.getOrThrow()) {
        println("Tool result: $result")
    }
}
```

### MCP Server Configuration

```kotlin
// Configure MCP servers
val mcpServers = mapOf(
    "github" to McpServerConfig(
        name = "github",
        enabled = true,
        transport = McpTransportConfig.Stdio(
            command = "mcp-server-github",
            args = listOf("--port", "8080")
        )
    ),
    "filesystem" to McpServerConfig(
        name = "filesystem",
        enabled = true,
        transport = McpTransportConfig.StreamableHttp(
            url = "http://localhost:3000/mcp",
            bearerTokenEnvVar = "FILESYSTEM_TOKEN"
        )
    )
)

// Initialize MCP servers
val mcpResult = toolSystem.initializeMcpServers(mcpServers)
if (mcpResult.isFailure) {
    println("Failed to initialize MCP servers: ${mcpResult.errorOrNull()?.message}")
}
```

### Custom Tool Handler

```kotlin
class CustomToolHandler : ToolHandler {
    override val kind: ToolKind = ToolKind.Custom
    
    override fun matchesKind(payload: ToolPayload): Boolean {
        return payload is ToolPayload.Custom
    }
    
    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val payload = invocation.payload as ToolPayload.Custom
        
        // Process custom tool input
        val result = processCustomInput(payload.input)
        
        return CodexResult.success(
            ToolOutput.Function(
                content = result,
                success = true
            )
        )
    }
    
    override fun getTimeoutMs(): Long = 30000L // 30 seconds
    
    private suspend fun processCustomInput(input: String): String {
        // Custom processing logic
        return "Processed: $input"
    }
}

// Register custom tool
val toolRegistry = /* get tool registry */
toolRegistry.register("custom_tool", CustomToolHandler())
```

## Configuration

### ToolSystemConfig

```kotlin
data class ToolSystemConfig(
    val enableParallelExecution: Boolean = false, // Disabled for Kotlin Native
    val maxConcurrentCalls: Int = 1,
    val defaultTimeoutMs: Long = 60000L,
    val enableMcp: Boolean = true,
    val enableBuiltInTools: Boolean = true
)
```

### ToolCallProcessorConfig

```kotlin
data class ToolCallProcessorConfig(
    val enableParallelExecution: Boolean = false,
    val maxConcurrentCalls: Int = 1,
    val defaultTimeoutMs: Long = 60000L
)
```

### McpToolExecutionConfig

```kotlin
data class McpToolExecutionConfig(
    val defaultTimeoutMs: Long = 60000L,
    val maxRetries: Int = 3,
    val retryDelayMs: Long = 1000L,
    val enableParallelExecution: Boolean = true
)
```

## Error Handling

The tool system uses `CodexResult<T>` for comprehensive error handling:

```kotlin
when (val result = toolSystem.processResponseItems(session, turn, items)) {
    is CodexResult.Success -> {
        val results = result.getOrThrow()
        // Process successful results
    }
    is CodexResult.Failure -> {
        val error = result.errorOrNull()
        println("Tool execution failed: ${error?.message}")
        // Handle error
    }
}
```

## Monitoring and Statistics

### System Statistics

```kotlin
val stats = toolSystem.getStatistics()
if (stats.isSuccess) {
    val systemStats = stats.getOrThrow()
    println("Built-in tools enabled: ${systemStats.builtInToolsEnabled}")
    println("MCP enabled: ${systemStats.mcpEnabled}")
    println("Total MCP tools: ${systemStats.mcpStats.totalTools}")
}
```

### Component Statistics

Each component provides detailed statistics:

```kotlin
// Tool call processor statistics
val processorStats = toolCallProcessor.getStatistics()

// MCP tool registry statistics
val mcpStats = mcpToolRegistry.getStatistics()

// Tool result formatter statistics
val formatterStats = toolResultFormatter.getStatistics()
```

## Security Considerations

### Command Validation

The system includes built-in safety checks for shell commands:
- Known safe command detection
- Escalated permission requirements
- Approval policy enforcement

### Sandbox Integration

All tool executions are properly sandboxed using the existing sandbox infrastructure:
- Seatbelt integration on macOS
- Seccomp integration on Linux
- Restricted token integration on Windows

### MCP Security

MCP connections support secure authentication:
- OAuth token management
- Bearer token handling
- API key authentication

## Performance Considerations

### Sequential vs Parallel Execution

For Kotlin Native, parallel execution is disabled by default due to platform limitations:
```kotlin
val config = ToolSystemConfig(
    enableParallelExecution = false, // Recommended for Kotlin Native
    maxConcurrentCalls = 1
)
```

### Timeout Management

Configurable timeouts at multiple levels:
- System-wide default timeout
- Tool-specific timeouts
- Per-call timeout overrides

### Memory Management

The system is designed for efficient memory usage:
- Streaming output for long-running operations
- Output truncation to prevent memory bloat
- Proper cleanup of resources

## Testing

### Unit Tests

Each component should have comprehensive unit tests covering:
- Normal operation scenarios
- Error conditions
- Edge cases
- Timeout handling

### Integration Tests

Integration tests should verify:
- End-to-end tool call processing
- MCP server connectivity
- Multi-tool execution scenarios
- Error propagation

### Mock MCP Servers

For testing, use mock MCP servers to:
- Test MCP integration without external dependencies
- Verify error handling and retry logic
- Test authentication flows

## Future Enhancements

### Additional Tool Handlers

Potential future tool handlers:
- Database query tools
- File system operations
- Network request tools
- Code analysis tools

### Enhanced MCP Features

Future MCP enhancements:
- Tool capability negotiation
- Streaming tool outputs
- Tool composition
- Tool versioning

### Performance Optimizations

Potential performance improvements:
- Connection pooling for MCP servers
- Tool result caching
- Lazy tool initialization
- Background tool preloading

## Troubleshooting

### Common Issues

1. **Tool execution timeouts**: Increase timeout values or check tool performance
2. **MCP connection failures**: Verify server configuration and authentication
3. **Permission denied errors**: Check approval policy and escalated permissions
4. **Memory usage**: Monitor output sizes and enable truncation

### Debug Logging

Enable debug logging for troubleshooting:
```kotlin
// In development builds
println("Tool system debug: $debugInfo")
```

### Error Recovery

The system includes automatic error recovery:
- Retry logic for transient failures
- Graceful degradation for unavailable tools
- Error isolation to prevent cascading failures