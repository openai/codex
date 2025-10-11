from __future__ import annotations

import http.server
import json
import threading
from dataclasses import dataclass, field
from typing import Any, Dict, List


DEFAULT_RESPONSE_ID = "resp_mock"
DEFAULT_MESSAGE_ID = "msg_mock"


@dataclass
class SseEvent:
    type: str
    payload: Dict[str, Any] = field(default_factory=dict)

    def format(self) -> str:
        data = {"type": self.type, **self.payload}
        return f"event: {self.type}\n" f"data: {json.dumps(data)}\n\n"


@dataclass
class SseBody:
    events: List[SseEvent]


@dataclass
class ResponsesProxy:
    url: str
    close: Any
    requests: List[Dict[str, Any]]


class _ProxyHandler(http.server.BaseHTTPRequestHandler):
    server_version = "ResponsesProxy/1.0"

    def do_POST(self) -> None:  # noqa: N802  # pragma: no cover - parsed indirectly
        if self.path not in {"/responses", "/v1/responses"}:
            self.server.other_requests.append(  # type: ignore[attr-defined]
                {  # pragma: no cover - diagnostics
                    "method": self.command,
                    "path": self.path,
                    "headers": {k.lower(): v for k, v in self.headers.items()},
                }
            )
            self.send_error(404)
            return

        content_length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(content_length)
        json_body = json.loads(body)
        headers = {key.lower(): value for key, value in self.headers.items()}
        self.server.requests.append(  # type: ignore[attr-defined]
            {
                "body": body.decode("utf-8"),
                "json": json_body,
                "headers": headers,
                "path": self.path,
            }
        )

        status_code = self.server.status_code  # type: ignore[attr-defined]
        self.send_response(status_code)
        self.send_header("content-type", "text/event-stream")
        self.end_headers()

        bodies: List[SseBody] = self.server.response_bodies  # type: ignore[attr-defined]
        index = self.server.response_index  # type: ignore[attr-defined]
        body_index = min(index, len(bodies) - 1)
        self.server.response_index += 1

        for event in bodies[body_index].events:
            self.wfile.write(event.format().encode("utf-8"))
        self.wfile.flush()

    def log_message(self, format: str, *args: Any) -> None:  # pragma: no cover - quiet server
        return


def _run_server(server: http.server.HTTPServer) -> None:
    with server:  # type: ignore[arg-type]
        server.serve_forever(poll_interval=0.1)


def start_responses_proxy(bodies: List[SseBody], status_code: int = 200) -> ResponsesProxy:
    requests: List[Dict[str, Any]] = []
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProxyHandler)
    server.requests = requests  # type: ignore[attr-defined]
    server.other_requests = []  # type: ignore[attr-defined]
    server.response_bodies = bodies  # type: ignore[attr-defined]
    server.response_index = 0  # type: ignore[attr-defined]
    server.status_code = status_code  # type: ignore[attr-defined]

    thread = threading.Thread(target=_run_server, args=(server,), daemon=True)
    thread.start()

    host, port = server.server_address
    url = f"http://{host}:{port}"

    def close() -> None:
        server.shutdown()
        thread.join()

    return ResponsesProxy(url=url, close=close, requests=requests)


def response_started(response_id: str = DEFAULT_RESPONSE_ID) -> SseEvent:
    return SseEvent(
        type="response.created",
        payload={
            "response": {
                "id": response_id,
            }
        },
    )


def assistant_message(text: str, item_id: str = DEFAULT_MESSAGE_ID) -> SseEvent:
    return SseEvent(
        type="response.output_item.done",
        payload={
            "item": {
                "type": "message",
                "role": "assistant",
                "id": item_id,
                "content": [
                    {
                        "type": "output_text",
                        "text": text,
                    }
                ],
            }
        },
    )


def response_completed(response_id: str = DEFAULT_RESPONSE_ID) -> SseEvent:
    return SseEvent(
        type="response.completed",
        payload={
            "response": {
                "id": response_id,
                "usage": {
                    "input_tokens": 42,
                    "input_tokens_details": {"cached_tokens": 12},
                    "output_tokens": 5,
                    "output_tokens_details": None,
                    "total_tokens": 47,
                },
            }
        },
    )


def response_failed(message: str) -> SseEvent:
    return SseEvent(
        type="error",
        payload={"error": {"code": "rate_limit_exceeded", "message": message}},
    )


def sse(*events: SseEvent) -> SseBody:
    return SseBody(events=list(events))
