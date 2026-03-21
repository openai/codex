#!/usr/bin/env python3
"""Client for the rust-analyzer LSP daemon."""

from __future__ import annotations

import argparse
import json
import hashlib
import os
import socket
import subprocess
import sys
import time
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Interact with the RA daemon")
    parser.add_argument("--workspace", required=True, help="Workspace root")
    parser.add_argument("--socket", default="", help="UNIX socket path (optional)")
    parser.add_argument(
        "--action",
        choices=["check", "state", "start", "stop"],
        default="check",
    )
    parser.add_argument(
        "--file",
        default="",
        help="Rust file relative to workspace (required for check)",
    )
    parser.add_argument("--label", default="", help="Optional label for check event")
    parser.add_argument("--timeout", type=float, default=45.0, help="Per-change timeout in seconds")
    parser.add_argument(
        "--install-ra",
        action="store_true",
        help="Install rust-analyzer if missing when starting daemon",
    )
    parser.add_argument(
        "--no-auto-start",
        action="store_true",
        help="Do not auto-start daemon if socket is missing",
    )
    parser.add_argument("--json", action="store_true", help="Output raw JSON only")
    return parser.parse_args()


def make_socket_path(workspace: Path) -> Path:
    key = hashlib.sha1(str(workspace.resolve()).encode("utf-8")).hexdigest()[:16]
    return Path("/tmp") / f"ra-lsp-daemon-{key}.sock"


def daemon_script_path() -> Path:
    return Path(__file__).resolve().parent / "ra_lsp_daemon.py"


def send_request(socket_path: Path, request: dict, timeout: float = 2.0) -> dict:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(timeout)
        client.connect(str(socket_path))
        payload = (json.dumps(request) + "\n").encode("utf-8")
        client.sendall(payload)
        response = b""
        while True:
            chunk = client.recv(4096)
            if not chunk:
                break
            response += chunk
            if response.endswith(b"\n"):
                break
        if not response:
            raise RuntimeError("daemon returned no response")
        return json.loads(response.decode("utf-8").strip())


def ensure_daemon(workspace: Path, socket_path: Path, timeout: float, install_ra: bool) -> None:
    # Cheap health probe.
    try:
        resp = send_request(socket_path, {"action": "ping", "workspace": str(workspace)})
        if resp.get("ok"):
            return
    except Exception:
        pass

    # Auto-start one daemon for this workspace.
    daemon_proc = subprocess.Popen(
        [
            sys.executable,
            str(daemon_script_path()),
            "--workspace",
            str(workspace),
            "--socket",
            str(socket_path),
            "--timeout",
            str(timeout),
        ]
        + (["--install-ra"] if install_ra else []),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )

    # Wait for readiness.
    for _ in range(40):
        time.sleep(0.1)
        try:
            resp = send_request(socket_path, {"action": "ping", "workspace": str(workspace)})
            if resp.get("ok"):
                return
        except Exception:
            continue
    raise RuntimeError("daemon did not become ready in time")


def main() -> int:
    args = parse_args()
    workspace = Path(args.workspace).expanduser().resolve()

    if args.socket:
        socket_path = Path(args.socket).expanduser()
    else:
        socket_path = make_socket_path(workspace)

    if args.action == "start":
        ensure_daemon(workspace, socket_path, args.timeout, args.install_ra)
        if args.json:
            print(json.dumps({"ok": True, "socket": str(socket_path)}))
        else:
            print(f"daemon ready on {socket_path}")
        return 0

    if not args.no_auto_start:
        ensure_daemon(workspace, socket_path, args.timeout, args.install_ra)
    else:
        try:
            send_request(socket_path, {"action": "ping", "workspace": str(workspace)})
        except Exception as exc:
            raise RuntimeError(f"daemon unavailable: {exc}")

    if args.action == "state":
        response = send_request(socket_path, {"action": "state", "workspace": str(workspace)})
        if args.json:
            print(json.dumps(response))
        else:
            state = response.get("state", {})
            print("state:")
            for key in ("workspace", "socket", "uptime_seconds", "requests", "ra_pid"):
                print(f"  {key}: {state.get(key)}")
            if state.get("open_files"):
                print("  open_files:")
                for file in state["open_files"]:
                    print(f"    - {file}")
        return 0

    if args.action == "check":
        if not args.file:
            raise RuntimeError("--file is required for action check")
        req = {
            "action": "check",
            "workspace": str(workspace),
            "file": args.file,
            "label": args.label or None,
            "timeout": args.timeout,
        }
        response = send_request(socket_path, req, timeout=max(0.5, min(120.0, args.timeout + 1)))
        if args.json:
            print(json.dumps(response))
            return 0
        if not response.get("ok"):
            print(f"check failed: {response.get('error')}")
            return 1
        print(
            f"{response.get('file')}: changed={response.get('changed')} "
            f"elapsed_ms={response.get('elapsed_ms'):.2f} "
            f"diagnostics={response.get('diagnostic_count')} cached={response.get('cached')}"
        )
        return 0

    if args.action == "stop":
        resp = send_request(socket_path, {"action": "stop", "workspace": str(workspace)})
        if args.json:
            print(json.dumps(resp))
        else:
            print("stop:", "ok" if resp.get("ok") else "failed")
        return 0

    raise RuntimeError("unknown action")


if __name__ == "__main__":
    raise SystemExit(main())
