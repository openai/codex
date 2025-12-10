import json
import threading
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import Any, Callable, Dict, Generator, Iterable, List, Optional, Tuple

DEFAULT_RESPONSE_ID = "resp_mock"
DEFAULT_MESSAGE_ID = "msg_mock"


class _ServerState:
    def __init__(self, responses: Iterable[Dict[str, Any]], status_code: int) -> None:
        self.responses = iter(responses)
        self.requests: List[RecordedRequest] = []
        self.status_code = status_code
        self.error: Optional[Exception] = None


class RecordedRequest:
    def __init__(self, body: str, headers: Dict[str, str]) -> None:
        self.body = body
        self.json = json.loads(body)
        self.headers = headers


def format_sse_event(event: Dict[str, Any]) -> str:
    return f"event: {event['type']}\n" + f"data: {json.dumps(event)}\n\n"


def start_responses_test_proxy(
    response_bodies: Iterable[Dict[str, Any]], status_code: int = HTTPStatus.OK
) -> Tuple[str, List[RecordedRequest], Callable[[], None]]:
    responses_iterable = response_bodies if isinstance(response_bodies, Generator) else list(response_bodies)
    state = _ServerState(responses_iterable, int(status_code))

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, fmt: str, *args: Any) -> None:  # pragma: no cover - silence stderr noise
            return

        def _read_body(self) -> str:
            length = int(self.headers.get("content-length", "0"))
            return self.rfile.read(length).decode("utf-8")

        def do_POST(self) -> None:  # noqa: N802
            if self.path != "/responses":
                self.send_error(HTTPStatus.NOT_FOUND)
                return
            body = self._read_body()
            state.requests.append(RecordedRequest(body, dict(self.headers)))
            try:
                response = next(state.responses)
            except Exception as exc:  # pragma: no cover - defensive
                state.error = exc
                self.send_error(HTTPStatus.INTERNAL_SERVER_ERROR, explain=str(exc))
                return

            self.send_response(state.status_code)
            self.send_header("content-type", "text/event-stream")
            self.end_headers()
            for event in response["events"]:
                self.wfile.write(format_sse_event(event).encode("utf-8"))
            self.wfile.flush()

    try:
        server = HTTPServer(("127.0.0.1", 0), Handler)
    except PermissionError as exc:
        raise RuntimeError("Cannot bind loopback HTTP server inside sandbox") from exc
    address, port = server.server_address
    url = f"http://{address}:{port}"

    def serve() -> None:
        with server:
            server.serve_forever(poll_interval=0.1)

    thread = threading.Thread(target=serve, daemon=True)
    thread.start()
    return url, state.requests, lambda: _stop_server(server, thread)


def _stop_server(server: HTTPServer, thread: threading.Thread) -> None:
    server.shutdown()
    thread.join(timeout=2)


def sse(*events: Dict[str, Any]) -> Dict[str, Any]:
    return {"kind": "sse", "events": list(events)}


def response_started(response_id: str = DEFAULT_RESPONSE_ID) -> Dict[str, Any]:
    return {"type": "response.created", "response": {"id": response_id}}


def assistant_message(text: str, item_id: str = DEFAULT_MESSAGE_ID) -> Dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": item_id,
            "content": [{"type": "output_text", "text": text}],
        },
    }


def shell_call() -> Dict[str, Any]:
    command = ["bash", "-lc", "echo 'Hello, world!'"]
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "call_id": f"call_id{threading.get_ident()}",
            "name": "shell",
            "arguments": json.dumps({"command": command, "timeout_ms": 100}),
        },
    }


def response_failed(error_message: str) -> Dict[str, Any]:
    return {"type": "error", "error": {"code": "rate_limit_exceeded", "message": error_message}}


def response_completed(response_id: str = DEFAULT_RESPONSE_ID, usage: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
    usage_payload = usage or {
        "input_tokens": 42,
        "input_tokens_details": {"cached_tokens": 12},
        "output_tokens": 5,
        "output_tokens_details": None,
        "total_tokens": 47,
    }
    return {"type": "response.completed", "response": {"id": response_id, "usage": usage_payload}}
