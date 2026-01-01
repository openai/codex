"""Codex Agent Client for interacting with Codex."""

import json
import os
from collections.abc import AsyncIterable, AsyncIterator
from dataclasses import replace
from typing import Any

from . import Transport
from ._errors import CLIConnectionError
from .types import CodexAgentOptions, HookEvent, HookMatcher, Message, ResultMessage


class CodexAgentClient:
    """
    Client for bidirectional, interactive conversations with Codex.

    This client provides full control over the conversation flow with support
    for streaming, interrupts, and dynamic message sending. For simple one-shot
    queries, consider using the query() function instead.

    Key features:
    - **Bidirectional**: Send and receive messages at any time
    - **Stateful**: Maintains conversation context across messages
    - **Interactive**: Send follow-ups based on responses
    - **Control flow**: Support for interrupts and session management

    When to use CodexAgentClient:
    - Building chat interfaces or conversational UIs
    - Interactive debugging or exploration sessions
    - Multi-turn conversations with context
    - When you need to react to Codex's responses
    - Real-time applications with user input
    - When you need interrupt capabilities

    When to use query() instead:
    - Simple one-off questions
    - Batch processing of prompts
    - Fire-and-forget automation scripts
    - When all inputs are known upfront
    - Stateless operations

    See examples/streaming_mode.py for full examples of CodexAgentClient in
    different scenarios.

    Caveat: As of v0.0.20, you cannot use a CodexAgentClient instance across
    different async runtime contexts (e.g., different trio nurseries or asyncio
    task groups). The client internally maintains a persistent anyio task group
    for reading messages that remains active from connect() until disconnect().
    This means you must complete all operations with the client within the same
    async context where it was connected. Ideally, this limitation should not
    exist.
    """

    def __init__(
        self,
        options: CodexAgentOptions | None = None,
        transport: Transport | None = None,
    ):
        """Initialize Codex Agent client."""
        if options is None:
            options = CodexAgentOptions()
        self.options = options
        self._custom_transport = transport
        self._transport: Transport | None = None
        self._query: Any | None = None
        os.environ["CODEX_ENTRYPOINT"] = "sdk-py-client"

    def _convert_hooks_to_internal_format(
        self, hooks: dict[HookEvent, list[HookMatcher]]
    ) -> dict[str, list[dict[str, Any]]]:
        """Convert HookMatcher format to internal Query format."""
        internal_hooks: dict[str, list[dict[str, Any]]] = {}
        for event, matchers in hooks.items():
            internal_hooks[event] = []
            for matcher in matchers:
                # Convert HookMatcher to internal dict format
                internal_matcher: dict[str, Any] = {
                    "matcher": matcher.matcher if hasattr(matcher, "matcher") else None,
                    "hooks": matcher.hooks if hasattr(matcher, "hooks") else [],
                }
                if hasattr(matcher, "timeout") and matcher.timeout is not None:
                    internal_matcher["timeout"] = matcher.timeout
                internal_hooks[event].append(internal_matcher)
        return internal_hooks

    async def connect(
        self, prompt: str | AsyncIterable[dict[str, Any]] | None = None
    ) -> None:
        """Connect to Codex with a prompt or message stream."""

        from ._internal.query import Query
        from ._internal.transport.subprocess_cli import SubprocessCLITransport

        # Auto-connect with empty async iterable if no prompt is provided
        async def _empty_stream() -> AsyncIterator[dict[str, Any]]:
            # Never yields, but indicates that this function is an iterator and
            # keeps the connection open.
            # This yield is never reached but makes this an async generator
            return
            yield {}  # type: ignore[unreachable]

        actual_prompt = _empty_stream() if prompt is None else prompt

        # Validate and configure permission settings (matching TypeScript SDK logic)
        if self.options.can_use_tool:
            # canUseTool callback requires streaming mode (AsyncIterable prompt)
            if isinstance(prompt, str):
                raise ValueError(
                    "can_use_tool callback requires streaming mode. "
                    "Please provide prompt as an AsyncIterable instead of a string."
                )

            # canUseTool and permission_prompt_tool_name are mutually exclusive
            if self.options.permission_prompt_tool_name:
                raise ValueError(
                    "can_use_tool callback cannot be used with permission_prompt_tool_name. "
                    "Please use one or the other."
                )

            # Automatically set permission_prompt_tool_name to "stdio" for control protocol
            options = replace(self.options, permission_prompt_tool_name="stdio")
        else:
            options = self.options

        # Use provided custom transport or create subprocess transport
        if self._custom_transport:
            self._transport = self._custom_transport
        else:
            self._transport = SubprocessCLITransport(
                prompt=actual_prompt,
                options=options,
            )
        await self._transport.connect()

        # Extract SDK MCP servers from options
        sdk_mcp_servers = {}
        if self.options.mcp_servers and isinstance(self.options.mcp_servers, dict):
            for name, config in self.options.mcp_servers.items():
                if isinstance(config, dict) and config.get("type") == "sdk":
                    sdk_mcp_servers[name] = config["instance"]  # type: ignore[typeddict-item]

        # Calculate initialize timeout from CODEX_CODE_STREAM_CLOSE_TIMEOUT env var if set
        # CODEX_CODE_STREAM_CLOSE_TIMEOUT is in milliseconds, convert to seconds
        initialize_timeout_ms = int(
            os.environ.get("CODEX_CODE_STREAM_CLOSE_TIMEOUT", "60000")
        )
        initialize_timeout = max(initialize_timeout_ms / 1000.0, 60.0)

        # Create Query to handle control protocol
        self._query = Query(
            transport=self._transport,
            is_streaming_mode=True,  # CodexAgentClient always uses streaming mode
            can_use_tool=self.options.can_use_tool,
            hooks=self._convert_hooks_to_internal_format(self.options.hooks)
            if self.options.hooks
            else None,
            sdk_mcp_servers=sdk_mcp_servers,
            initialize_timeout=initialize_timeout,
        )

        # Start reading messages and initialize
        await self._query.start()
        await self._query.initialize()

        # If we have an initial prompt stream, start streaming it
        if prompt is not None and isinstance(prompt, AsyncIterable) and self._query._tg:
            self._query._tg.start_soon(self._query.stream_input, prompt)

    async def receive_messages(self) -> AsyncIterator[Message]:
        """Receive all messages from Codex."""
        if not self._query:
            raise CodexConnectionError("Not connected. Call connect() first.")

        from ._internal.message_parser import parse_message

        async for data in self._query.receive_messages():
            yield parse_message(data)

    async def query(
        self, prompt: str | AsyncIterable[dict[str, Any]], session_id: str = "default"
    ) -> None:
        """
        Send a new request in streaming mode.

        Args:
            prompt: Either a string message or an async iterable of message dictionaries
            session_id: Session identifier for the conversation
        """
        if not self._query or not self._transport:
            raise CodexConnectionError("Not connected. Call connect() first.")

        # Handle string prompts
        if isinstance(prompt, str):
            message = {
                "type": "user",
                "message": {"role": "user", "content": prompt},
                "parent_tool_use_id": None,
                "session_id": session_id,
            }
            await self._transport.write(json.dumps(message) + "\n")
        else:
            # Handle AsyncIterable prompts - stream them
            async for msg in prompt:
                # Ensure session_id is set on each message
                if "session_id" not in msg:
                    msg["session_id"] = session_id
                await self._transport.write(json.dumps(msg) + "\n")

    async def interrupt(self) -> None:
        """Send interrupt signal (only works with streaming mode)."""
        if not self._query:
            raise CodexConnectionError("Not connected. Call connect() first.")
        await self._query.interrupt()

    async def set_permission_mode(self, mode: str) -> None:
        """Change permission mode during conversation (only works with streaming mode).

        Args:
            mode: The permission mode to set. Valid options:
                - 'default': CLI prompts for dangerous tools
                - 'acceptEdits': Auto-accept file edits
                - 'bypassPermissions': Allow all tools (use with caution)

        Example:
            ```python
            async with CodexAgentClient() as client:
                # Start with default permissions
                await client.query("Help me analyze this codebase")

                # Review mode done, switch to auto-accept edits
                await client.set_permission_mode('acceptEdits')
                await client.query("Now implement the fix we discussed")
            ```
        """
        if not self._query:
            raise CodexConnectionError("Not connected. Call connect() first.")
        await self._query.set_permission_mode(mode)

    async def set_model(self, model: str | None = None) -> None:
        """Change the AI model during conversation (only works with streaming mode).

        Args:
            model: The model identifier string to use, or None to use default.

        Example:
            ```python
            async with CodexAgentClient() as client:
                # Start with default model
                await client.query("Help me understand this problem")

                # Switch to a different model for implementation
                await client.set_model('gpt-4o')
                await client.query("Now implement the solution")
            ```
        """
        if not self._query:
            raise CodexConnectionError("Not connected. Call connect() first.")
        await self._query.set_model(model)

    async def rewind_files(self, user_message_id: str) -> None:
        """Rewind tracked files to their state at a specific user message.

        Requires:
            - `enable_file_checkpointing=True` to track file changes
            - `extra_args={"replay-user-messages": None}` to receive UserMessage
              objects with `uuid` in the response stream

        Args:
            user_message_id: UUID of the user message to rewind to. This should be
                the `uuid` field from a `UserMessage` received during the conversation.

        Example:
            ```python
            options = CodexAgentOptions(
                enable_file_checkpointing=True,
                extra_args={"replay-user-messages": None},
            )
            async with CodexAgentClient(options) as client:
                await client.query("Make some changes to my files")
                async for msg in client.receive_response():
                    if isinstance(msg, UserMessage) and msg.uuid:
                        checkpoint_id = msg.uuid  # Save this for later

                # Later, rewind to that point
                await client.rewind_files(checkpoint_id)
            ```
        """
        if not self._query:
            raise CodexConnectionError("Not connected. Call connect() first.")
        await self._query.rewind_files(user_message_id)

    async def get_server_info(self) -> dict[str, Any] | None:
        """Get server initialization info including available commands and output styles.

        Returns initialization information from the Codex server including:
        - Available commands (slash commands, system commands, etc.)
        - Current and available output styles
        - Server capabilities

        Returns:
            Dictionary with server info, or None if not in streaming mode

        Example:
            ```python
            async with CodexAgentClient() as client:
                info = await client.get_server_info()
                if info:
                    print(f"Commands available: {len(info.get('commands', []))}")
                    print(f"Output style: {info.get('output_style', 'default')}")
            ```
        """
        if not self._query:
            raise CodexConnectionError("Not connected. Call connect() first.")
        # Return the initialization result that was already obtained during connect
        return getattr(self._query, "_initialization_result", None)

    async def receive_response(self) -> AsyncIterator[Message]:
        """
        Receive messages from the model until and including a ResultMessage.

        This async iterator yields all messages in sequence and automatically terminates
        after yielding a ResultMessage (which indicates the response is complete).
        It's a convenience method over receive_messages() for single-response workflows.

        **Stopping Behavior:**
        - Yields each message as it's received
        - Terminates immediately after yielding a ResultMessage
        - The ResultMessage IS included in the yielded messages
        - If no ResultMessage is received, the iterator continues indefinitely

        Yields:
            Message: Each message received (UserMessage, AssistantMessage, SystemMessage, ResultMessage)

        Example:
            ```python
            async with CodexAgentClient() as client:
                await client.query("What's the capital of France?")

                async for msg in client.receive_response():
                    if isinstance(msg, AssistantMessage):
                        for block in msg.content:
                            if isinstance(block, TextBlock):
                                print(f"Codex: {block.text}")
                    elif isinstance(msg, ResultMessage):
                        print(f"Cost: ${msg.total_cost_usd:.4f}")
                        # Iterator will terminate after this message
            ```

        Note:
            To collect all messages: `messages = [msg async for msg in client.receive_response()]`
            The final message in the list will always be a ResultMessage.
        """
        async for message in self.receive_messages():
            yield message
            if isinstance(message, ResultMessage):
                return

    async def disconnect(self) -> None:
        """Disconnect from Codex."""
        if self._query:
            await self._query.close()
            self._query = None
        self._transport = None

    async def __aenter__(self) -> "CodexAgentClient":
        """Enter async context - automatically connects with empty stream for interactive use."""
        await self.connect()
        return self

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> bool:
        """Exit async context - always disconnects."""
        await self.disconnect()
        return False
