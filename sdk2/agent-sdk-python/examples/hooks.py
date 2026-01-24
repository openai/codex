#!/usr/bin/env python
"""Example of using hooks with Codex SDK via CodexAgentOptions.

This file demonstrates various hook patterns using the hooks parameter
in CodexAgentOptions instead of decorator-based hooks.

Usage:
./examples/hooks.py - List the examples
./examples/hooks.py all - Run all examples
./examples/hooks.py PreToolUse - Run a specific example
"""

import asyncio
import logging
import sys
from typing import Any

from agent_sdk import CodexAgentOptions, CodexSDKClient
from agent_sdk.types import (
    AssistantMessage,
    HookContext,
    HookInput,
    HookJSONOutput,
    HookMatcher,
    Message,
    ResultMessage,
    TextBlock,
)

# Set up logging to see what's happening
logging.basicConfig(level=logging.INFO, format="%(asctime)s - %(message)s")
logger = logging.getLogger(__name__)


def display_message(msg: Message) -> None:
    """Standardized message display function."""
    if isinstance(msg, AssistantMessage):
        for block in msg.content:
            if isinstance(block, TextBlock):
                print(f"Codex: {block.text}")
    elif isinstance(msg, ResultMessage):
        print("Result ended")


##### Hook callback functions
async def check_bash_command(
    input_data: HookInput, tool_use_id: str | None, context: HookContext
) -> HookJSONOutput:
    """Prevent certain bash commands from being executed."""
    tool_name = input_data["tool_name"]
    tool_input = input_data["tool_input"]

    if tool_name != "Bash":
        return {}

    command = tool_input.get("command", "")
    block_patterns = ["foo.sh"]

    for pattern in block_patterns:
        if pattern in command:
            logger.warning(f"Blocked command: {command}")
            return {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": f"Command contains invalid pattern: {pattern}",
                }
            }

    return {}


async def add_custom_instructions(
    input_data: HookInput, tool_use_id: str | None, context: HookContext
) -> HookJSONOutput:
    """Add custom instructions when a session starts."""
    return {
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": "My favorite color is hot pink",
        }
    }


async def review_tool_output(
    input_data: HookInput, tool_use_id: str | None, context: HookContext
) -> HookJSONOutput:
    """Review tool output and provide additional context or warnings."""
    tool_response = input_data.get("tool_response", "")

    # If the tool produced an error, add helpful context
    if "error" in str(tool_response).lower():
        return {
            "systemMessage": "⚠️ The command produced an error",
            "reason": "Tool execution failed - consider checking the command syntax",
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": "The command encountered an error. You may want to try a different approach.",
            }
        }

    return {}


async def strict_approval_hook(
    input_data: HookInput, tool_use_id: str | None, context: HookContext
) -> HookJSONOutput:
    """Demonstrates using permissionDecision to control tool execution."""
    tool_name = input_data.get("tool_name")
    tool_input = input_data.get("tool_input", {})

    # Block any Write operations to specific files
    if tool_name == "Write":
        file_path = tool_input.get("file_path", "")
        if "important" in file_path.lower():
            logger.warning(f"Blocked Write to: {file_path}")
            return {
                "reason": "Writes to files containing 'important' in the name are not allowed for safety",
                "systemMessage": "🚫 Write operation blocked by security policy",
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": "Security policy blocks writes to important files",
                },
            }

    # Allow everything else explicitly
    return {
        "reason": "Tool use approved after security review",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "permissionDecisionReason": "Tool passed security checks",
        },
    }


async def stop_on_error_hook(
    input_data: HookInput, tool_use_id: str | None, context: HookContext
) -> HookJSONOutput:
    """Demonstrates using continue=False to stop execution on certain conditions."""
    tool_response = input_data.get("tool_response", "")

    # Stop execution if we see a critical error
    if "critical" in str(tool_response).lower():
        logger.error("Critical error detected - stopping execution")
        return {
            "continue_": False,
            "stopReason": "Critical error detected in tool output - execution halted for safety",
            "systemMessage": "🛑 Execution stopped due to critical error",
        }

    return {"continue_": True}


async def example_pretooluse() -> None:
    """Basic example demonstrating hook protection."""
    print("=== PreToolUse Example ===")
    print("This example demonstrates how PreToolUse can block some bash commands but not others.\n")

    # Configure hooks using CodexAgentOptions
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
        print("Test 1: Trying a command that our PreToolUse hook should block...")
        print("User: Run the bash command: ./foo.sh --help")
        await client.query("Run the bash command: ./foo.sh --help")

        async for msg in client.receive_response():
            display_message(msg)

        print("\n" + "=" * 50 + "\n")

        # Test 2: Safe command that should work
        print("Test 2: Trying a command that our PreToolUse hook should allow...")
        print("User: Run the bash command: echo 'Hello from hooks example!'")
        await client.query("Run the bash command: echo 'Hello from hooks example!'")

        async for msg in client.receive_response():
            display_message(msg)

        print("\n" + "=" * 50 + "\n")

    print("\n")


