# Multi-Agent Orchestration System

Codex CLI includes a powerful multi-agent orchestration system that allows you to invoke specialized AI agents for complex tasks. Each agent can be customized with specific system prompts while inheriting tools and permissions from the parent context.

## Overview

The agent system enables you to:

- Define specialized agents with custom system prompts
- Invoke agents using natural `@agent` mention syntax
- Automatically track agent tasks in the plan system
- View real-time agent execution progress
- Maintain context isolation between agent executions
- Prevent recursive agent spawning for safety
- Get comprehensive summaries of agent work

## Quick Start

### Using @Agent Mentions (Recommended)

The most natural way to invoke agents is using the `@agent` mention syntax:

```
@researcher: find information about React hooks

@code-reviewer: review the changes in src/

@test-writer: create unit tests for the new functions
```

When you use `@agent` mentions:

- The agent task is automatically added to the plan with "in_progress" status
- You see real-time progress updates during execution
- The plan updates to "completed" when the agent finishes

### Using the Agent Tool

You can also invoke agents programmatically with the `agent` tool:

```
Please use the researcher agent to find information about React hooks

Use the code-reviewer agent to review the changes in src/

Have the test-writer agent create unit tests for the new functions
```

### Viewing Available Agents

Use the `/agents` command to see all available agents:

```
/agents
```

This displays:

- Built-in agents (marked with •)
- Custom agents from your configuration (marked with ◦)
- Brief descriptions of each agent's purpose

## Built-in Agents

Codex comes with one built-in agent:

- **`general`** - A general-purpose AI assistant for completing tasks efficiently and accurately

## Custom Agent Configuration

Create custom agents by adding a configuration file at `~/.codex/agents.toml`:

```toml
# ~/.codex/agents.toml

[researcher]
prompt = """
You are a research specialist. Your role is to:
- Gather comprehensive information from multiple sources
- Verify facts and cross-reference findings
- Provide detailed citations and sources
- Summarize findings in a structured format
Focus on accuracy and thoroughness over speed.
"""

[code-reviewer]
prompt = """
You are an expert code reviewer. Your responsibilities:
- Identify potential bugs and security issues
- Suggest performance improvements
- Ensure code follows best practices
- Check for proper error handling
- Verify test coverage
Provide constructive feedback with specific examples.
"""

[test-writer]
prompt_file = "prompts/test-writer.md"  # Load from external file

[refactorer]
prompt = """
You are a refactoring specialist. Focus on:
- Improving code readability and maintainability
- Reducing complexity and duplication
- Applying design patterns appropriately
- Ensuring backward compatibility
Always explain the reasoning behind refactoring decisions.
"""
tools = ["read", "write", "grep"]  # Optional: override available tools

[documenter]
prompt = """
You are a documentation expert. Your tasks:
- Write clear, comprehensive documentation
- Create useful code examples
- Maintain consistent formatting
- Include API references
- Add helpful diagrams when appropriate
"""
permissions = "readonly"  # Optional: override permissions
```

## Configuration Options

Each agent supports the following configuration options:

| Field         | Type   | Description                                                           |
| ------------- | ------ | --------------------------------------------------------------------- |
| `prompt`      | String | The system prompt that defines the agent's behavior                   |
| `prompt_file` | String | Path to a file containing the prompt (alternative to inline `prompt`) |
| `tools`       | Array  | Optional: Override the available tools for this agent                 |
| `permissions` | String | Optional: Override the permission level for this agent                |

### Prompt Files

For longer prompts, you can store them in separate files:

```toml
[complex-agent]
prompt_file = "prompts/complex-agent.md"  # Relative to ~/.codex/
```

Or use absolute paths:

```toml
[complex-agent]
prompt_file = "/home/user/my-prompts/complex-agent.md"
```

## Visual Feedback and Plan Integration

### Real-Time Status Indicators

When agents execute, you'll see visual feedback:

- **⚡ Running** (yellow) - Agent is currently executing
- **⟳** Progress loops - Iterative steps the agent is performing
- **📝** File changes - Files being modified
- **💬** Outputs - Agent responses and analysis
- **🔧** Tool usage - Tools being invoked
- **✓ Done** (green) - Agent completed successfully

### Automatic Plan Tracking

Every agent invocation automatically creates a plan item:

1. **Plan Creation**: When you use `@agent: task`, a plan item is created
2. **Status Tracking**: Plan shows "in_progress" during execution
3. **Completion Update**: Plan updates to "completed" when done
4. **Linked Execution**: Plan items are linked to agent events via `plan_item_id`

Example workflow:

```
User: @researcher: find async Rust patterns
System: [Creates plan item: "@researcher: find async Rust patterns" - in_progress]
System: ⚡ Running researcher
System: ⟳ [researcher] Analyzing task requirements
System: ⟳ [researcher] Searching for documentation
System: ✓ Done researcher (3.2s)
System: [Updates plan item to completed]
```

## Agent Behavior

### Context Inheritance

By default, agents inherit:

- All available tools from the parent context
- Permission levels from the parent context
- Working directory and environment variables

This ensures agents have the same capabilities as the main conversation while maintaining isolation.

### Recursion Prevention

To prevent infinite loops and resource exhaustion, agents **cannot spawn other agents**. If an agent attempts to use the `agent` tool, it will receive an error message.

