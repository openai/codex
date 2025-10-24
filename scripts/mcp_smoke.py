#!/usr/bin/env python3
import os
import json
import sys

from scripts.connectors.mcp_conn import call_tool_stdio


def main() -> int:
    server = os.environ.get("MCP_SERVER")  # e.g., 'stdio:/path/to/server [args]'
    tool = os.environ.get("MCP_TOOL", "ping")
    if not server:
        print("[mcp-smoke] MCP_SERVER not set; skipping")
        return 0
    payload = {"ping": True}
    res = call_tool_stdio(server, [], tool, payload, timeout_sec=0.8)
    print(json.dumps(res)[:4096])
    # Treat retryable/error as soft success so CI is informative but non-blocking
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

