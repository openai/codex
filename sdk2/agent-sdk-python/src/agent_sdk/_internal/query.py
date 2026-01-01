"""Query class for handling bidirectional control protocol."""

import json
import logging
import os
from collections.abc import AsyncIterable, AsyncIterator, Awaitable, Callable
from contextlib import suppress
from typing import TYPE_CHECKING, Any

import anyio
from mcp.types import (
    CallToolRequest,
    CallToolRequestParams,
    ListToolsRequest,
)

from ..types import (
    PermissionResultAllow,
    PermissionResultDeny,
    SDKControlPermissionRequest,
    SDKControlRequest,
    SDKControlResponse,
    SDKHookCallbackRequest,
    ToolPermissionContext,
)
from .transport import Transport

if TYPE_CHECKING:
    from mcp.server import Server as McpServer

logger = logging.getLogger(__name__)


def _convert_hook_output_for_cli(hook_output: dict[str, Any]) -> dict[str, Any]:
    """Convert Python-safe field names to CLI-expected field names.

    The Python SDK uses `async_` and `continue_` to avoid keyword conflicts,
    but the CLI expects `async` and `continue`. This function performs the
    necessary conversion.
    """
    converted = {}
    for key, value in hook_output.items():
        # Convert Python-safe names to JavaScript names
        if key == "async_":
            converted["async"] = value
        elif key == "continue_":
            converted["continue"] = value
        else:
            converted[key] = value
    return converted


