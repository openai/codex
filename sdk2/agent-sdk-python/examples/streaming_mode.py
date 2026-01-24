#!/usr/bin/env python3
"""
Comprehensive examples of using CodexSDKClient for streaming mode.

This file demonstrates various patterns for building applications with
the CodexSDKClient streaming interface.

The queries are intentionally simplistic. In reality, a query can be a more
complex task that Codex SDK uses its agentic capabilities and tools (e.g. run
bash commands, edit files, search the web, fetch web content) to accomplish.

Usage:
./examples/streaming_mode.py - List the examples
./examples/streaming_mode.py all - Run all examples
./examples/streaming_mode.py basic_streaming - Run a specific example
"""

import asyncio
import contextlib
import sys

from agent_sdk import (
    AssistantMessage,
    CodexAgentOptions,
    CodexSDKClient,
    CLIConnectionError,
    ResultMessage,
    SystemMessage,
    TextBlock,
    ToolResultBlock,
    ToolUseBlock,
    UserMessage,
)


def display_message(msg):
    """Standardized message display function.

    - UserMessage: "User: <content>"
    - AssistantMessage: "Codex: <content>"
    - SystemMessage: ignored
    - ResultMessage: "Result ended" + cost if available
    """
    if isinstance(msg, UserMessage):
        for block in msg.content:
            if isinstance(block, TextBlock):
                print(f"User: {block.text}")
    elif isinstance(msg, AssistantMessage):
        for block in msg.content:
            if isinstance(block, TextBlock):
                print(f"Codex: {block.text}")
    elif isinstance(msg, SystemMessage):
        # Ignore system messages
        pass
    elif isinstance(msg, ResultMessage):
        print("Result ended")


async def example_basic_streaming():
    """Basic streaming with context manager."""
    print("=== Basic Streaming Example ===")

    async with CodexSDKClient() as client:
        print("User: What is 2+2?")
        await client.query("What is 2+2?")

        # Receive complete response using the helper method
        async for msg in client.receive_response():
            display_message(msg)

    print("\n")


async def example_multi_turn_conversation():
    """Multi-turn conversation using receive_response helper."""
    print("=== Multi-Turn Conversation Example ===")

    async with CodexSDKClient() as client:
        # First turn
        print("User: What's the capital of France?")
        await client.query("What's the capital of France?")

        # Extract and print response
        async for msg in client.receive_response():
            display_message(msg)

        # Second turn - follow-up
        print("\nUser: What's the population of that city?")
        await client.query("What's the population of that city?")

        async for msg in client.receive_response():
            display_message(msg)

    print("\n")


async def example_concurrent_responses():
    """Handle responses while sending new messages."""
    print("=== Concurrent Send/Receive Example ===")

    async with CodexSDKClient() as client:
        # Background task to continuously receive messages
        async def receive_messages():
            async for message in client.receive_messages():
                display_message(message)

        # Start receiving in background
        receive_task = asyncio.create_task(receive_messages())

        # Send multiple messages with delays
        questions = [
            "What is 2 + 2?",
            "What is the square root of 144?",
            "What is 10% of 80?",
        ]

        for question in questions:
            print(f"\nUser: {question}")
            await client.query(question)
            await asyncio.sleep(3)  # Wait between messages

        # Give time for final responses
        await asyncio.sleep(2)

        # Clean up
        receive_task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await receive_task

    print("\n")


async def example_with_interrupt():
    """Demonstrate interrupt capability."""
    print("=== Interrupt Example ===")
    print("IMPORTANT: Interrupts require active message consumption.")

    async with CodexSDKClient() as client:
        # Start a long-running task
        print("\nUser: Count from 1 to 100 slowly")
        await client.query(
            "Count from 1 to 100 slowly, with a brief pause between each number"
        )

        # Create a background task to consume messages
        messages_received = []

        async def consume_messages():
            """Consume messages in the background to enable interrupt processing."""
            async for message in client.receive_response():
                messages_received.append(message)
                display_message(message)

        # Start consuming messages in the background
        consume_task = asyncio.create_task(consume_messages())

        # Wait 2 seconds then send interrupt
        await asyncio.sleep(2)
        print("\n[After 2 seconds, sending interrupt...]")
        await client.interrupt()

        # Wait for the consume task to finish processing the interrupt
        await consume_task

        # Send new instruction after interrupt
        print("\nUser: Never mind, just tell me a quick joke")
        await client.query("Never mind, just tell me a quick joke")

        # Get the joke
        async for msg in client.receive_response():
            display_message(msg)

    print("\n")


