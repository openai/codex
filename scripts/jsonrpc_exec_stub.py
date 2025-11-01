#!/usr/bin/env python3
"""
Build and run a single non-interactive Codex turn over app-server JSON‑RPC
and stub Responses API HTTP via the experimental responsesApi/call delegate.
This always performs `cargo build` and then runs
`cargo run --bin codex -- app-server` (per docs/install.md).

Equivalent usage to:
  codex exec --skip-git-repo-check "my prompt here"

This script:
- Starts `codex-app-server` as a child process (stdio JSON-RPC, JSONL).
- Performs the initialize → newConversation → addConversationListener flow.
- Sends your prompt via sendUserMessage.
- Hooks server→client requests with method `responsesApi/call` and, instead of
  performing real HTTP, streams a tiny synthetic Responses event sequence:
    response.created → response.output_item.done (assistant: "Hello, world!") → response.completed
- Prints the stubbed assistant message to stdout.

Requirements:
- Rust toolchain + Cargo installed locally.
- To exercise the delegate path, ensure Codex has the feature enabled. You can
  either add to your ~/.codex/config.toml:
    [features]
    responses_http_over_jsonrpc = true
  or run with --with-temp-codex-home to isolate config and auto-enable it.

Note: This script purposely does not perform any real network calls.
"""

from __future__ import annotations

import argparse
import json
import os
import queue
import sys
import tempfile
import threading
import subprocess
from dataclasses import dataclass
from typing import Any, Dict, Optional, Tuple


# ------------------------- JSON-RPC wire helpers -------------------------


RequestId = int | str


@dataclass
class JSONRPCRequest:
    id: RequestId
    method: str
    params: Optional[Dict[str, Any]] = None

    def to_wire(self) -> Dict[str, Any]:
        out = {"id": self.id, "method": self.method}
        if self.params is not None:
            out["params"] = self.params
        return out


@dataclass
class JSONRPCResponse:
    id: RequestId
    result: Dict[str, Any]

    def to_wire(self) -> Dict[str, Any]:
        return {"id": self.id, "result": self.result}


@dataclass
class JSONRPCNotification:
    method: str
    params: Optional[Dict[str, Any]] = None

    def to_wire(self) -> Dict[str, Any]:
        out = {"method": self.method}
        if self.params is not None:
            out["params"] = self.params
        return out


class JsonRpcIO:
    def __init__(self, proc: subprocess.Popen[bytes], trace: bool = True):
        self.proc = proc
        self.trace = trace
        self._next_id = 0
        self._lock = threading.Lock()
        self._incoming: "queue.Queue[Dict[str, Any]]" = queue.Queue()
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()

    def _read_loop(self) -> None:
        assert self.proc.stdout is not None
        for raw in self.proc.stdout:
            line = raw.decode("utf-8", errors="replace").rstrip("\n")
            if not line:
                continue
            if self.trace:
                print(f"[jsonrpc <-] {line}", file=sys.stderr)
            try:
                msg = json.loads(line)
            except Exception as e:
                print(f"[client] failed to parse line: {e}: {line}", file=sys.stderr)
                continue
            self._incoming.put(msg)

    def next_id(self) -> int:
        with self._lock:
            rid = self._next_id
            self._next_id += 1
            return rid

    def send_obj(self, obj: Dict[str, Any]) -> None:
        payload = json.dumps(obj, separators=(",", ":"))
        if self.trace:
            print(f"[jsonrpc ->] {payload}", file=sys.stderr)
        assert self.proc.stdin is not None
        self.proc.stdin.write(payload.encode("utf-8") + b"\n")
        self.proc.stdin.flush()

    def send_request(self, method: str, params: Optional[Dict[str, Any]] = None) -> RequestId:
        rid = self.next_id()
        self.send_obj(JSONRPCRequest(id=rid, method=method, params=params).to_wire())
        return rid

    def send_response(self, id: RequestId, result: Dict[str, Any]) -> None:
        self.send_obj(JSONRPCResponse(id=id, result=result).to_wire())

    def send_notification(self, method: str, params: Optional[Dict[str, Any]] = None) -> None:
        self.send_obj(JSONRPCNotification(method=method, params=params).to_wire())

    def recv(self, timeout: Optional[float] = None) -> Optional[Dict[str, Any]]:
        try:
            return self._incoming.get(timeout=timeout)
        except queue.Empty:
            return None