async def example_userpromptsubmit() -> None:
    """Demonstrate context retention across conversation."""
    print("=== UserPromptSubmit Example ===")
    print("This example shows how a UserPromptSubmit hook can add context.\n")

    options = CodexAgentOptions(
        hooks={
            "UserPromptSubmit": [
                HookMatcher(matcher=None, hooks=[add_custom_instructions]),
            ],
        }
    )

    async with CodexSDKClient(options=options) as client:
        print("User: What's my favorite color?")
        await client.query("What's my favorite color?")

        async for msg in client.receive_response():
            display_message(msg)

    print("\n")


async def example_posttooluse() -> None:
    """Demonstrate PostToolUse hook with reason and systemMessage fields."""
    print("=== PostToolUse Example ===")
    print("This example shows how PostToolUse can provide feedback with reason and systemMessage.\n")

    options = CodexAgentOptions(
        allowed_tools=["Bash"],
        hooks={
            "PostToolUse": [
                HookMatcher(matcher="Bash", hooks=[review_tool_output]),
            ],
        }
    )

    async with CodexSDKClient(options=options) as client:
        print("User: Run a command that will produce an error: ls /nonexistent_directory")
        await client.query("Run this command: ls /nonexistent_directory")

        async for msg in client.receive_response():
            display_message(msg)

    print("\n")


async def example_decision_fields() -> None:
    """Demonstrate permissionDecision, reason, and systemMessage fields."""
    print("=== Permission Decision Example ===")
    print("This example shows how to use permissionDecision='allow'/'deny' with reason and systemMessage.\n")

    options = CodexAgentOptions(
        allowed_tools=["Write", "Bash"],
        model="gpt-4o",
        hooks={
            "PreToolUse": [
                HookMatcher(matcher="Write", hooks=[strict_approval_hook]),
            ],
        }
    )

    async with CodexSDKClient(options=options) as client:
        # Test 1: Try to write to a file with "important" in the name (should be blocked)
        print("Test 1: Trying to write to important_config.txt (should be blocked)...")
        print("User: Write 'test' to important_config.txt")
        await client.query("Write the text 'test data' to a file called important_config.txt")

        async for msg in client.receive_response():
            display_message(msg)

        print("\n" + "=" * 50 + "\n")

        # Test 2: Write to a regular file (should be approved)
        print("Test 2: Trying to write to regular_file.txt (should be approved)...")
        print("User: Write 'test' to regular_file.txt")
        await client.query("Write the text 'test data' to a file called regular_file.txt")

        async for msg in client.receive_response():
            display_message(msg)

    print("\n")


async def example_continue_control() -> None:
    """Demonstrate continue and stopReason fields for execution control."""
    print("=== Continue/Stop Control Example ===")
    print("This example shows how to use continue_=False with stopReason to halt execution.\n")

    options = CodexAgentOptions(
        allowed_tools=["Bash"],
        hooks={
            "PostToolUse": [
                HookMatcher(matcher="Bash", hooks=[stop_on_error_hook]),
            ],
        }
    )

    async with CodexSDKClient(options=options) as client:
        print("User: Run a command that outputs 'CRITICAL ERROR'")
        await client.query("Run this bash command: echo 'CRITICAL ERROR: system failure'")

        async for msg in client.receive_response():
            display_message(msg)

    print("\n")


async def main() -> None:
    """Run all examples or a specific example based on command line argument."""
    examples = {
        "PreToolUse": example_pretooluse,
        "UserPromptSubmit": example_userpromptsubmit,
        "PostToolUse": example_posttooluse,
        "DecisionFields": example_decision_fields,
        "ContinueControl": example_continue_control,
    }

    if len(sys.argv) < 2:
        # List available examples
        print("Usage: python hooks.py <example_name>")
        print("\nAvailable examples:")
        print("  all - Run all examples")
        for name in examples:
            print(f"  {name}")
        print("\nExample descriptions:")
        print("  PreToolUse       - Block commands using PreToolUse hook")
        print("  UserPromptSubmit - Add context at prompt submission")
        print("  PostToolUse      - Review tool output with reason and systemMessage")
        print("  DecisionFields   - Use permissionDecision='allow'/'deny' with reason")
        print("  ContinueControl  - Control execution with continue_ and stopReason")
        sys.exit(0)

    example_name = sys.argv[1]

    if example_name == "all":
        # Run all examples
        for example in examples.values():
            await example()
            print("-" * 50 + "\n")
    elif example_name in examples:
        # Run specific example
        await examples[example_name]()
    else:
        print(f"Error: Unknown example '{example_name}'")
        print("\nAvailable examples:")
        print("  all - Run all examples")
        for name in examples:
            print(f"  {name}")
        sys.exit(1)


if __name__ == "__main__":
    print("Starting Codex SDK Hooks Examples...")
    print("=" * 50 + "\n")
    asyncio.run(main())
