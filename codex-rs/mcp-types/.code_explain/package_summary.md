# mcp-types Package Summary

## Purpose
Auto-generated type definitions for the Model Context Protocol. Provides complete, type-safe Rust and TypeScript representations of all MCP protocol messages and data structures.

## Key Components

### Type Definitions
- **Protocol Messages**: All MCP message types
- **JSON-RPC Types**: Request, response, and error types
- **Data Structures**: Tools, resources, and capabilities

### Code Generation
- **TypeScript Support**: Generates TypeScript definitions via ts-rs
- **Serialization**: Serde implementations for all types
- **Validation**: Type-safe protocol compliance

## Main Functionality
1. **Type Safety**: Compile-time protocol validation
2. **Serialization**: JSON serialization/deserialization
3. **Cross-language Support**: Rust and TypeScript types
4. **Protocol Versioning**: Support for protocol evolution
5. **Documentation**: Generated from protocol specification

## Dependencies
- `serde`: Serialization framework
- `serde_json`: JSON support
- `ts-rs`: TypeScript generation
- Basic type libraries

## Integration Points
- Used by `mcp-client` for client implementation
- Used by `mcp-server` for server implementation
- Foundation for all MCP communication
- TypeScript types for web frontends

## Type Categories

### Core Protocol
- Initialize/Initialized
- Request/Response/Error
- Notification messages
- Capability negotiation

### Tools
- Tool definitions
- Tool invocation requests
- Tool results and errors
- Parameter schemas

### Resources
- Resource definitions
- Resource templates
- Resource retrieval
- Resource updates

### Transport
- Message framing
- Connection management
- Protocol negotiation
- Version handling

## Generation Process
- Generated from MCP specification
- Automated updates with protocol changes
- Backward compatibility considerations
- Version-specific type sets

## Usage Patterns
- Import for type-safe MCP implementation
- Use for message validation
- Reference for protocol compliance
- Base for custom extensions