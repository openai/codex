#!/usr/bin/env python3
"""Persistent rust-analyzer daemon with a tiny JSON-over-UNIX-socket control API."""

from __future__ import annotations

import argparse
import json
import os
import re
import hashlib
import select
import socket
import signal
import subprocess
import threading
import time
from pathlib import Path
from typing import Dict, Optional


def find_rust_analyzer(install_if_missing: bool) -> str:
    rustup_path = subprocess.run(
        ["rustup", "which", "--toolchain", "stable", "rust-analyzer"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        timeout=20,
    )
    if rustup_path.returncode == 0 and rustup_path.stdout.strip():
        candidate = rustup_path.stdout.strip()
        probe = subprocess.run(
            [candidate, "-V"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=10,
        )
        if probe.returncode == 0:
            return candidate

    for candidate in ["rust-analyzer"]:
        proc = subprocess.run(
            [candidate, "-V"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=10,
        )
        if proc.returncode == 0:
            return candidate

    if not install_if_missing:
        raise RuntimeError(
            "rust-analyzer not runnable. Re-run with --install-ra or install with "
            "`rustup component add rust-analyzer`."
        )

    install = subprocess.run(["rustup", "component", "add", "rust-analyzer"], check=False)
    if install.returncode != 0:
        raise RuntimeError("failed to install rust-analyzer.")
    probe = subprocess.run(
        ["rust-analyzer", "-V"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=10,
    )
    if probe.returncode != 0:
        raise RuntimeError("rust-analyzer still not runnable after install.")
    return "rust-analyzer"


def to_uri(path: Path) -> str:
    return path.resolve().as_uri()


class LspSession:
    def __init__(self, ra_bin: str, workspace: Path):
        self.proc = subprocess.Popen(
            [ra_bin],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            cwd=str(workspace),
            text=False,
            bufsize=0,
        )
        if self.proc.stdin is None or self.proc.stdout is None:
            raise RuntimeError("Unable to start rust-analyzer stdio pipes.")
        self._stdin = self.proc.stdin
        self._stdout = self.proc.stdout
        self._buffer = b""
        self._next_id = 1

    def _read_exact(self, n: int, timeout: float) -> bytes:
        out = b""
        end = time.time() + timeout
        while len(out) < n:
            remaining = end - time.time()
            if remaining <= 0:
                raise TimeoutError("timeout while reading rust-analyzer response.")
            ready, _, _ = select.select([self._stdout], [], [], remaining)
            if not ready:
                raise TimeoutError("timeout while reading rust-analyzer response.")
            chunk = os.read(self._stdout.fileno(), 65536)
            if not chunk:
                raise EOFError("rust-analyzer closed stdout.")
            out += chunk
        return out

    def recv(self, timeout: float) -> Dict:
        end = time.time() + timeout
        header_end = b"\r\n\r\n"
        while header_end not in self._buffer:
            self._buffer += self._read_exact(1, max(0.1, end - time.time()))
        header, rest = self._buffer.split(header_end, 1)
        self._buffer = rest

        match = re.search(rb"Content-Length:\s*(\d+)", header, flags=re.IGNORECASE)
        if not match:
            raise RuntimeError(f"malformed RA header: {header!r}")
        body_len = int(match.group(1))
        body = self._buffer
        if len(body) < body_len:
            body += self._read_exact(body_len - len(body), max(0.1, end - time.time()))
            self._buffer = b""
        else:
            self._buffer = body[body_len:]
            body = body[:body_len]
        return json.loads(body.decode("utf-8"))

    def send(self, message: Dict) -> None:
        payload = json.dumps(message).encode("utf-8")
        header = f"Content-Length: {len(payload)}\r\n\r\n".encode("ascii")
        self._stdin.write(header + payload)
        self._stdin.flush()

    def request(self, method: str, params: Dict, timeout: float = 120.0) -> Dict:
        req_id = self._next_id
        self._next_id += 1
        self.send(
            {
                "jsonrpc": "2.0",
                "id": req_id,
                "method": method,
                "params": params,
            }
        )
        while True:
            message = self.recv(timeout)
            if message.get("id") == req_id:
                return message

    def notify(self, method: str, params: Dict) -> None:
        self.send({"jsonrpc": "2.0", "method": method, "params": params})

    def wait_for_file_diagnostics(self, file_uri: str, timeout: float) -> int:
        deadline = time.time() + timeout
        while True:
            msg = self.recv(max(0.05, deadline - time.time()))
            if msg.get("method") == "textDocument/publishDiagnostics":
                params = msg.get("params", {})
                if params.get("uri") == file_uri:
                    return len(params.get("diagnostics", []))
            if time.time() >= deadline:
                raise TimeoutError("timed out waiting for diagnostics.")

    def close(self) -> None:
        if self.proc.poll() is None:
            self.proc.terminate()
            self.proc.wait(timeout=2)


class RaDaemon:
    def __init__(self, workspace: Path, socket_path: Path, timeout: float, install_ra: bool):
        self.workspace = workspace
        self.socket_path = socket_path
        self.timeout = timeout
        self.started_at = time.time()
        self._state_lock = threading.Lock()
        self._stop = threading.Event()
        self._requests = 0
        self._file_state: Dict[str, Dict[str, object]] = {}
        self._session = LspSession(find_rust_analyzer(install_ra), workspace)
        self._initialize_lsp()

    def _initialize_lsp(self) -> None:
        req = self._session.request(
            "initialize",
            {
                "processId": os.getpid(),
                "rootUri": to_uri(self.workspace),
                "rootPath": str(self.workspace),
                "capabilities": {},
                "workspaceFolders": [{"uri": to_uri(self.workspace), "name": self.workspace.name}],
            },
            timeout=self.timeout,
        )
        if req.get("error"):
            raise RuntimeError(f"initialize error: {req['error']!r}")
        self._session.notify("initialized", {})

    def check_file(self, file_path: Path, label: Optional[str]) -> Dict:
        file_path = file_path.expanduser().resolve()
        if self.workspace not in file_path.parents and file_path.parent != self.workspace:
            return {"ok": False, "error": "file is outside workspace"}

        file_uri = to_uri(file_path)
        current_text = file_path.read_text(encoding="utf-8")
        with self._state_lock:
            self._requests += 1
            state = self._file_state.get(file_uri)
            previous_text = ""
            version = 1
            if state:
                previous_text = state.get("text", "")
                version = int(state.get("version", 1)) + 1

            if state is None:
                self._session.notify(
                    "textDocument/didOpen",
                    {
                        "textDocument": {
                            "uri": file_uri,
                            "languageId": "rust",
                            "version": version,
                            "text": current_text,
                        }
                    },
                )
                self._file_state[file_uri] = {
                    "text": current_text,
                    "version": version,
                    "diagnostics": 0,
                }
                changed = True
            else:
                changed = current_text != previous_text
                if changed:
                    self._session.notify(
                        "textDocument/didChange",
                        {
                            "textDocument": {"uri": file_uri, "version": version},
                            "contentChanges": [{"text": current_text}],
                        },
                    )
                    self._file_state[file_uri]["version"] = version
                    self._file_state[file_uri]["text"] = current_text

        if not changed:
            return {
                "ok": True,
                "file": file_uri,
                "changed": False,
                "elapsed_ms": 0.0,
                "diagnostic_count": int(state["diagnostics"]) if state else 0,
                "label": label,
                "cached": True,
            }

        start = time.perf_counter()
        diag_count = self._session.wait_for_file_diagnostics(file_uri, self.timeout)
        elapsed_ms = (time.perf_counter() - start) * 1000.0
        with self._state_lock:
            self._file_state[file_uri]["diagnostics"] = diag_count

        return {
            "ok": True,
            "file": file_uri,
            "changed": True,
            "elapsed_ms": elapsed_ms,
            "diagnostic_count": int(diag_count),
            "label": label,
                "cached": False,
            }

    def request_stop(self) -> None:
        self._stop.set()

    def state(self) -> Dict:
        with self._state_lock:
            return {
                "workspace": str(self.workspace),
                "socket": str(self.socket_path),
                "uptime_seconds": round(time.time() - self.started_at, 3),
                "requests": self._requests,
                "open_files": sorted(self._file_state.keys()),
                "ra_pid": self._session.proc.pid,
            }

    def should_stop(self) -> bool:
        return self._stop.is_set()

    def close(self):
        self._session.close()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Rust-analyzer daemon")
    parser.add_argument("--workspace", required=True, help="Workspace root")
    parser.add_argument(
        "--socket",
        required=True,
        help="UNIX socket path for daemon control channel",
    )
    parser.add_argument("--timeout", type=float, default=45.0, help="LSP timeout seconds")
    parser.add_argument(
        "--install-ra",
        action="store_true",
        help="Install rust-analyzer if missing",
    )
    return parser.parse_args()


def make_socket_path(workspace: Path) -> Path:
    key = hashlib.sha1(str(workspace.resolve()).encode("utf-8")).hexdigest()[:16]
    return Path("/tmp") / f"ra-lsp-daemon-{key}.sock"


def parse_request(raw: str) -> Dict:
    req = json.loads(raw)
    if not isinstance(req, dict):
        raise ValueError("request must be a JSON object")
    return req


def handle_client(conn: socket.socket, daemon: RaDaemon) -> None:
    with conn:
        try:
            conn.settimeout(2.0)
            raw = b""
            while not raw.endswith(b"\n"):
                chunk = conn.recv(4096)
                if not chunk:
                    break
                raw += chunk
            if not raw:
                return
            request = parse_request(raw.decode("utf-8").strip())
            action = request.get("action")
            req_id = request.get("id")
            response = {"id": req_id}

            if action == "ping":
                response.update(
                    {
                        "ok": True,
                        "state": {
                            "workspace": str(daemon.workspace),
                            "socket": str(daemon.socket_path),
                        },
                    }
                )
            elif action == "state":
                response.update({"ok": True, "state": daemon.state()})
            elif action == "check":
                if "file" not in request:
                    response.update({"ok": False, "error": "missing file field"})
                else:
                    workspace = Path(request["workspace"])
                    workspace = workspace.expanduser().resolve()
                    file_path = (workspace / request["file"]).resolve()
                    response.update(
                        daemon.check_file(
                            file_path,
                            request.get("label"),
                        )
                    )
            elif action == "stop":
                response.update({"ok": True})
                daemon.request_stop()
            else:
                response.update({"ok": False, "error": f"unknown action: {action}"})

            conn.sendall((json.dumps(response) + "\n").encode("utf-8"))
        except Exception as exc:
            try:
                err = {
                    "id": req_id,
                    "ok": False,
                    "error": repr(exc),
                }
                conn.sendall((json.dumps(err) + "\n").encode("utf-8"))
            except Exception:
                pass


def main() -> int:
    args = parse_args()
    workspace = Path(args.workspace).expanduser().resolve()
    socket_path = Path(args.socket).expanduser()

    if not workspace.is_dir():
        raise RuntimeError(f"workspace does not exist: {workspace}")

    if socket_path.exists():
        socket_path.unlink()

    daemon = RaDaemon(workspace, socket_path, args.timeout, args.install_ra)

    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    server.settimeout(0.5)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(str(socket_path))
    server.listen()
    def shutdown():
        server.close()
        daemon.close()
        if socket_path.exists():
            socket_path.unlink(missing_ok=True)

    for sig in (signal.SIGTERM, signal.SIGINT):
        signal.signal(sig, lambda *_: shutdown())

    print(f"ra-lsp daemon running: workspace={workspace} socket={socket_path} pid={os.getpid()}")
    try:
        while not daemon.should_stop():
            try:
                conn, _ = server.accept()
            except (OSError, TimeoutError):
                if daemon.should_stop():
                    break
                continue
            try:
                t = threading.Thread(target=handle_client, args=(conn, daemon), daemon=True)
                t.start()
            except Exception:
                conn.close()
    finally:
        shutdown()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
