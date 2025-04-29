# Codex CLI Tools Guide

This document describes the tools available in Codex CLI, including those migrated from Cline.

## Core Tools

### File Operations

#### `read_file`

- **Description**: Reads the contents of a file at the specified path.
- **Parameters**:
  - `path`: The path of the file to read (relative to the current working directory)
- **Usage**: Use when you need to examine existing files.

#### `write_to_file`

- **Description**: Creates or overwrites a file with specified content.
- **Parameters**:
  - `path`: The path of the file to write to
  - `content`: The content to write to the file
- **Usage**: Use for creating new files or completely replacing existing files.

#### `replace_in_file`

- **Description**: Makes targeted edits to specific parts of an existing file.
- **Parameters**:
  - `path`: The path of the file to modify
  - `diff`: One or more SEARCH/REPLACE blocks that define the changes
- **Usage**: Prefer for making incremental changes to existing files.

#### `search_files`

- **Description**: Performs a regex search across files in a specified directory.
- **Parameters**:
  - `path`: The path of the directory to search in
  - `regex`: The regular expression pattern to search for
  - `file_pattern`: (optional) Glob pattern to filter files
- **Usage**: Use for finding patterns or specific content across multiple files.

#### `list_files`

- **Description**: Lists files and directories within the specified directory.
- **Parameters**:
  - `path`: The path of the directory to list contents for
  - `recursive`: (optional) Whether to list files recursively
- **Usage**: Use to explore directory contents and understand project structure.

### System Operations

#### `execute_command`

- **Description**: Executes a CLI command on the system.
- **Parameters**:
  - `command`: The CLI command to execute
  - `requires_approval`: Whether this command requires explicit user approval
- **Usage**: Use for running system commands, building projects, or other CLI operations.

## Enhanced Tools (Migrated from Cline)

### Code Analysis

#### `list_code_definition_names`

- **Description**: Lists definition names (classes, functions, methods, etc.) in source code files.
- **Parameters**:
  - `path`: The path of the directory to list definitions for
- **Usage**: Use to get a high-level overview of code structure without reading each file individually.
- **Benefits**: Helps quickly understand the architecture and organization of code.

### Browser Interaction

#### `browser_action`

- **Description**: Interacts with a Puppeteer-controlled browser for testing web applications.
- **Parameters**:
  - `action`: The action to perform (launch, click, type, scroll_down, scroll_up, close)
  - `url`: (optional) The URL to navigate to (for launch action)
  - `coordinate`: (optional) The X and Y coordinates for click actions
  - `text`: (optional) The text to type (for type action)
- **Usage**: Use for testing web applications, automating browser interactions, and validating UI changes.
- **Important Notes**:
  - Always start with `launch` and end with `close`
  - Only one action can be performed per message
  - Browser window size is fixed at 900x600 pixels

### User Interaction

#### `ask_followup_question`

- **Description**: Asks the user a question to gather additional information.
- **Parameters**:
  - `question`: The question to ask the user
  - `options`: (optional) An array of options for the user to choose from
- **Usage**: Use when you need clarification or additional details to complete a task effectively.
- **Best Practices**: Keep questions specific and focused; provide options when appropriate.

#### `attempt_completion`

- **Description**: Presents the result of your work to the user.
- **Parameters**:
  - `result`: The result of the task
  - `command`: (optional) A CLI command to demonstrate the result
- **Usage**: Use when you have completed the user's task and want to present the final result.
- **Best Practices**: Provide comprehensive descriptions of what you've accomplished.

### MCP Integration

#### `use_mcp_tool`

- **Description**: Uses a tool provided by a connected MCP server.
- **Parameters**:
  - `server_name`: The name of the MCP server providing the tool
  - `tool_name`: The name of the tool to execute
  - `arguments`: A JSON object containing the tool's input parameters
- **Usage**: Use to access functionality provided by Model Context Protocol servers.

#### `access_mcp_resource`

- **Description**: Accesses a resource provided by a connected MCP server.
- **Parameters**:
  - `server_name`: The name of the MCP server providing the resource
  - `uri`: The URI identifying the specific resource to access
- **Usage**: Use to access data resources provided by MCP servers.

## Special-Purpose Tools

#### `plan_mode_respond`

- **Description**: Responds to the user in plan mode.
- **Parameters**:
  - `response`: The response to provide to the user
- **Usage**: Only available in PLAN MODE; use for planning a solution to the user's task.

#### `new_task`

- **Description**: Creates a new task with preloaded context.
- **Parameters**:
  - `context`: The context to preload the new task with
- **Usage**: Use when the user wants to create a new task while preserving context.

#### `condense`

- **Description**: Creates a summary of the conversation to compact the context window.
- **Parameters**:
  - `context`: The condensed context to continue with
- **Usage**: Use when the conversation history needs to be compacted.

## Tool Selection Guidelines

1. **Default to targeted tools**: Use `replace_in_file` for small changes instead of `write_to_file` for entire files.
2. **Use the right tool for exploration**: `list_code_definition_names` for code understanding, `search_files` for finding patterns.
3. **Browser interactions**: Always use `browser_action` sequentially with one action per message.
4. **User interaction**: Use `ask_followup_question` sparingly and only when necessary.
5. **Completion**: Always use `attempt_completion` to present final results.

## Provider Compatibility

All tools are supported across all model providers (OpenAI, Anthropic/Claude, and Gemini), with provider-specific prompt adaptations to ensure optimal performance.
