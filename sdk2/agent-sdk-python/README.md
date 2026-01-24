# Codex Agent SDK for Python

Python SDK for Codex Agent.

## Installation

```bash
pip install codex-agent-sdk
```

**Prerequisites:**

- Python 3.10+

**Note:** The Codex CLI is automatically bundled with the package - no separate installation required! The SDK will use the bundled CLI by default. If you prefer to use a system-wide installation or a specific version, you can:

- Install Codex separately
- Specify a custom path: `CodexAgentOptions(cli_path="/path/to/codex")`

## Quick Start

```python
import anyio
from agent_sdk import query

async def main():
    async for message in query(prompt="What is 2 + 2?"):
        print(message)

anyio.run(main)
```

## Basic Usage: query()

`query()` is an async function for querying Codex. It returns an `AsyncIterator` of response messages. See [src/agent_sdk/query.py](src/agent_sdk/query.py).

```python
from agent_sdk import query, CodexAgentOptions, AssistantMessage, TextBlock

# Simple query
async for message in query(prompt="Hello Codex"):
    if isinstance(message, AssistantMessage):
        for block in message.content:
            if isinstance(block, TextBlock):
                print(block.text)

# With options
options = CodexAgentOptions(
    system_prompt="You are a helpful assistant",
    max_turns=1
)

async for message in query(prompt="Tell me a joke", options=options):
    print(message)
```

### Using Tools

```python
options = CodexAgentOptions(
    allowed_tools=["Read", "Write", "Bash"],
    permission_mode='acceptEdits'  # auto-accept file edits
)

async for message in query(
    prompt="Create a hello.py file",
    options=options
):
    # Process tool use and results
    pass
```

### Working Directory

```python
from pathlib import Path

options = CodexAgentOptions(
    cwd="/path/to/project"  # or Path("/path/to/project")
)
```

## CodexSDKClient

`CodexSDKClient` supports bidirectional, interactive conversations with Codex. See [src/agent_sdk/client.py](src/agent_sdk/client.py).

Unlike `query()`, `CodexSDKClient` additionally enables **custom tools** and **hooks**, both of which can be defined as Python functions.

### Custom Tools (as In-Process SDK MCP Servers)

A **custom tool** is a Python function that you can offer to Codex, for Codex to invoke as needed.

Custom tools are implemented in-process MCP servers that run directly within your Python application, eliminating the need for separate processes that regular MCP servers require.

For an end-to-end example, see [MCP Calculator](examples/mcp_calculator.py).

#### Creating a Simple Tool

```python
from agent_sdk import tool, create_sdk_mcp_server, CodexAgentOptions, CodexSDKClient

# Define a tool using the @tool decorator
@tool("greet", "Greet a user", {"name": str})
async def greet_user(args):
    return {
        "content": [
            {"type": "text", "text": f"Hello, {args['name']}!"}
        ]
    }

# Create an SDK MCP server
server = create_sdk_mcp_server(
    name="my-tools",
    version="1.0.0",
    tools=[greet_user]
)

# Use it with Codex
options = CodexAgentOptions(
    mcp_servers={"tools": server},
    allowed_tools=["mcp__tools__greet"]
)

async with CodexSDKClient(options=options) as client:
    await client.query("Greet Alice")

    # Extract and print response
    async for msg in client.receive_response():
        print(msg)
```

#### Benefits Over External MCP Servers

- **No subprocess management** - Runs in the same process as your application
- **Better performance** - No IPC overhead for tool calls
- **Simpler deployment** - Single Python process instead of multiple
- **Easier debugging** - All code runs in the same process
- **Type safety** - Direct Python function calls with type hints

#### Migration from External Servers

```python
# BEFORE: External MCP server (separate process)
options = CodexAgentOptions(
    mcp_servers={
        "calculator": {
            "type": "stdio",
            "command": "python",
            "args": ["-m", "calculator_server"]
        }
    }
)

# AFTER: SDK MCP server (in-process)
from my_tools import add, subtract  # Your tool functions

calculator = create_sdk_mcp_server(
    name="calculator",
    tools=[add, subtract]
)

options = CodexAgentOptions(
    mcp_servers={"calculator": calculator}
)
```

#### Mixed Server Support

You can use both SDK and external MCP servers together:

```python
options = CodexAgentOptions(
    mcp_servers={
        "internal": sdk_server,      # In-process SDK server
        "external": {                # External subprocess server
            "type": "stdio",
            "command": "external-server"
        }
    }
)
```

### Hooks

A **hook** is a Python function that the Codex _application_ (_not_ the model) invokes at specific points of the agent loop. Hooks can provide deterministic processing and automated feedback for Codex.

For more examples, see examples/hooks.py.