class Query:
    """Handles bidirectional control protocol on top of Transport.

    This class manages:
    - Control request/response routing
    - Hook callbacks
    - Tool permission callbacks
    - Message streaming
    - Initialization handshake
    """

    def __init__(
        self,
        transport: Transport,
        is_streaming_mode: bool,
        can_use_tool: Callable[
            [str, dict[str, Any], ToolPermissionContext],
            Awaitable[PermissionResultAllow | PermissionResultDeny],
        ]
        | None = None,
        hooks: dict[str, list[dict[str, Any]]] | None = None,
        sdk_mcp_servers: dict[str, "McpServer"] | None = None,
        initialize_timeout: float = 60.0,
    ):
        """Initialize Query with transport and callbacks.

        Args:
            transport: Low-level transport for I/O
            is_streaming_mode: Whether using streaming (bidirectional) mode
            can_use_tool: Optional callback for tool permission requests
            hooks: Optional hook configurations
            sdk_mcp_servers: Optional SDK MCP server instances
            initialize_timeout: Timeout in seconds for the initialize request
        """
        self._initialize_timeout = initialize_timeout
        self.transport = transport
        self.is_streaming_mode = is_streaming_mode
        self.can_use_tool = can_use_tool
        self.hooks = hooks or {}
        self.sdk_mcp_servers = sdk_mcp_servers or {}

        # Control protocol state
        self.pending_control_responses: dict[str, anyio.Event] = {}
        self.pending_control_results: dict[str, dict[str, Any] | Exception] = {}
        self.hook_callbacks: dict[str, Callable[..., Any]] = {}
        self.next_callback_id = 0
        self._request_counter = 0

        # Message stream
        self._message_send, self._message_receive = anyio.create_memory_object_stream[
            dict[str, Any]
        ](max_buffer_size=100)
        self._tg: anyio.abc.TaskGroup | None = None
        self._initialized = False
        self._closed = False
        self._initialization_result: dict[str, Any] | None = None

        # Track first result for proper stream closure with SDK MCP servers
        self._first_result_event = anyio.Event()
        self._stream_close_timeout = (
            float(os.environ.get("CODEX_CODE_STREAM_CLOSE_TIMEOUT", "60000")) / 1000.0
        )  # Convert ms to seconds

    async def initialize(self) -> dict[str, Any] | None:
        """Initialize control protocol if in streaming mode.

        Returns:
            Initialize response with supported commands, or None if not streaming
        """
        if not self.is_streaming_mode:
            return None

        # Build hooks configuration for initialization
        hooks_config: dict[str, Any] = {}
        if self.hooks:
            for event, matchers in self.hooks.items():
                if matchers:
                    hooks_config[event] = []
                    for matcher in matchers:
                        callback_ids = []
                        for callback in matcher.get("hooks", []):
                            callback_id = f"hook_{self.next_callback_id}"
                            self.next_callback_id += 1
                            self.hook_callbacks[callback_id] = callback
                            callback_ids.append(callback_id)
                        hook_matcher_config: dict[str, Any] = {
                            "matcher": matcher.get("matcher"),
                            "hookCallbackIds": callback_ids,
                        }
                        if matcher.get("timeout") is not None:
                            hook_matcher_config["timeout"] = matcher.get("timeout")
                        hooks_config[event].append(hook_matcher_config)

        # Send initialize request
        request = {
            "subtype": "initialize",
            "hooks": hooks_config if hooks_config else None,
        }

        # Use longer timeout for initialize since MCP servers may take time to start
        response = await self._send_control_request(
            request, timeout=self._initialize_timeout
        )
        self._initialized = True
        self._initialization_result = response  # Store for later access
        return response

    async def start(self) -> None:
        """Start reading messages from transport."""
        if self._tg is None:
            self._tg = anyio.create_task_group()
            await self._tg.__aenter__()
            self._tg.start_soon(self._read_messages)

    async def _read_messages(self) -> None:
        """Read messages from transport and route them."""
        try:
            async for message in self.transport.read_messages():
                if self._closed:
                    break

                msg_type = message.get("type")

                # Route control messages
                if msg_type == "control_response":
                    response = message.get("response", {})
                    request_id = response.get("request_id")
                    if request_id in self.pending_control_responses:
                        event = self.pending_control_responses[request_id]
                        if response.get("subtype") == "error":
                            self.pending_control_results[request_id] = Exception(
                                response.get("error", "Unknown error")
                            )
                        else:
                            self.pending_control_results[request_id] = response
                        event.set()
                    continue

                elif msg_type == "control_request":
                    # Handle incoming control requests from CLI
                    # Cast message to SDKControlRequest for type safety
                    request: SDKControlRequest = message  # type: ignore[assignment]
                    if self._tg:
                        self._tg.start_soon(self._handle_control_request, request)
                    continue

                elif msg_type == "control_cancel_request":
                    # Handle cancel requests
                    # TODO: Implement cancellation support
                    continue

                # Track results for proper stream closure
                if msg_type == "result":
                    self._first_result_event.set()

                # Regular SDK messages go to the stream
                await self._message_send.send(message)

        except anyio.get_cancelled_exc_class():
            # Task was cancelled - this is expected behavior
            logger.debug("Read task cancelled")
            raise  # Re-raise to properly handle cancellation
        except Exception as e:
            logger.error(f"Fatal error in message reader: {e}")
            # Signal all pending control requests so they fail fast instead of timing out
            for request_id, event in list(self.pending_control_responses.items()):
                if request_id not in self.pending_control_results:
                    self.pending_control_results[request_id] = e
                    event.set()
            # Put error in stream so iterators can handle it
            await self._message_send.send({"type": "error", "error": str(e)})
        finally:
            # Always signal end of stream
            await self._message_send.send({"type": "end"})

    async def _handle_control_request(self, request: SDKControlRequest) -> None:
        """Handle incoming control request from CLI."""
        request_id = request["request_id"]
        request_data = request["request"]
        subtype = request_data["subtype"]

        try:
            response_data: dict[str, Any] = {}

            if subtype == "can_use_tool":
                permission_request: SDKControlPermissionRequest = request_data  # type: ignore[assignment]
                original_input = permission_request["input"]
                # Handle tool permission request
                if not self.can_use_tool:
                    raise Exception("canUseTool callback is not provided")

                context = ToolPermissionContext(
                    signal=None,  # TODO: Add abort signal support
                    suggestions=permission_request.get("permission_suggestions", [])
                    or [],
                )

                response = await self.can_use_tool(
                    permission_request["tool_name"],
                    permission_request["input"],
                    context,
                )

                # Convert PermissionResult to expected dict format
                if isinstance(response, PermissionResultAllow):
                    response_data = {
                        "behavior": "allow",
                        "updatedInput": (
                            response.updated_input
                            if response.updated_input is not None
                            else original_input
                        ),
                    }
                    if response.updated_permissions is not None:
                        response_data["updatedPermissions"] = [
                            permission.to_dict()
                            for permission in response.updated_permissions
                        ]
                elif isinstance(response, PermissionResultDeny):
                    response_data = {"behavior": "deny", "message": response.message}
                    if response.interrupt:
                        response_data["interrupt"] = response.interrupt
                else:
                    raise TypeError(
                        f"Tool permission callback must return PermissionResult (PermissionResultAllow or PermissionResultDeny), got {type(response)}"
                    )

            elif subtype == "hook_callback":
                hook_callback_request: SDKHookCallbackRequest = request_data  # type: ignore[assignment]
                # Handle hook callback
                callback_id = hook_callback_request["callback_id"]
                callback = self.hook_callbacks.get(callback_id)
                if not callback:
                    raise Exception(f"No hook callback found for ID: {callback_id}")

                hook_output = await callback(
                    request_data.get("input"),
                    request_data.get("tool_use_id"),
                    {"signal": None},  # TODO: Add abort signal support
                )
                # Convert Python-safe field names (async_, continue_) to CLI-expected names (async, continue)
                response_data = _convert_hook_output_for_cli(hook_output)

            elif subtype == "mcp_message":
                # Handle SDK MCP request
                server_name = request_data.get("server_name")
                mcp_message = request_data.get("message")

                if not server_name or not mcp_message:
                    raise Exception("Missing server_name or message for MCP request")

                # Type narrowing - we've verified these are not None above
                assert isinstance(server_name, str)
                assert isinstance(mcp_message, dict)
                mcp_response = await self._handle_sdk_mcp_request(
                    server_name, mcp_message
                )
                # Wrap the MCP response as expected by the control protocol
                response_data = {"mcp_response": mcp_response}

            else:
                raise Exception(f"Unsupported control request subtype: {subtype}")

            # Send success response
            success_response: SDKControlResponse = {
                "type": "control_response",
                "response": {
                    "subtype": "success",
                    "request_id": request_id,
                    "response": response_data,
                },
            }
            await self.transport.write(json.dumps(success_response) + "\n")

        except Exception as e:
            # Send error response
            error_response: SDKControlResponse = {
                "type": "control_response",
                "response": {
                    "subtype": "error",
                    "request_id": request_id,
                    "error": str(e),
                },
            }
            await self.transport.write(json.dumps(error_response) + "\n")

    async def _send_control_request(
        self, request: dict[str, Any], timeout: float = 60.0
    ) -> dict[str, Any]:
        """Send control request to CLI and wait for response.

        Args:
            request: The control request to send
            timeout: Timeout in seconds to wait for response (default 60s)
        """
        if not self.is_streaming_mode:
            raise Exception("Control requests require streaming mode")

        # Generate unique request ID
        self._request_counter += 1
        request_id = f"req_{self._request_counter}_{os.urandom(4).hex()}"

        # Create event for response
        event = anyio.Event()
        self.pending_control_responses[request_id] = event

        # Build and send request
        control_request = {
            "type": "control_request",
            "request_id": request_id,
            "request": request,
        }

        await self.transport.write(json.dumps(control_request) + "\n")

        # Wait for response
        try:
            with anyio.fail_after(timeout):
                await event.wait()

            result = self.pending_control_results.pop(request_id)
            self.pending_control_responses.pop(request_id, None)

            if isinstance(result, Exception):
                raise result

            response_data = result.get("response", {})
            return response_data if isinstance(response_data, dict) else {}
        except TimeoutError as e:
            self.pending_control_responses.pop(request_id, None)
            self.pending_control_results.pop(request_id, None)
            raise Exception(f"Control request timeout: {request.get('subtype')}") from e

    async def _handle_sdk_mcp_request(
        self, server_name: str, message: dict[str, Any]
    ) -> dict[str, Any]:
        """Handle an MCP request for an SDK server.

        This acts as a bridge between JSONRPC messages from the CLI
        and the in-process MCP server. Ideally the MCP SDK would provide
        a method to handle raw JSONRPC, but for now we route manually.

        Args:
            server_name: Name of the SDK MCP server
            message: The JSONRPC message

        Returns:
            The response message
        """
        if server_name not in self.sdk_mcp_servers:
            return {
                "jsonrpc": "2.0",
                "id": message.get("id"),
                "error": {
                    "code": -32601,
                    "message": f"Server '{server_name}' not found",
                },
            }

        server = self.sdk_mcp_servers[server_name]
        method = message.get("method")
        params = message.get("params", {})

        try:
            # TODO: Python MCP SDK lacks the Transport abstraction that TypeScript has.
            # TypeScript: server.connect(transport) allows custom transports
            # Python: server.run(read_stream, write_stream) requires actual streams
            #
            # This forces us to manually route methods. When Python MCP adds Transport
            # support, we can refactor to match the TypeScript approach.
            if method == "initialize":
                # Handle MCP initialization - hardcoded for tools only, no listChanged
                return {
                    "jsonrpc": "2.0",
                    "id": message.get("id"),
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}  # Tools capability without listChanged
                        },
                        "serverInfo": {
                            "name": server.name,
                            "version": server.version or "1.0.0",
                        },
                    },
                }

            elif method == "tools/list":
                request = ListToolsRequest(method=method)
                handler = server.request_handlers.get(ListToolsRequest)
                if handler:
                    result = await handler(request)
                    # Convert MCP result to JSONRPC response
                    tools_data = [
                        {
                            "name": tool.name,
                            "description": tool.description,
                            "inputSchema": (
                                tool.inputSchema.model_dump()
                                if hasattr(tool.inputSchema, "model_dump")
                                else tool.inputSchema
                            )
                            if tool.inputSchema
                            else {},
                        }
                        for tool in result.root.tools  # type: ignore[union-attr]
                    ]
                    return {
                        "jsonrpc": "2.0",
                        "id": message.get("id"),
                        "result": {"tools": tools_data},
                    }

            elif method == "tools/call":
                call_request = CallToolRequest(
                    method=method,
                    params=CallToolRequestParams(
                        name=params.get("name"), arguments=params.get("arguments", {})
                    ),
                )
                handler = server.request_handlers.get(CallToolRequest)
                if handler:
                    result = await handler(call_request)
                    # Convert MCP result to JSONRPC response
                    content = []
                    for item in result.root.content:  # type: ignore[union-attr]
                        if hasattr(item, "text"):
                            content.append({"type": "text", "text": item.text})
                        elif hasattr(item, "data") and hasattr(item, "mimeType"):
                            content.append(
                                {
                                    "type": "image",
                                    "data": item.data,
                                    "mimeType": item.mimeType,
                                }
                            )

                    response_data = {"content": content}
                    if hasattr(result.root, "is_error") and result.root.is_error:
                        response_data["is_error"] = True  # type: ignore[assignment]

                    return {
                        "jsonrpc": "2.0",
                        "id": message.get("id"),
                        "result": response_data,
                    }

            elif method == "notifications/initialized":
                # Handle initialized notification - just acknowledge it
                return {"jsonrpc": "2.0", "result": {}}

            # Add more methods here as MCP SDK adds them (resources, prompts, etc.)
            # This is the limitation Ashwin pointed out - we have to manually update

            return {
                "jsonrpc": "2.0",
                "id": message.get("id"),
                "error": {"code": -32601, "message": f"Method '{method}' not found"},
            }

        except Exception as e:
            return {
                "jsonrpc": "2.0",
                "id": message.get("id"),
                "error": {"code": -32603, "message": str(e)},
            }

    async def interrupt(self) -> None:
        """Send interrupt control request."""
        await self._send_control_request({"subtype": "interrupt"})

    async def set_permission_mode(self, mode: str) -> None:
        """Change permission mode."""
        await self._send_control_request(
            {
                "subtype": "set_permission_mode",
                "mode": mode,
            }
        )

    async def set_model(self, model: str | None) -> None:
        """Change the AI model."""
        await self._send_control_request(
            {
                "subtype": "set_model",
                "model": model,
            }
        )

    async def rewind_files(self, user_message_id: str) -> None:
        """Rewind tracked files to their state at a specific user message.

        Requires file checkpointing to be enabled via the `enable_file_checkpointing` option.

        Args:
            user_message_id: UUID of the user message to rewind to
        """
        await self._send_control_request(
            {
                "subtype": "rewind_files",
                "user_message_id": user_message_id,
            }
        )

    async def stream_input(self, stream: AsyncIterable[dict[str, Any]]) -> None:
        """Stream input messages to transport.

        If SDK MCP servers or hooks are present, waits for the first result
        before closing stdin to allow bidirectional control protocol communication.
        """
        try:
            async for message in stream:
                if self._closed:
                    break
                await self.transport.write(json.dumps(message) + "\n")

            # If we have SDK MCP servers or hooks that need bidirectional communication,
            # wait for first result before closing the channel
            has_hooks = bool(self.hooks)
            if self.sdk_mcp_servers or has_hooks:
                logger.debug(
                    f"Waiting for first result before closing stdin "
                    f"(sdk_mcp_servers={len(self.sdk_mcp_servers)}, has_hooks={has_hooks})"
                )
                try:
                    with anyio.move_on_after(self._stream_close_timeout):
                        await self._first_result_event.wait()
                        logger.debug("Received first result, closing input stream")
                except Exception:
                    logger.debug(
                        "Timed out waiting for first result, closing input stream"
                    )

            # After all messages sent (and result received if needed), end input
            await self.transport.end_input()
        except Exception as e:
            logger.debug(f"Error streaming input: {e}")

    async def receive_messages(self) -> AsyncIterator[dict[str, Any]]:
        """Receive SDK messages (not control messages)."""
        async for message in self._message_receive:
            # Check for special messages
            if message.get("type") == "end":
                break
            elif message.get("type") == "error":
                raise Exception(message.get("error", "Unknown error"))

            yield message

    async def close(self) -> None:
        """Close the query and transport."""
        self._closed = True
        if self._tg:
            self._tg.cancel_scope.cancel()
            # Wait for task group to complete cancellation
            with suppress(anyio.get_cancelled_exc_class()):
                await self._tg.__aexit__(None, None, None)
        await self.transport.close()

    # Make Query an async iterator
    def __aiter__(self) -> AsyncIterator[dict[str, Any]]:
        """Return async iterator for messages."""
        return self.receive_messages()

    async def __anext__(self) -> dict[str, Any]:
        """Get next message."""
        async for message in self.receive_messages():
            return message
        raise StopAsyncIteration