async def example_manual_message_handling():
    """Manually handle message stream for custom logic."""
    print("=== Manual Message Handling Example ===")

    async with CodexSDKClient() as client:
        await client.query("List 5 programming languages and their main use cases")

        # Manually process messages with custom logic
        languages_found = []

        async for message in client.receive_messages():
            if isinstance(message, AssistantMessage):
                for block in message.content:
                    if isinstance(block, TextBlock):
                        text = block.text
                        print(f"Codex: {text}")
                        # Custom logic: extract language names
                        for lang in [
                            "Python",
                            "JavaScript",
                            "Java",
                            "C++",
                            "Go",
                            "Rust",
                            "Ruby",
                        ]:
                            if lang in text and lang not in languages_found:
                                languages_found.append(lang)
                                print(f"Found language: {lang}")
            elif isinstance(message, ResultMessage):
                display_message(message)
                print(f"Total languages mentioned: {len(languages_found)}")
                break

    print("\n")


async def example_with_options():
    """Use CodexAgentOptions to configure the client."""
    print("=== Custom Options Example ===")

    # Configure options
    options = CodexAgentOptions(
        allowed_tools=["Read", "Write"],  # Allow file operations
        system_prompt="You are a helpful coding assistant.",
        env={
            "MODEL": "gpt-4o",
        },
    )

    async with CodexSDKClient(options=options) as client:
        print("User: Create a simple hello.txt file with a greeting message")
        await client.query("Create a simple hello.txt file with a greeting message")

        tool_uses = []
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                display_message(msg)
                for block in msg.content:
                    if hasattr(block, "name") and not isinstance(
                        block, TextBlock
                    ):  # ToolUseBlock
                        tool_uses.append(getattr(block, "name", ""))
            else:
                display_message(msg)

        if tool_uses:
            print(f"Tools used: {', '.join(tool_uses)}")

    print("\n")


async def example_async_iterable_prompt():
    """Demonstrate send_message with async iterable."""
    print("=== Async Iterable Prompt Example ===")

    async def create_message_stream():
        """Generate a stream of messages."""
        print("User: Hello! I have multiple questions.")
        yield {
            "type": "user",
            "message": {"role": "user", "content": "Hello! I have multiple questions."},
            "parent_tool_use_id": None,
            "session_id": "qa-session",
        }

        print("User: First, what's the capital of Japan?")
        yield {
            "type": "user",
            "message": {
                "role": "user",
                "content": "First, what's the capital of Japan?",
            },
            "parent_tool_use_id": None,
            "session_id": "qa-session",
        }

        print("User: Second, what's 15% of 200?")
        yield {
            "type": "user",
            "message": {"role": "user", "content": "Second, what's 15% of 200?"},
            "parent_tool_use_id": None,
            "session_id": "qa-session",
        }

    async with CodexSDKClient() as client:
        # Send async iterable of messages
        await client.query(create_message_stream())

        # Receive the three responses
        async for msg in client.receive_response():
            display_message(msg)
        async for msg in client.receive_response():
            display_message(msg)
        async for msg in client.receive_response():
            display_message(msg)

    print("\n")


async def example_bash_command():
    """Example showing tool use blocks when running bash commands."""
    print("=== Bash Command Example ===")

    async with CodexSDKClient() as client:
        print("User: Run a bash echo command")
        await client.query("Run a bash echo command that says 'Hello from bash!'")

        # Track all message types received
        message_types = []

        async for msg in client.receive_messages():
            message_types.append(type(msg).__name__)

            if isinstance(msg, UserMessage):
                # User messages can contain tool results
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"User: {block.text}")
                    elif isinstance(block, ToolResultBlock):
                        print(
                            f"Tool Result (id: {block.tool_use_id}): {block.content[:100] if block.content else 'None'}..."
                        )

            elif isinstance(msg, AssistantMessage):
                # Assistant messages can contain tool use blocks
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Codex: {block.text}")
                    elif isinstance(block, ToolUseBlock):
                        print(f"Tool Use: {block.name} (id: {block.id})")
                        if block.name == "Bash":
                            command = block.input.get("command", "")
                            print(f"  Command: {command}")

            elif isinstance(msg, ResultMessage):
                print("Result ended")
                if msg.total_cost_usd:
                    print(f"Cost: ${msg.total_cost_usd:.4f}")
                break

        print(f"\nMessage types received: {', '.join(set(message_types))}")

    print("\n")


