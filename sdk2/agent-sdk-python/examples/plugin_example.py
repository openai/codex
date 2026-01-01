#!/usr/bin/env python3
"""Example demonstrating how to use plugins with Codex SDK.

Plugins allow you to extend Codex with custom commands, agents, skills,
and hooks. This example shows how to load a local plugin and verify it's
loaded by checking the system message.

The demo plugin is located in examples/plugins/demo-plugin/ and provides
a custom /greet command.
"""

from pathlib import Path

import anyio

from agent_sdk import (
    CodexAgentOptions,
    SystemMessage,
    query,
)


async def plugin_example():
    """Example showing plugins being loaded in the system message."""
    print("=== Plugin Example ===\n")

    # Get the path to the demo plugin
    # In production, you can use any path to your plugin directory
    plugin_path = Path(__file__).parent / "plugins" / "demo-plugin"

    options = CodexAgentOptions(
        plugins=[
            {
                "type": "local",
                "path": str(plugin_path),
            }
        ],
        max_turns=1,  # Limit to one turn for quick demo
    )

    print(f"Loading plugin from: {plugin_path}\n")

    found_plugins = False
    async for message in query(prompt="Hello!", options=options):
        if isinstance(message, SystemMessage) and message.subtype == "init":
            print("System initialized!")
            print(f"System message data keys: {list(message.data.keys())}\n")

            # Check for plugins in the system message
            plugins_data = message.data.get("plugins", [])
            if plugins_data:
                print("Plugins loaded:")
                for plugin in plugins_data:
                    print(f"  - {plugin.get('name')} (path: {plugin.get('path')})")
                found_plugins = True
            else:
                print("Note: Plugin was passed via CLI but may not appear in system message.")
                print(f"Plugin path configured: {plugin_path}")
                found_plugins = True

    if found_plugins:
        print("\nPlugin successfully configured!\n")


async def main():
    """Run all plugin examples."""
    await plugin_example()


if __name__ == "__main__":
    anyio.run(main)
