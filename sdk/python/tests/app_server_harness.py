from __future__ import annotations

import json
import queue
import shutil
import threading
import time
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from openai_codex import AppServerConfig

Json = dict[str, Any]


@dataclass(frozen=True)
class CapturedResponsesRequest:
    """Recorded request sent by app-server to the mock Responses API."""

    method: str
    path: str
    headers: dict[str, str]
    body: bytes

    def body_json(self) -> Json:
        """Decode the request body as JSON."""
        return json.loads(self.body.decode("utf-8"))

    def input(self) -> list[Json]:
        """Return the Responses API input array from the request."""
        value = self.body_json().get("input")
        if not isinstance(value, list):
            raise AssertionError(f"expected input list, got {value!r}")
        return value

    def message_input_texts(self, role: str) -> list[str]:
        """Return all input_text strings for message inputs matching one role."""
        texts: list[str] = []
        for item in self.input():
            if item.get("type") != "message" or item.get("role") != role:
                continue
            content = item.get("content")
            if isinstance(content, str):
                texts.append(content)
                continue
            if not isinstance(content, list):
                continue
            for span in content:
                if isinstance(span, dict) and span.get("type") == "input_text":
                    text = span.get("text")
                    if isinstance(text, str):
                        texts.append(text)
        return texts

    def message_content_items(self, role: str) -> list[Json]:
        """Return structured content items for message inputs matching one role."""
        items: list[Json] = []
        for item in self.input():
            if item.get("type") != "message" or item.get("role") != role:
                continue
            content = item.get("content")
            if not isinstance(content, list):
                continue
            items.extend(part for part in content if isinstance(part, dict))
        return items

    def message_image_urls(self, role: str) -> list[str]:
        """Return all input_image URLs for message inputs matching one role."""
        urls: list[str] = []
        for item in self.message_content_items(role):
            if item.get("type") != "input_image":
                continue
            image_url = item.get("image_url")
            if isinstance(image_url, str):
                urls.append(image_url)
        return urls

    def header(self, name: str) -> str | None:
        """Return a captured request header by case-insensitive name."""
        return self.headers.get(name.lower())


@dataclass(frozen=True)
class MockSseResponse:
    """One queued SSE response served by the mock Responses API."""

    body: str
    delay_between_events_s: float = 0.0

    def chunks(self) -> list[bytes]:
        """Split an SSE body into event chunks while preserving framing."""
        chunks: list[bytes] = []
        for part in self.body.split("\n\n"):
            if not part:
                continue
            chunks.append(f"{part}\n\n".encode("utf-8"))
        return chunks


class MockResponsesServer:
    """Local HTTP server that records `/v1/responses` requests and returns SSE."""

    def __init__(self) -> None:
        self._responses: queue.Queue[MockSseResponse] = queue.Queue()
        self._requests: list[CapturedResponsesRequest] = []
        self._requests_lock = threading.Lock()
        self._server = _ResponsesHttpServer(("127.0.0.1", 0), _ResponsesHandler, self)
        self._thread = threading.Thread(
            target=self._server.serve_forever,
            name="mock-responses-api",
            daemon=True,
        )

    def __enter__(self) -> MockResponsesServer:
        self._thread.start()
        return self

    def __exit__(self, _exc_type: object, _exc: object, _tb: object) -> None:
        self.close()

    @property
    def url(self) -> str:
        """Return the base URL for app-server config."""
        host, port = self._server.server_address
        return f"http://{host}:{port}"

    def close(self) -> None:
        """Stop the background HTTP server thread."""
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=2)

    def enqueue_sse(
        self,
        body: str,
        *,
        delay_between_events_s: float = 0.0,
    ) -> None:
        """Queue one SSE body for the next `/v1/responses` request."""
        self._responses.put(
            MockSseResponse(
                body=body,
                delay_between_events_s=delay_between_events_s,
            )
        )

    def enqueue_assistant_message(self, text: str, *, response_id: str = "resp-1") -> None:
        """Queue a completed assistant-message model response."""
        self.enqueue_sse(
            sse(
                [
                    ev_response_created(response_id),
                    ev_assistant_message(f"msg-{response_id}", text),
                    ev_completed(response_id),
                ]
            )
        )

    def requests(self) -> list[CapturedResponsesRequest]:
        """Return all recorded Responses API requests."""
        with self._requests_lock:
            return list(self._requests)

    def single_request(self) -> CapturedResponsesRequest:
        """Return the only recorded request, failing if the count differs."""
        requests = self.requests()
        if len(requests) != 1:
            raise AssertionError(f"expected 1 request, got {len(requests)}")
        return requests[0]

    def wait_for_requests(
        self,
        count: int,
        *,
        timeout_s: float = 5.0,
    ) -> list[CapturedResponsesRequest]:
        """Wait until at least `count` requests have been recorded."""
        deadline = time.monotonic() + timeout_s
        while time.monotonic() < deadline:
            requests = self.requests()
            if len(requests) >= count:
                return requests
            time.sleep(0.01)
        requests = self.requests()
        raise AssertionError(f"expected {count} requests, got {len(requests)}")

    def _record_request(self, handler: BaseHTTPRequestHandler, body: bytes) -> None:
        """Record one inbound HTTP request from app-server."""
        headers = {key.lower(): value for key, value in handler.headers.items()}
        request = CapturedResponsesRequest(
            method=handler.command,
            path=handler.path,
            headers=headers,
            body=body,
        )
        with self._requests_lock:
            self._requests.append(request)

    def _next_response(self) -> MockSseResponse:
        """Return the next queued SSE response or fail the HTTP request."""
        return self._responses.get_nowait()


