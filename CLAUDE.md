# Claude Provider Implementation Guidelines

## Clean Implementation Principles

When implementing Claude provider support, follow these guidelines to ensure clean, maintainable code:

1. **Avoid Unnecessary Optionality**
   - Don't add environment variable flags that change behavior significantly
   - Don't add conditional logic that creates multiple code paths
   - Example to avoid: `if (process.env.USE_MOCK === "true") { ... } else { ... }`

2. **Use Direct Dependencies**
   - Use the official Anthropic SDK directly
   - Don't create abstraction layers that aren't needed
   - Follow the provider interface contract cleanly

3. **Testing Strategy**
   - Create dedicated mock classes for testing
   - Use dependency injection in tests rather than runtime switches
   - Create separate test files for different scenarios

4. **Error Handling**
   - Handle Claude-specific errors explicitly
   - Don't try to make errors look like OpenAI errors
   - Ensure error messages are clear about their source

5. **Configuration**
   - Use clear, specific configuration properties
   - Follow the established pattern for API keys and URLs
   - Document each configuration option

## Implementation Notes

The Claude provider implements the `LLMProvider` interface, which means it must adapt the Anthropic API to match the interface OpenAI already implements. This includes:

- Converting messages between formats
- Adapting streaming responses
- Mapping tool calls to function calls
- Converting errors to a common format

For testing purposes, we have a separate `LLMMock` class that can be used in tests, but the actual implementation should always use the real Anthropic SDK directly.