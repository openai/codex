# mcp-client Package Summary

## Purpose
Client library for communicating with Model Context Protocol (MCP) servers. Enables Codex to connect to and interact with external MCP-compliant tools and services.

## Key Components

### Client Implementation
- **Protocol Client**: Full MCP protocol client implementation
- **Connection Management**: Handles server connections and reconnections
- **Message Handling**: Serialization/deserialization of MCP messages

### Communication Layer
- **Async Operations**: Non-blocking communication with servers
- **Request/Response**: Manages request-response cycles
- **Event Streaming**: Handles server-initiated events

## Main Functionality
1. **Server Discovery**: Locate and connect to MCP servers
2. **Tool Invocation**: Call tools exposed by MCP servers
3. **Resource Access**: Retrieve resources from servers
4. **Protocol Negotiation**: Handle capability negotiation
5. **Error Recovery**: Automatic retry and error handling

## Dependencies
- `mcp-types`: Protocol type definitions
- `tokio`: Async runtime for network operations
- `serde_json`: JSON message serialization
- Standard networking libraries

## Integration Points
- Used by `core` to access external tools
- Works with `mcp-types` for message formats
- Integrates with authentication systems
- Connects to various MCP server implementations

## Protocol Features
- **JSON-RPC 2.0**: Standard RPC protocol
- **Bidirectional Communication**: Client and server initiated messages
- **Tool Discovery**: Dynamic tool enumeration
- **Resource Management**: Access to server-side resources
- **Capability Negotiation**: Feature detection and versioning

## Use Cases
- **External Tool Integration**: Access external development tools
- **Service Connectivity**: Connect to cloud services
- **Plugin System**: Extend Codex with MCP servers
- **Cross-platform Tools**: Access tools regardless of implementation

## Connection Types
- Standard I/O pipes
- TCP/IP sockets
- WebSocket connections
- Named pipes (platform-specific)