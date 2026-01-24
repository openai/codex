#!/usr/bin/env python3
"""Example demonstrating setting sources control.

This example shows how to use the setting_sources option to control which
settings are loaded, including custom slash commands, agents, and other
configurations.

Setting sources determine where Codex loads configurations from:
- "user": Global user settings (~/.codex/)
- "project": Project-level settings (.codex/ in project)
- "local": Local gitignored settings (.codex-local/)

IMPORTANT: When setting_sources is not provided (None), NO settings are loaded
by default. This creates an isolated environment. To load settings, explicitly
specify which sources to use.

By controlling which sources are loaded, you can:
- Create isolated environments with no custom settings (default)
- Load only user settings, excluding project-specific configurations
- Combine multiple sources as needed

Usage:
./examples/setting_sources.py - List the examples
./examples/setting_sources.py all - Run all examples
./examples/setting_sources.py default - Run a specific example
"""

import asyncio
import sys
from pathlib import Path

from agent_sdk import (
    CodexAgentOptions,
    CodexSDKClient,
    SystemMessage,
)


def extract_slash_commands(msg: SystemMessage) -> list[str]:
    """Extract slash command names from system message."""
    if msg.subtype == "init":
        commands = msg.data.get("slash_commands", [])
        return commands
    return []


async def example_default():
    """Default behavior - no settings loaded."""
    print("=== Default Behavior Example ===")
    print("Setting sources: None (default)")
    print("Expected: No custom slash commands will be available\n")

    sdk_dir = Path(__file__).parent.parent

    options = CodexAgentOptions(
        cwd=sdk_dir,
    )

    async with CodexSDKClient(options=options) as client:
        await client.query("What is 2 + 2?")

        async for msg in client.receive_response():
            if isinstance(msg, SystemMessage) and msg.subtype == "init":
                commands = extract_slash_commands(msg)
                print(f"Available slash commands: {commands}")
                if "commit" in commands:
                    print("❌ /commit is available (unexpected)")
                else:
                    print("✓ /commit is NOT available (expected - no settings loaded)")
                break

    print()


async def example_user_only():
    """Load only user-level settings, excluding project settings."""
    print("=== User Settings Only Example ===")
    print("Setting sources: ['user']")
    print("Expected: Project slash commands (like /commit) will NOT be available\n")

    # Use the SDK repo directory which has .codex/commands/commit.md
    sdk_dir = Path(__file__).parent.parent

    options = CodexAgentOptions(
        setting_sources=["user"],
        cwd=sdk_dir,
    )

    async with CodexSDKClient(options=options) as client:
        # Send a simple query
        await client.query("What is 2 + 2?")

        # Check the initialize message for available commands
        async for msg in client.receive_response():
            if isinstance(msg, SystemMessage) and msg.subtype == "init":
                commands = extract_slash_commands(msg)
                print(f"Available slash commands: {commands}")
                if "commit" in commands:
                    print("❌ /commit is available (unexpected)")
                else:
                    print("✓ /commit is NOT available (expected)")
                break

    print()


async def example_project_and_user():
    """Load both project and user settings."""
    print("=== Project + User Settings Example ===")
    print("Setting sources: ['user', 'project']")
    print("Expected: Project slash commands (like /commit) WILL be available\n")

    sdk_dir = Path(__file__).parent.parent

    options = CodexAgentOptions(
        setting_sources=["user", "project"],
        cwd=sdk_dir,
    )

    async with CodexSDKClient(options=options) as client:
        await client.query("What is 2 + 2?")

        async for msg in client.receive_response():
            if isinstance(msg, SystemMessage) and msg.subtype == "init":
                commands = extract_slash_commands(msg)
                print(f"Available slash commands: {commands}")
                if "commit" in commands:
                    print("✓ /commit is available (expected)")
                else:
                    print("❌ /commit is NOT available (unexpected)")
                break

    print()




async def main():
    """Run all examples or a specific example based on command line argument."""
    examples = {
        "default": example_default,
        "user_only": example_user_only,
        "project_and_user": example_project_and_user,
    }

    if len(sys.argv) < 2:
        print("Usage: python setting_sources.py <example_name>")
        print("\nAvailable examples:")
        print("  all - Run all examples")
        for name in examples:
            print(f"  {name}")
        sys.exit(0)

    example_name = sys.argv[1]

    if example_name == "all":
        for example in examples.values():
            await example()
            print("-" * 50 + "\n")
    elif example_name in examples:
        await examples[example_name]()
    else:
        print(f"Error: Unknown example '{example_name}'")
        print("\nAvailable examples:")
        print("  all - Run all examples")
        for name in examples:
            print(f"  {name}")
        sys.exit(1)


if __name__ == "__main__":
    print("Starting Codex SDK Setting Sources Examples...")
    print("=" * 50 + "\n")
    asyncio.run(main())