# ----------------------------- App flow logic ----------------------------


def _exe_suffix() -> str:
    return ".exe" if os.name == "nt" else ""


def project_root() -> str:
    # scripts/ is directly under the repo root
    return os.path.abspath(os.path.join(os.path.dirname(__file__), os.pardir))


def codex_rs_dir() -> str:
    return os.path.join(project_root(), "codex-rs")


def default_server_path(release: bool) -> str:
    target = "release" if release else "debug"
    return os.path.join(codex_rs_dir(), "target", target, f"codex-app-server{_exe_suffix()}")


def build_app_server(release: bool, offline: bool) -> str:
    """Build codex-app-server and return the path to the binary.

    Follows docs/install.md guidance by invoking `cargo build` from the
    codex-rs workspace root.
    """
    bin_path = default_server_path(release)

    # Fast path: binary already exists, skip build
    if os.path.exists(bin_path):
        return bin_path

    cmd = ["cargo", "build"]
    if release:
        cmd.append("--release")
    if offline:
        cmd.append("--offline")

    print(f"[client] building app-server: {' '.join(cmd)} (cwd={codex_rs_dir()})", file=sys.stderr)
    try:
        subprocess.run(cmd, cwd=codex_rs_dir(), check=True)
    except FileNotFoundError:
        print(
            "[client] cargo not found. Install Rust per docs/install.md (rustup) and re-run.",
            file=sys.stderr,
        )
        raise SystemExit(127)
    except subprocess.CalledProcessError as e:
        # Retry offline if not already tried, as a best-effort use of local cache
        if not offline:
            try:
                print("[client] build failed; retrying with --offline", file=sys.stderr)
                subprocess.run(cmd + ["--offline"], cwd=codex_rs_dir(), check=True)
            except subprocess.CalledProcessError:
                raise SystemExit(e.returncode)
        else:
            raise SystemExit(e.returncode)

    if not os.path.exists(bin_path):
        print(
            f"[client] build succeeded but binary not found at {bin_path}.\n"
            f"         If your workspace default-members omit app-server, try: cargo build -p codex-app-server",
            file=sys.stderr,
        )
        raise SystemExit(2)

    return bin_path


def write_temp_config(codex_home: str) -> None:
    os.makedirs(codex_home, exist_ok=True)
    cfg_path = os.path.join(codex_home, "config.toml")
    # Minimal toggle for the delegate; leave provider/model defaults to user.
    with open(cfg_path, "w", encoding="utf-8") as f:
        f.write("""[features]
responses_http_over_jsonrpc = true
""")