async def example_control_protocol():
    """Demonstrate server info and interrupt capabilities."""
    print("=== Control Protocol Example ===")
    print("Shows server info retrieval and interrupt capability\n")

    async with CodexSDKClient() as client:
        # 1. Get server initialization info
        print("1. Getting server info...")
        server_info = await client.get_server_info()

        if server_info:
            print("✓ Server info retrieved successfully!")
            print(f"  - Available commands: {len(server_info.get('commands', []))}")
            print(f"  - Output style: {server_info.get('output_style', 'unknown')}")

            # Show available output styles if present
            styles = server_info.get('available_output_styles', [])
            if styles:
                print(f"  - Available output styles: {', '.join(styles)}")

            # Show a few example commands
            commands = server_info.get('commands', [])[:5]
            if commands:
                print("  - Example commands:")
                for cmd in commands:
                    if isinstance(cmd, dict):
                        print(f"    • {cmd.get('name', 'unknown')}")
        else:
            print("✗ No server info available (may not be in streaming mode)")

        print("\n2. Testing interrupt capability...")

        # Start a long-running task
        print("User: Count from 1 to 20 slowly")
        await client.query("Count from 1 to 20 slowly, pausing between each number")

        # Start consuming messages in background to enable interrupt
        messages = []
        async def consume():
            async for msg in client.receive_response():
                messages.append(msg)
                if isinstance(msg, AssistantMessage):
                    for block in msg.content:
                        if isinstance(block, TextBlock):
                            # Print first 50 chars to show progress
                            print(f"Codex: {block.text[:50]}...")
                            break
                if isinstance(msg, ResultMessage):
                    break

        consume_task = asyncio.create_task(consume())

        # Wait a moment then interrupt
        await asyncio.sleep(2)
        print("\n[Sending interrupt after 2 seconds...]")

        try:
            await client.interrupt()
            print("✓ Interrupt sent successfully")
        except Exception as e:
            print(f"✗ Interrupt failed: {e}")

        # Wait for task to complete
        with contextlib.suppress(asyncio.CancelledError):
            await consume_task

        # Send new query after interrupt
        print("\nUser: Just say 'Hello!'")
        await client.query("Just say 'Hello!'")

        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Codex: {block.text}")

    print("\n")


async def example_error_handling():
    """Demonstrate proper error handling."""
    print("=== Error Handling Example ===")

    client = CodexSDKClient()

    try:
        await client.connect()

        # Send a message that will take time to process
        print("User: Run a bash sleep command for 60 seconds not in the background")
        await client.query("Run a bash sleep command for 60 seconds not in the background")

        # Try to receive response with a short timeout
        try:
            messages = []
            async with asyncio.timeout(10.0):
                async for msg in client.receive_response():
                    messages.append(msg)
                    if isinstance(msg, AssistantMessage):
                        for block in msg.content:
                            if isinstance(block, TextBlock):
                                print(f"Codex: {block.text[:50]}...")
                    elif isinstance(msg, ResultMessage):
                        display_message(msg)
                        break

        except asyncio.TimeoutError:
            print(
                "\nResponse timeout after 10 seconds - demonstrating graceful handling"
            )
            print(f"Received {len(messages)} messages before timeout")

    except CLIConnectionError as e:
        print(f"Connection error: {e}")

    except Exception as e:
        print(f"Unexpected error: {e}")

    finally:
        # Always disconnect
        await client.disconnect()

    print("\n")


async def main():
    """Run all examples or a specific example based on command line argument."""
    examples = {
        "basic_streaming": example_basic_streaming,
        "multi_turn_conversation": example_multi_turn_conversation,
        "concurrent_responses": example_concurrent_responses,
        "with_interrupt": example_with_interrupt,
        "manual_message_handling": example_manual_message_handling,
        "with_options": example_with_options,
        "async_iterable_prompt": example_async_iterable_prompt,
        "bash_command": example_bash_command,
        "control_protocol": example_control_protocol,
        "error_handling": example_error_handling,
    }

    if len(sys.argv) < 2:
        # List available examples
        print("Usage: python streaming_mode.py <example_name>")
        print("\nAvailable examples:")
        print("  all - Run all examples")
        for name in examples:
            print(f"  {name}")
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
    asyncio.run(main())
