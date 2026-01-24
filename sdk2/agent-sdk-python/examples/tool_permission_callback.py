#!/usr/bin/env python3
"""Example: Tool Permission Callbacks.

This example demonstrates how to use tool permission callbacks to control
which tools Codex can use and modify their inputs.
"""

import asyncio
import json

from agent_sdk import (
    AssistantMessage,
    CodexAgentOptions,
    CodexSDKClient,
    PermissionResultAllow,
    PermissionResultDeny,
    ResultMessage,
    TextBlock,
    ToolPermissionContext,
)

# Track tool usage for demonstration
tool_usage_log = []


async def my_permission_callback(
    tool_name: str,
    input_data: dict,
    context: ToolPermissionContext
) -> PermissionResultAllow | PermissionResultDeny:
    """Control tool permissions based on tool type and input."""

    # Log the tool request
    tool_usage_log.append({
        "tool": tool_name,
        "input": input_data,
        "suggestions": context.suggestions
    })

    print(f"\n🔧 Tool Permission Request: {tool_name}")
    print(f"   Input: {json.dumps(input_data, indent=2)}")

    # Always allow read operations
    if tool_name in ["Read", "Glob", "Grep"]:
        print(f"   ✅ Automatically allowing {tool_name} (read-only operation)")
        return PermissionResultAllow()

    # Deny write operations to system directories
    if tool_name in ["Write", "Edit", "MultiEdit"]:
        file_path = input_data.get("file_path", "")
        if file_path.startswith("/etc/") or file_path.startswith("/usr/"):
            print(f"   ❌ Denying write to system directory: {file_path}")
            return PermissionResultDeny(
                message=f"Cannot write to system directory: {file_path}"
            )

        # Redirect writes to a safe directory
        if not file_path.startswith("/tmp/") and not file_path.startswith("./"):
            safe_path = f"./safe_output/{file_path.split('/')[-1]}"
            print(f"   ⚠️  Redirecting write from {file_path} to {safe_path}")
            modified_input = input_data.copy()
            modified_input["file_path"] = safe_path
            return PermissionResultAllow(
                updated_input=modified_input
            )

    # Check dangerous bash commands
    if tool_name == "Bash":
        command = input_data.get("command", "")
        dangerous_commands = ["rm -rf", "sudo", "chmod 777", "dd if=", "mkfs"]

        for dangerous in dangerous_commands:
            if dangerous in command:
                print(f"   ❌ Denying dangerous command: {command}")
                return PermissionResultDeny(
                    message=f"Dangerous command pattern detected: {dangerous}"
                )

        # Allow but log the command
        print(f"   ✅ Allowing bash command: {command}")
        return PermissionResultAllow()

    # For all other tools, ask the user
    print(f"   ❓ Unknown tool: {tool_name}")
    print(f"      Input: {json.dumps(input_data, indent=6)}")
    user_input = input("   Allow this tool? (y/N): ").strip().lower()

    if user_input in ("y", "yes"):
        return PermissionResultAllow()
    else:
        return PermissionResultDeny(
            message="User denied permission"
        )


async def main():
    """Run example with tool permission callbacks."""

    print("=" * 60)
    print("Tool Permission Callback Example")
    print("=" * 60)
    print("\nThis example demonstrates how to:")
    print("1. Allow/deny tools based on type")
    print("2. Modify tool inputs for safety")
    print("3. Log tool usage")
    print("4. Prompt for unknown tools")
    print("=" * 60)

    # Configure options with our callback
    options = CodexAgentOptions(
        can_use_tool=my_permission_callback,
        # Use default permission mode to ensure callbacks are invoked
        permission_mode="default",
        cwd="."  # Set working directory
    )

    # Create client and send a query that will use multiple tools
    async with CodexSDKClient(options) as client:
        print("\n📝 Sending query ...")
        await client.query(
            "Please do the following:\n"
            "1. List the files in the current directory\n"
            "2. Create a simple Python hello world script at hello.py\n"
            "3. Run the script to test it"
        )

        print("\n📨 Receiving response...")
        message_count = 0

        async for message in client.receive_response():
            message_count += 1

            if isinstance(message, AssistantMessage):
                # Print Codex's text responses
                for block in message.content:
                    if isinstance(block, TextBlock):
                        print(f"\n💬 Codex: {block.text}")

            elif isinstance(message, ResultMessage):
                print("\n✅ Task completed!")
                print(f"   Duration: {message.duration_ms}ms")
                if message.total_cost_usd:
                    print(f"   Cost: ${message.total_cost_usd:.4f}")
                print(f"   Messages processed: {message_count}")

    # Print tool usage summary
    print("\n" + "=" * 60)
    print("Tool Usage Summary")
    print("=" * 60)
    for i, usage in enumerate(tool_usage_log, 1):
        print(f"\n{i}. Tool: {usage['tool']}")
        print(f"   Input: {json.dumps(usage['input'], indent=6)}")
        if usage['suggestions']:
            print(f"   Suggestions: {usage['suggestions']}")


if __name__ == "__main__":
    asyncio.run(main())