def run_exec_over_jsonrpc(
    prompt: str,
    server: Optional[str],
    hello_text: str,
    use_temp_home: bool,
    cwd: Optional[str],
    auto_build: bool,
    release: bool,
    offline: bool,
    trace_jsonrpc: bool,
) -> int:
    env = os.environ.copy()
    tmpdir: Optional[tempfile.TemporaryDirectory[str]] = None

    if use_temp_home:
        tmpdir = tempfile.TemporaryDirectory(prefix="codex-home-")
        env["CODEX_HOME"] = tmpdir.name
        write_temp_config(tmpdir.name)
    # Make server a little chattier for troubleshooting.
    env.setdefault("RUST_LOG", "info")

    # Spawn app-server either via cargo run (preferred) or direct binary.
    proc, used_cargo_run = spawn_server_process(server, env, release, offline, auto_build)

    rpc = JsonRpcIO(proc, trace=trace_jsonrpc)

    # initialize
    init_id = rpc.send_request(
        "initialize",
        {
            "clientInfo": {
                "name": "jsonrpc-stub-client",
                "title": None,
                "version": "0.1.0",
            }
        },
    )

    # Wait for initialize response before sending initialized notification.
    init_timeout = 180 if used_cargo_run else 30
    while True:
        msg = rpc.recv(timeout=init_timeout)
        if msg is None:
            print(
                "Timed out waiting for initialize response. If building via cargo run, try again or use --release for faster startup.",
                file=sys.stderr,
            )
            return 2
        if "result" in msg and msg.get("id") == init_id:
            break
        if "error" in msg and msg.get("id") == init_id:
            print(f"initialize error: {msg['error']}", file=sys.stderr)
            return 2
        # Buffer/ignore anything else until we get the init response.

    rpc.send_notification("initialized")

    # newConversation (default settings); optionally override cwd.
    new_conv_params: Dict[str, Any] = {}
    if cwd:
        new_conv_params["cwd"] = cwd
    # Important: always send a params object (even if empty) to satisfy the
    # ClientRequest::NewConversation schema.
    new_conv_id = rpc.send_request("newConversation", new_conv_params)

    conversation_id: Optional[str] = None
    while True:
        msg = rpc.recv(timeout=30)
        if msg is None:
            print("Timed out waiting for newConversation response", file=sys.stderr)
            return 2
        if "result" in msg and msg.get("id") == new_conv_id:
            try:
                conversation_id = msg["result"]["conversationId"]
            except Exception as e:
                print(f"Malformed newConversation response: {e}", file=sys.stderr)
                return 2
            break
        if "error" in msg and msg.get("id") == new_conv_id:
            print(f"newConversation error: {msg['error']}", file=sys.stderr)
            return 2

    # Subscribe to raw events (handy for debugging/consuming output items if desired).
    rpc.send_request(
        "addConversationListener",
        {"conversationId": conversation_id, "experimentalRawEvents": True},
    )

    # Send the user message with your prompt.
    send_id = rpc.send_request(
        "sendUserMessage",
        {
            "conversationId": conversation_id,
            "items": [
                {
                    "type": "text",
                    "data": {"text": prompt},
                }
            ],
        },
    )

    # Handle server→client delegated HTTP calls and stream notifications until
    # the task completes. Accumulate assistant text from raw_response_item.
    final_ok = False
    saw_responses_call = False
    assistant_chunks: list[str] = []
    while True:
        msg = rpc.recv(timeout=120)
        if msg is None:
            print("Timed out waiting for server messages", file=sys.stderr)
            return 2

        # Server → Client request: responsesApi/call
        if msg.get("method") == "responsesApi/call" and "id" in msg:
            req_id = msg["id"]
            params = msg.get("params", {})
            call_id = params.get("callId", "resp_stub")
            url = params.get("url", "")
            saw_responses_call = True
            print(f"[client] intercept responsesApi/call callId={call_id} url={url}", file=sys.stderr)

            # Stream hello‑world output back as Responses events.
            created = {"type": "response.created", "response": {"id": call_id}}
            message_done = {
                "type": "response.output_item.done",
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "output_text", "text": hello_text},
                    ],
                },
            }
            completed = {"type": "response.completed", "response": {"id": call_id}}

            rpc.send_notification(
                "responsesApi/event",
                {"callId": call_id, "event": created},
            )
            rpc.send_notification(
                "responsesApi/event",
                {"callId": call_id, "event": message_done},
            )
            rpc.send_notification(
                "responsesApi/event",
                {"callId": call_id, "event": completed},
            )

            # Finalize the delegated request with a 200 status.
            rpc.send_response(req_id, {"status": 200, "requestId": None, "error": None})
            continue

        # Collect assistant text from raw events if present.
        if msg.get("method") == "codex/event/raw_response_item" and msg.get("params"):
            try:
                params = msg["params"]
                item = params.get("item")
                if isinstance(item, dict) and item.get("type") == "message" and item.get("role") == "assistant":
                    for c in item.get("content", []) or []:
                        if isinstance(c, dict) and c.get("type") == "output_text" and isinstance(c.get("text"), str):
                            assistant_chunks.append(c["text"])
            except Exception:
                pass

        # End when the task completes.
        if msg.get("method") == "codex/event/task_complete":
            break

        # sendUserMessage completed
        if "result" in msg and msg.get("id") == send_id:
            final_ok = True
            continue
        if "error" in msg and msg.get("id") == send_id:
            print(f"sendUserMessage error: {msg['error']}", file=sys.stderr)
            return 2

        # Optionally, one could watch for raw output items here:
        # if msg.get("method") == "codex/event/raw_response_item": print(msg)

    # Print the assistant text like `codex exec` would.
    output = "".join(assistant_chunks).strip()
    if output:
        print(output)
    elif saw_responses_call:
        # Fallback if raw events were somehow missed but we did stub the call.
        print(hello_text)
    else:
        print("[no assistant output received]", file=sys.stderr)
        print(
            "[hint] No responsesApi/call observed. Enable the feature: --with-temp-codex-home or set [features].responses_http_over_jsonrpc=true in config.toml",
            file=sys.stderr,
        )

    # Clean up child process.
    try:
        proc.terminate()
    except Exception:
        pass
    if tmpdir is not None:
        tmpdir.cleanup()
    return 0 if final_ok else 1