### Execution Isolation

Each agent execution:

- Runs in an isolated conversation context
- Cannot access the parent conversation history
- Returns a comprehensive summary to the parent
- Tracks all file changes and execution loops

## Agent Summaries

When an agent completes its task, it provides a structured summary including:

1. **Execution Loops** - Key steps and iterations performed
2. **File Changes** - All files created, modified, or deleted
3. **Key Outputs** - Important results (auto-compacted for long outputs)
4. **Final Summary** - Overall accomplishment and any recommendations

Long outputs are automatically compacted to show the first 5 and last 3 lines with a truncation indicator.

## Best Practices

### 1. Specialized Prompts

Create focused agents with clear responsibilities:

```toml
[security-auditor]
prompt = """
You are a security specialist focused exclusively on:
- Identifying vulnerabilities (SQL injection, XSS, etc.)
- Checking authentication and authorization
- Reviewing encryption and data protection
- Analyzing dependencies for known CVEs
Do not fix issues, only identify and report them.
"""
```

### 2. Tool Restrictions

Limit tools for safety when appropriate:

```toml
[analyzer]
tools = ["read", "grep", "glob"]  # Read-only analysis
prompt = "You are a code analyzer. Examine code without making changes..."
```

### 3. Composable Agents

Design agents that work well together:

```toml
[planner]
prompt = "Create detailed implementation plans with clear steps..."

[implementer]
prompt = "Execute implementation plans step by step..."

[validator]
prompt = "Verify implementations meet requirements..."
```

### 4. Prompt Engineering

Structure prompts for clarity:

```toml
[api-designer]
prompt = """
Role: API Design Specialist

Responsibilities:
- Design RESTful APIs following OpenAPI specification
- Ensure consistent naming conventions
- Include proper error responses
- Document all endpoints thoroughly

Constraints:
- Follow REST best practices
- Use semantic versioning
- Include rate limiting considerations

Output Format:
- OpenAPI 3.0 specification
- Implementation examples
- Testing strategies
"""
```

## Examples

### Research Agent

```toml
[researcher]
prompt = """
You are a meticulous researcher. For any topic:
1. Start with a broad overview
2. Identify authoritative sources
3. Deep dive into specific aspects
4. Cross-reference claims
5. Summarize with citations
Always distinguish between facts and opinions.
"""
```

Usage: "Use the researcher agent to investigate WebAssembly performance characteristics"

### Migration Agent

```toml
[migrator]
prompt = """
You are a migration specialist. When migrating code:
1. Analyze the current implementation
2. Identify breaking changes
3. Create a migration plan
4. Implement incrementally
5. Verify backward compatibility
6. Update documentation
Prioritize safety and reversibility.
"""
```

Usage: "Have the migrator agent help upgrade our React 17 code to React 18"

### Performance Agent

```toml
[performance-optimizer]
prompt = """
You are a performance optimization expert:
- Profile code to identify bottlenecks
- Suggest algorithmic improvements
- Optimize resource usage
- Reduce unnecessary computations
- Implement caching strategies
Always measure before and after changes.
"""
tools = ["read", "write", "exec"]
```

Usage: "Use the performance-optimizer agent to improve the data processing pipeline"

## Troubleshooting

### Agent Not Found

If an agent isn't recognized:

1. Check that `~/.codex/agents.toml` exists
2. Verify the agent name matches exactly (case-sensitive)
3. Ensure the TOML syntax is valid
4. Check file permissions

### Prompt File Not Loading

If using `prompt_file`:

1. Verify the file path is correct
2. Check file permissions
3. Use absolute paths if relative paths aren't working
4. Ensure the file contains valid text

### Agent Recursion Error

If you see "Agents cannot spawn other agents":

- This is by design to prevent infinite loops
- Restructure your task to avoid nested agent calls
- Use a single agent for the entire task

## Advanced Configuration

### Environment-Specific Agents

Create different agent sets for different environments:

```bash
# Development agents
cp ~/.codex/agents.toml ~/.codex/agents.dev.toml

# Production agents
cp ~/.codex/agents.toml ~/.codex/agents.prod.toml

# Symlink based on environment
ln -sf ~/.codex/agents.dev.toml ~/.codex/agents.toml
```

### Team Sharing

Share agent configurations with your team:

```bash
# Add to version control
git add .codex/agents.toml
git commit -m "Add team agent configurations"

# Team members can then:
cp project/.codex/agents.toml ~/.codex/agents.toml
```

### Dynamic Loading

Agents are loaded at runtime, so you can modify `~/.codex/agents.toml` without restarting Codex. Changes take effect on the next agent invocation.

## Limitations

- Agents cannot spawn other agents (recursion prevention)
- Agent context is isolated from parent conversation
- Maximum execution time follows parent timeout settings
- Tool availability depends on parent configuration

## Future Enhancements

Planned improvements for the agent system:

- Agent templates and inheritance
- Conditional agent selection based on task analysis
- Agent performance metrics and analytics
- Collaborative multi-agent workflows
- Agent versioning and rollback capabilities

## See Also

- [Configuration Guide](./config.md) - General Codex configuration
- [Model Context Protocol](./advanced.md#model-context-protocol-mcp) - MCP server integration
- [Custom Prompts](./prompts.md) - System prompt customization