#### Example

```python
from agent_sdk import CodexAgentOptions, CodexSDKClient, HookMatcher

async def check_bash_command(input_data, tool_use_id, context):
    tool_name = input_data["tool_name"]
    tool_input = input_data["tool_input"]
    if tool_name != "Bash":
        return {}
    command = tool_input.get("command", "")
    block_patterns = ["foo.sh"]
    for pattern in block_patterns:
        if pattern in command:
            return {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": f"Command contains invalid pattern: {pattern}",
                }
            }
    return {}

options = CodexAgentOptions(
    allowed_tools=["Bash"],
    hooks={
        "PreToolUse": [
            HookMatcher(matcher="Bash", hooks=[check_bash_command]),
        ],
    }
)

async with CodexSDKClient(options=options) as client:
    # Test 1: Command with forbidden pattern (will be blocked)
    await client.query("Run the bash command: ./foo.sh --help")
    async for msg in client.receive_response():
        print(msg)

    print("\n" + "=" * 50 + "\n")

    # Test 2: Safe command that should work
    await client.query("Run the bash command: echo 'Hello from hooks example!'")
    async for msg in client.receive_response():
        print(msg)
```

## Types

See [src/agent_sdk/types.py](src/agent_sdk/types.py) for complete type definitions:

- `CodexAgentOptions` - Configuration options
- `AssistantMessage`, `UserMessage`, `SystemMessage`, `ResultMessage` - Message types
- `TextBlock`, `ToolUseBlock`, `ToolResultBlock` - Content blocks

## Error Handling

```python
from agent_sdk import (
    CodexSDKError,      # Base error
    CLINotFoundError,    # Codex not installed
    CLIConnectionError,  # Connection issues
    ProcessError,        # Process failed
    CLIJSONDecodeError,  # JSON parsing issues
)

try:
    async for message in query(prompt="Hello"):
        pass
except CLINotFoundError:
    print("Please install Codex")
except ProcessError as e:
    print(f"Process failed with exit code: {e.exit_code}")
except CLIJSONDecodeError as e:
    print(f"Failed to parse response: {e}")
```

See [src/agent_sdk/\_errors.py](src/agent_sdk/_errors.py) for all error types.

## Available Tools

See the Codex documentation for a complete list of available tools.

## Examples

See [examples/quick_start.py](examples/quick_start.py) for a complete working example.

See [examples/streaming_mode.py](examples/streaming_mode.py) for comprehensive examples involving `CodexSDKClient`. You can even run interactive examples in IPython from [examples/streaming_mode_ipython.py](examples/streaming_mode_ipython.py).

## Development

If you're contributing to this project, run the initial setup script to install git hooks:

```bash
./scripts/initial-setup.sh
```

This installs a pre-push hook that runs lint checks before pushing, matching the CI workflow. To skip the hook temporarily, use `git push --no-verify`.

### Building Wheels Locally

To build wheels with the bundled Codex CLI:

```bash
# Install build dependencies
pip install build twine

# Build wheel with bundled CLI
python scripts/build_wheel.py

# Build with specific version
python scripts/build_wheel.py --version 0.1.4

# Build with specific CLI version
python scripts/build_wheel.py --cli-version 2.0.0

# Clean bundled CLI after building
python scripts/build_wheel.py --clean

# Skip CLI download (use existing)
python scripts/build_wheel.py --skip-download
```

The build script:

1. Downloads Codex CLI for your platform
2. Bundles it in the wheel
3. Builds both wheel and source distribution
4. Checks the package with twine

See `python scripts/build_wheel.py --help` for all options.

### Release Workflow

The package is published to PyPI via the GitHub Actions workflow in `.github/workflows/publish.yml`. To create a new release:

1. **Trigger the workflow** manually from the Actions tab with two inputs:
   - `version`: The package version to publish (e.g., `0.1.5`)
   - `codex_version`: The Codex CLI version to bundle (e.g., `2.0.0` or `latest`)

2. **The workflow will**:
   - Build platform-specific wheels for macOS, Linux, and Windows
   - Bundle the specified Codex CLI version in each wheel
   - Build a source distribution
   - Publish all artifacts to PyPI
   - Create a release branch with version updates
   - Open a PR to main with:
     - Updated `pyproject.toml` version
     - Updated `src/agent_sdk/_version.py`
     - Updated `src/agent_sdk/_cli_version.py` with bundled CLI version
     - Auto-generated `CHANGELOG.md` entry

3. **Review and merge** the release PR to update main with the new version information

The workflow tracks both the package version and the bundled CLI version separately, allowing you to release a new package version with an updated CLI without code changes.

## License and terms

Use of this SDK is governed by applicable terms of service.
