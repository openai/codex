#!/usr/bin/env python3
"""Benchmark rust-analyzer incremental diagnostic latency in one persistent session."""

from __future__ import annotations

import argparse
import difflib
import json
import os
import re
import select
import subprocess
import time
from pathlib import Path
from typing import Dict


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Time RA diagnostics after edits.")
    parser.add_argument("workspace", help="Workspace root (directory containing Cargo.toml)")
    parser.add_argument("file", help="Rust file to benchmark")
    parser.add_argument("--iterations", type=int, default=0, help="0 means infinite loop")
    parser.add_argument(
        "--log",
        default="",
        help="Path to CSV log file (default: /tmp/ra-lsp-timing.csv)",
    )
    parser.add_argument("--timeout", type=float, default=45.0, help="Per-change timeout in seconds")
    parser.add_argument(
        "--install-ra",
        action="store_true",
        help="Install rust-analyzer with rustup when missing",
    )
    return parser.parse_args()


def classify_change(old: str, new: str) -> str:
    if "".join(old.split()) == "".join(new.split()):
        return "whitespace"

    old_lines = old.splitlines()
    new_lines = new.splitlines()
    diff = list(
        difflib.unified_diff(
            old_lines,
            new_lines,
            fromfile="old",
            tofile="new",
            lineterm="",
        )
    )

    changed = [
        line
        for line in diff
        if (
            (line.startswith("+") and not line.startswith("+++"))
            or (line.startswith("-") and not line.startswith("---"))
        )
    ]
    if not changed:
        return "code"

    if all(
        line[1:].lstrip().startswith(("//", "/*", "*", "*/")) or not line[1:].strip()
        for line in changed
    ):
        return "comment"

    return "code"


def find_rust_analyzer(install_if_missing: bool) -> str:
    # Prefer the explicit stable toolchain binary even if cwd enforces an older one.
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

    candidates = ["rust-analyzer"]
    for candidate in candidates:
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
            "rust-analyzer not runnable. Re-run with --install-ra or install manually "
            "with `rustup component add rust-analyzer`."
        )

    install = subprocess.run(
        ["rustup", "component", "add", "rust-analyzer"],
        check=False,
    )
    if install.returncode != 0:
        raise RuntimeError("Failed to install rust-analyzer via rustup.")

    proc = subprocess.run(
        ["rust-analyzer", "-V"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=10,
    )
    if proc.returncode != 0:
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
                raise TimeoutError("Timeout while reading rust-analyzer response.")
            ready, _, _ = select.select([self._stdout], [], [], remaining)
            if not ready:
                raise TimeoutError("Timeout while reading rust-analyzer response.")
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
            raise RuntimeError(f"Malformed RA header: {header!r}")
        body_len = int(match.group(1))
        body = self._buffer
        if len(body) < body_len:
            body += self._read_exact(
                body_len - len(body), max(0.1, end - time.time())
            )
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
            remaining = deadline - time.time()
            if remaining <= 0:
                raise TimeoutError("Timed out waiting for diagnostics.")

    def close(self) -> None:
        if self.proc.poll() is None:
            self.proc.terminate()
            self.proc.wait(timeout=2)


def main() -> int:
    args = parse_args()
    workspace = Path(args.workspace).expanduser().resolve()
    file_path = workspace / args.file
    if not workspace.exists() or not workspace.is_dir():
        raise RuntimeError(f"workspace does not exist: {workspace}")
    if not file_path.exists():
        raise RuntimeError(f"file does not exist: {file_path}")

    log_path = Path(args.log or "/tmp/ra-lsp-timing.csv").expanduser()
    ra_bin = find_rust_analyzer(args.install_ra)
    session = LspSession(ra_bin, workspace)
    file_uri = to_uri(file_path)

    with log_path.open("a", encoding="utf-8") as log:
        if log.tell() == 0:
            log.write("ts,round,label,auto_label,elapsed_ms,diagnostic_count\n")

        init = session.request(
            "initialize",
            {
                "processId": os.getpid(),
                "rootUri": to_uri(workspace),
                "rootPath": str(workspace),
                "capabilities": {},
                "workspaceFolders": [{"uri": to_uri(workspace), "name": workspace.name}],
            },
        )
        if init.get("error"):
            raise RuntimeError(f"initialize error: {init['error']!r}")
        session.notify("initialized", {})

        previous_text = file_path.read_text(encoding="utf-8")
        version = 1
        session.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": file_uri,
                    "languageId": "rust",
                    "version": version,
                    "text": previous_text,
                }
            },
        )

        print("Initial diagnostics sync...")
        count = session.wait_for_file_diagnostics(file_uri, args.timeout)
        print(f"initial diagnostics: {count}")

        rounds = 0
        while args.iterations == 0 or rounds < args.iterations:
            prompt = input("Edit target file now, then press Enter (or 'q' to quit): ").strip()
            if prompt.lower() in {"q", "quit", "exit"}:
                break

            current_text = file_path.read_text(encoding="utf-8")
            if current_text == previous_text:
                print("No change detected; skipping.")
                continue

            label = input("Label (blank=auto): ").strip()
            auto_label = classify_change(previous_text, current_text)
            if not label:
                label = auto_label

            version += 1
            session.notify(
                "textDocument/didChange",
                {
                    "textDocument": {"uri": file_uri, "version": version},
                    "contentChanges": [
                        {"text": current_text},
                    ],
                },
            )
            started = time.time()
            diag_count = session.wait_for_file_diagnostics(file_uri, args.timeout)
            elapsed_ms = (time.time() - started) * 1000.0

            now = time.strftime("%Y-%m-%d %H:%M:%S")
            log.write(
                f"{now},{rounds + 1},{label},{auto_label},{elapsed_ms:.3f},{diag_count}\n"
            )
            log.flush()
            previous_text = current_text
            rounds += 1
            print(f"{label} | {auto_label} | {elapsed_ms:.2f} ms | diagnostics: {diag_count}")

    session.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
