"""Simple example demonstrating stderr callback for capturing CLI debug output."""

import asyncio

from agent_sdk import CodexAgentOptions, query


async def main():
    """Capture stderr output from the CLI using a callback."""

    # Collect stderr messages
    stderr_messages = []

    def stderr_callback(message: str):
        """Callback that receives each line of stderr output."""
        stderr_messages.append(message)
        # Optionally print specific messages
        if "[ERROR]" in message:
            print(f"Error detected: {message}")

    # Create options with stderr callback and enable debug mode
    options = CodexAgentOptions(
        stderr=stderr_callback,
        extra_args={"debug-to-stderr": None}  # Enable debug output
    )

    # Run a query
    print("Running query with stderr capture...")
    async for message in query(
        prompt="What is 2+2?",
        options=options
    ):
        if hasattr(message, 'content'):
            if isinstance(message.content, str):
                print(f"Response: {message.content}")

    # Show what we captured
    print(f"\nCaptured {len(stderr_messages)} stderr lines")
    if stderr_messages:
        print("First stderr line:", stderr_messages[0][:100])


if __name__ == "__main__":
    asyncio.run(main())