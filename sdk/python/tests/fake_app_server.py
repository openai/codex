from __future__ import annotations

import json
import sys


def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


thread_counter = 0
turn_counter = 0

for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    msg = json.loads(raw)

    if "method" in msg and msg.get("id") is not None:
        method = msg["method"]
        req_id = msg["id"]
        params = msg.get("params") or {}

        if method == "initialize":
            send({"id": req_id, "result": {"serverInfo": {"name": "fake"}}})
        elif method == "thread/start":
            thread_counter += 1
            tid = f"thr_{thread_counter}"
            send({"id": req_id, "result": {"thread": {"id": tid, "preview": ""}}})
            send({"method": "thread/started", "params": {"thread": {"id": tid}}})
        elif method == "thread/resume":
            tid = params["threadId"]
            send({"id": req_id, "result": {"thread": {"id": tid}}})
        elif method == "thread/list":
            send({"id": req_id, "result": {"data": [{"id": "thr_1"}], "nextCursor": None}})
        elif method == "thread/read":
            tid = params["threadId"]
            send({"id": req_id, "result": {"thread": {"id": tid, "turns": []}}})
        elif method == "turn/start":
            turn_counter += 1
            turn_id = f"turn_{turn_counter}"
            tid = params["threadId"]
            send({"id": req_id, "result": {"turn": {"id": turn_id, "status": "inProgress"}}})
            send({"method": "turn/started", "params": {"turn": {"id": turn_id}}})

            if params.get("requireApproval"):
                send(
                    {
                        "id": "approval-1",
                        "method": "item/commandExecution/requestApproval",
                        "params": {
                            "threadId": tid,
                            "turnId": turn_id,
                            "itemId": "cmd-1",
                            "command": "echo hi",
                        },
                    }
                )
                # Wait for client response before proceeding.
                raw_response = sys.stdin.readline()
                if raw_response:
                    _ = json.loads(raw_response)

            send({"method": "item/agentMessage/delta", "params": {"itemId": "i1", "delta": "hello "}})
            send({"method": "item/agentMessage/delta", "params": {"itemId": "i1", "delta": "world"}})
            send({"method": "turn/completed", "params": {"threadId": tid, "turn": {"id": turn_id, "status": "completed"}}})
        elif method == "turn/interrupt":
            send({"id": req_id, "result": {}})
        elif method == "model/list":
            send({"id": req_id, "result": {"data": [{"id": "gpt-5"}]}})
        else:
            send({"id": req_id, "error": {"code": -32601, "message": f"unknown method {method}"}})

    elif "method" in msg and msg.get("id") is None:
        # notifications from client (e.g. initialized)
        pass
