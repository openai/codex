"""Query function for one-shot interactions with Codex."""

import os
from collections.abc import AsyncIterable, AsyncIterator
from typing import Any

from ._internal.client import InternalClient
from ._internal.transport import Transport
from .types import CodexAgentOptions, Message


async def query(
    *,
    prompt: str | AsyncIterable[dict[str, Any]],
    options: CodexAgentOptions | None = None,
    transport: Transport | None = None,
) -> AsyncIterator[Message]:
    """
    Query Codex for one-shot or unidirectional streaming interactions.

    This function is ideal for simple, stateless queries where you don't need
    bidirectional communication or conversation management. For interactive,
    stateful conversations, use CodexSDKClient instead.

    Key differences from CodexSDKClient:
    - **Unidirectional**: Send all messages upfront, receive all responses
    - **Stateless**: Each query is independent, no conversation state
    - **Simple**: Fire-and-forget style, no connection management
    - **No interrupts**: Cannot interrupt or send follow-up messages

    When to use query():
    - Simple one-off questions ("What is 2+2?")
    - Batch processing of independent prompts
    - Code generation or analysis tasks
    - Automated scripts and CI/CD pipelines
    - When you know all inputs upfront

    When to use CodexSDKClient:
    - Interactive conversations with follow-ups
    - Chat applications or REPL-like interfaces
    - When you need to send messages based on responses
    - When you need interrupt capabilities
    - Long-running sessions with state

    Args:
        prompt: The prompt to send to the model. Can be a string for single-shot queries
                or an AsyncIterable[dict] for streaming mode with continuous interaction.
                In streaming mode, each dict should have the structure:
                {
                    "type": "user",
                    "message": {"role": "user", "content": "..."},
                    "parent_tool_use_id": None,
                    "session_id": "..."
                }
        options: Optional configuration (defaults to CodexAgentOptions() if None).
                 Set options.permission_mode to control tool execution:
                 - 'default': CLI prompts for dangerous tools
                 - 'acceptEdits': Auto-accept file edits
                 - 'bypassPermissions': Allow all tools (use with caution)
                 Set options.cwd for working directory.
        transport: Optional transport implementation. If provided, this will be used
                  instead of the default transport selection based on options.
                  The transport will be automatically configured with the prompt and options.

    Yields:
        Messages from the conversation

    Example - Simple query:
        ```python
        # One-off question
        async for message in query(prompt="What is the capital of France?"):
            print(message)
        ```

    Example - With options:
        ```python
        # Code generation with specific settings
        async for message in query(
            prompt="Create a Python web server",
            options=CodexAgentOptions(
                system_prompt="You are an expert Python developer",
                cwd="/home/user/project"
            )
        ):
            print(message)
        ```

    Example - Streaming mode (still unidirectional):
        ```python
        async def prompts():
            yield {"type": "user", "message": {"role": "user", "content": "Hello"}}
            yield {"type": "user", "message": {"role": "user", "content": "How are you?"}}

        # All prompts are sent, then all responses received
        async for message in query(prompt=prompts()):
            print(message)
        ```

    Example - With custom transport:
        ```python
        from agent_sdk import query, Transport

        class MyCustomTransport(Transport):
            # Implement custom transport logic
            pass

        transport = MyCustomTransport()
        async for message in query(
            prompt="Hello",
            transport=transport
        ):
            print(message)
        ```

    """
    if options is None:
        options = CodexAgentOptions()

    os.environ["CODEX_CODE_ENTRYPOINT"] = "sdk-py"

    client = InternalClient()

    async for message in client.process_query(
        prompt=prompt, options=options, transport=transport
    ):
        yield message
