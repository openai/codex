# mcp-server Package Summary

## Purpose
Model Context Protocol server implementation that exposes Codex functionality as an MCP service. Allows external applications to integrate with Codex capabilities via standardized protocol.

## Key Components

### Server Implementation
- **JSON-RPC Handler**: Processes incoming MCP requests
- **Tool Registry**: Manages and exposes available tools
- **Resource Provider**: Serves resources to clients

### Integration Layer
- **Codex Bridge**: Connects MCP requests to Codex core functionality
- **Approval Workflows**: Manages permission and approval flows
- **Configuration Management**: Server setup and configuration

## Main Functionality
1. **Tool Exposure**: Makes Codex tools available via MCP
2. **Request Processing**: Handles tool invocation requests
3. **Resource Serving**: Provides access to Codex resources
4. **Session Management**: Maintains client sessions
5. **Security Enforcement**: Applies security policies to requests

## Dependencies
- `core`: Core Codex functionality
- `mcp-types`: Protocol type definitions
- `tokio`: Async server runtime
- `serde_json`: Message serialization
- Server infrastructure libraries

## Integration Points
- Exposes `core` functionality via MCP
- Uses `execpolicy` for security validation
- Integrates with authentication systems
- Works with approval mechanisms

## Server Capabilities
- **Tool Registration**: Dynamic tool advertisement
- **Multi-client Support**: Handle multiple concurrent clients
- **Stateful Sessions**: Maintain conversation context
- **Resource Management**: Serve files and data
- **Event Streaming**: Push events to clients

## Protocol Implementation
- **MCP Compliance**: Full protocol specification support
- **Version Negotiation**: Handle different protocol versions
- **Error Handling**: Proper error responses
- **Async Operations**: Non-blocking request processing

## Use Cases
- **IDE Integration**: Provide Codex features to IDEs
- **Tool Ecosystem**: Part of larger MCP tool ecosystem
- **Service Architecture**: Microservice deployment
- **Remote Access**: Access Codex over network

## Configuration Options
- Server address and port
- Authentication requirements
- Tool permissions
- Resource access controls
- Approval policies