class AppServerHarness:
    """Test fixture that points a pinned runtime app-server at MockResponsesServer."""

    def __init__(self, tmp_path: Path) -> None:
        self.tmp_path = tmp_path
        self.codex_home = tmp_path / "codex-home"
        self.workspace = tmp_path / "workspace"
        self.responses = MockResponsesServer()

    def __enter__(self) -> AppServerHarness:
        self.codex_home.mkdir()
        self.workspace.mkdir()
        self.responses.__enter__()
        self._write_config()
        return self

    def __exit__(self, _exc_type: object, _exc: object, _tb: object) -> None:
        self.responses.__exit__(_exc_type, _exc, _tb)
        shutil.rmtree(self.codex_home, ignore_errors=True)
        shutil.rmtree(self.workspace, ignore_errors=True)

    def app_server_config(self) -> AppServerConfig:
        """Build SDK config for an isolated pinned-runtime app-server process."""
        return AppServerConfig(
            cwd=str(self.workspace),
            env={
                "CODEX_HOME": str(self.codex_home),
                "CODEX_APP_SERVER_DISABLE_MANAGED_CONFIG": "1",
                "RUST_LOG": "warn",
            },
        )

    def _write_config(self) -> None:
        """Write config.toml that routes model calls to the mock server."""
        config_toml = self.codex_home / "config.toml"
        config_toml.write_text(
            f"""
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for Python SDK tests"
base_url = "{self.responses.url}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
""".lstrip()
        )


class _ResponsesHttpServer(ThreadingHTTPServer):
    """ThreadingHTTPServer carrying a reference to the owning mock."""

    def __init__(
        self,
        server_address: tuple[str, int],
        handler_class: type[BaseHTTPRequestHandler],
        mock: MockResponsesServer,
    ) -> None:
        super().__init__(server_address, handler_class)
        self.mock = mock


class _ResponsesHandler(BaseHTTPRequestHandler):
    """HTTP handler for the subset of the Responses API used by SDK tests."""

    server: _ResponsesHttpServer

    def log_message(self, _format: str, *_args: object) -> None:
        """Silence default stderr logging; pytest failures print captured requests."""
        return None

    def do_GET(self) -> None:
        """Serve a minimal `/v1/models` response if app-server asks for models."""
        if self.path.endswith("/v1/models") or self.path.endswith("/models"):
            self._send_json(
                {
                    "object": "list",
                    "data": [
                        {
                            "id": "mock-model",
                            "object": "model",
                            "created": 0,
                            "owned_by": "openai",
                        }
                    ],
                }
            )
            return
        self.send_error(404, f"unexpected GET {self.path}")

    def do_POST(self) -> None:
        """Serve queued SSE responses for `/v1/responses` requests."""
        length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(length)
        self.server.mock._record_request(self, body)

        if not (self.path.endswith("/v1/responses") or self.path.endswith("/responses")):
            self.send_error(404, f"unexpected POST {self.path}")
            return

        try:
            response = self.server.mock._next_response()
        except queue.Empty:
            self.send_error(500, "no queued SSE response")
            return

        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.end_headers()
        for chunk in response.chunks():
            self.wfile.write(chunk)
            self.wfile.flush()
            if response.delay_between_events_s:
                time.sleep(response.delay_between_events_s)

    def _send_json(self, payload: Json) -> None:
        """Write one JSON response."""
        body = json.dumps(payload).encode("utf-8")
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def sse(events: list[Json]) -> str:
    """Build an SSE body from Responses API event JSON objects."""
    chunks: list[str] = []
    for event in events:
        event_type = event["type"]
        chunks.append(f"event: {event_type}\ndata: {json.dumps(event)}\n")
    return "\n".join(chunks) + "\n"


def ev_response_created(response_id: str) -> Json:
    """Return a minimal `response.created` event."""
    return {"type": "response.created", "response": {"id": response_id}}


def ev_completed(response_id: str) -> Json:
    """Return a minimal `response.completed` event with usage."""
    return {
        "type": "response.completed",
        "response": {
            "id": response_id,
            "usage": {
                "input_tokens": 1,
                "input_tokens_details": None,
                "output_tokens": 1,
                "output_tokens_details": None,
                "total_tokens": 2,
            },
        },
    }


def ev_completed_with_usage(
    response_id: str,
    *,
    input_tokens: int,
    cached_input_tokens: int,
    output_tokens: int,
    reasoning_output_tokens: int,
    total_tokens: int,
) -> Json:
    """Return `response.completed` with explicit token accounting."""
    return {
        "type": "response.completed",
        "response": {
            "id": response_id,
            "usage": {
                "input_tokens": input_tokens,
                "input_tokens_details": {"cached_tokens": cached_input_tokens},
                "output_tokens": output_tokens,
                "output_tokens_details": {
                    "reasoning_tokens": reasoning_output_tokens,
                },
                "total_tokens": total_tokens,
            },
        },
    }


def ev_assistant_message(item_id: str, text: str) -> Json:
    """Return a completed assistant message output item."""
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": item_id,
            "content": [{"type": "output_text", "text": text}],
        },
    }


def ev_message_item_added(item_id: str, text: str = "") -> Json:
    """Return an assistant message added event before streaming deltas."""
    return {
        "type": "response.output_item.added",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": item_id,
            "content": [{"type": "output_text", "text": text}],
        },
    }


def ev_output_text_delta(delta: str) -> Json:
    """Return an output-text delta event."""
    return {
        "type": "response.output_text.delta",
        "delta": delta,
    }


def ev_function_call(call_id: str, name: str, arguments: str) -> Json:
    """Return a completed function-call output item."""
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
        },
    }


def ev_failed(response_id: str, message: str) -> Json:
    """Return a failed model response event."""
    return {
        "type": "response.failed",
        "response": {
            "id": response_id,
            "error": {"code": "server_error", "message": message},
        },
    }