def spawn_server_process(
    server: Optional[str], env: Dict[str, str], release: bool, offline: bool, auto_build: bool
) -> Tuple[subprocess.Popen[bytes], bool]:
    """Start app-server by building the workspace and running via Cargo.

    Always follows: `cargo build` then `cargo run --bin codex -- app-server`.
    Returns (process, used_cargo_run=True).
    """
    # Build workspace first.
    _ = build_app_server(release=release, offline=offline)

    # Run via cargo
    cmd = ["cargo", "run"]
    if release:
        cmd.append("--release")
    if offline:
        cmd.append("--offline")
    cmd += [
        "--bin",
        "codex",
        "--",
        # Force‑enable the delegate feature so the client sees responsesApi/call.
        "--enable",
        "responses_http_over_jsonrpc",
        "app-server",
    ]
    print(
        f"[client] starting via cargo run: {' '.join(cmd)} (cwd={codex_rs_dir()})",
        file=sys.stderr,
    )
    try:
        proc = subprocess.Popen(
            cmd,
            cwd=codex_rs_dir(),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=sys.stderr,
            env=env,
        )
    except FileNotFoundError:
        print(
            "[client] cargo not found. Install Rust per docs/install.md (rustup).",
            file=sys.stderr,
        )
        raise SystemExit(127)
    return proc, True


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("prompt", help="Prompt to send (like codex exec)")
    p.add_argument(
        "--server",
        default=None,
        help="Path to codex-app-server binary (default: auto-build + local target)",
    )
    p.add_argument(
        "--hello-text",
        default="Hello, world!",
        help="Stubbed assistant text streamed back via Responses events",
    )
    p.add_argument(
        "--with-temp-codex-home",
        action="store_true",
        help="Use a temp CODEX_HOME and auto-enable responses_http_over_jsonrpc",
    )
    p.add_argument(
        "--cwd",
        default=None,
        help="Set conversation working directory (server-side)",
    )
    build = p.add_argument_group("build")
    build.add_argument(
        "--build",
        dest="build",
        action="store_true",
        help="Build codex-app-server before running (default when no --server is provided)",
    )
    build.add_argument(
        "--no-build",
        dest="build",
        action="store_false",
        help="Do not build automatically (expects --server or existing local binary)",
    )
    p.set_defaults(build=True)
    build.add_argument(
        "--release",
        action="store_true",
        help="Build/run the release binary",
    )
    build.add_argument(
        "--offline",
        action="store_true",
        help="Pass --offline to cargo build (use local cache only)",
    )
    trace = p.add_argument_group("trace")
    trace.add_argument(
        "--trace-jsonrpc",
        dest="trace_jsonrpc",
        action="store_true",
        default=True,
        help="Log every JSON-RPC message sent/received (default: on)",
    )
    trace.add_argument(
        "--no-trace-jsonrpc",
        dest="trace_jsonrpc",
        action="store_false",
        help="Disable JSON-RPC message tracing",
    )
    return p.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    # Default to temp CODEX_HOME when none is set for a turnkey experience.
    use_temp_home_flag = bool(args.with_temp_codex_home)
    use_temp_home_auto = "CODEX_HOME" not in os.environ
    return run_exec_over_jsonrpc(
        prompt=args.prompt,
        server=args.server,
        hello_text=args.hello_text,
        use_temp_home=use_temp_home_flag or use_temp_home_auto,
        cwd=args.cwd,
        auto_build=bool(args.build or (args.server is None)),
        release=bool(args.release),
        offline=bool(args.offline),
        trace_jsonrpc=bool(args.trace_jsonrpc),
    )


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
