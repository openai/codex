import json
import os
import shutil
import subprocess
from typing import Any


def call_tool_stdio(command: str, args: list[str], tool: str, payload: dict, timeout_sec: int = 1) -> dict[str, Any]:
    """
    Invoke an MCP tool via an external stdio client if available.

    Expected external client: set MCP_CLIENT_BIN in env or ensure 'codex-mcp-client' is on PATH.
    Contract (convention):
      codex-mcp-client --server "stdio:/path/to/server [args...]" --tool TOOL --timeout-ms N --params JSON

    Returns a dict with either a tool-defined result or an error/retryable envelope.
    This keeps Python light and defers protocol details to a dedicated client.
    """
    client_bin = os.environ.get("MCP_CLIENT_BIN") or "codex-mcp-client"
    if shutil.which(client_bin) is None:
        return {"status": "retryable", "error": "mcp client not found", "meta": {"client": client_bin}}

    server = "stdio:" + command if not command.startswith("stdio:") else command
    params_json = json.dumps(payload, separators=(",", ":"))
    cmd = [
        client_bin,
        "--server",
        server,
        "--tool",
        tool,
        "--timeout-ms",
        str(int(max(1, timeout_sec) * 1000)),
        "--params",
        params_json,
    ]
    try:
        p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        out, err = p.communicate(timeout=timeout_sec)
        if p.returncode != 0:
            return {"status": "error", "error": f"client rc={p.returncode}", "stderr": err[:2000]}
        try:
            return json.loads(out)
        except Exception:
            # Return text as fallback
            return {"status": "ok", "text": out[:65536]}
    except subprocess.TimeoutExpired:
        p.kill()
        return {"status": "retryable", "error": "timeout", "meta": {"timeout_sec": timeout_sec}}
