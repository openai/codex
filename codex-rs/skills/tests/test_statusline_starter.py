"""Exercise the shipped statusline starter through its stdio protocol."""

import json
import os
from pathlib import Path
import subprocess
import sys
import unittest


STARTER = (
    Path(__file__).resolve().parents[1]
    / "src/assets/samples/create-statusline/assets/statusline.py"
)


class StatuslineStarterTest(unittest.TestCase):
    def test_advertised_resource_uses_each_reads_snapshot(self):
        def exchange(*messages):
            wire = "".join(
                json.dumps({"jsonrpc": "2.0", **message}, ensure_ascii=False) + "\n"
                for message in messages
            )
            result = subprocess.run(
                [sys.executable, "-u", str(STARTER)],
                input=wire,
                capture_output=True,
                encoding="utf-8",
                check=True,
                timeout=10,
                env={**os.environ, "PYTHONIOENCODING": "ascii"},
            )
            replies = [json.loads(line) for line in result.stdout.splitlines()]
            self.assertEqual(
                [reply["id"] for reply in replies],
                [message["id"] for message in messages if "id" in message],
            )
            return [reply["result"] for reply in replies]

        handshake = (
            {
                "id": 1,
                "method": "initialize",
                "params": {"protocolVersion": "2025-06-18"},
            },
            {"method": "notifications/initialized"},
        )
        initialized, listed = exchange(*handshake, {"id": 2, "method": "tools/list"})
        for capability in ("tools", "resources"):
            self.assertIn(capability, initialized["capabilities"])
        (tool,) = listed["tools"]
        (entry,) = tool["_meta"]["openai/ui"]["entrypoints"]
        self.assertEqual(entry["type"], "statusline")
        uri = entry["resourceUri"]
        self.assertTrue(uri.startswith("ui://"))

        samples = [("Model A", "working"), ("Modèle 曲", "ready")]
        reads = []
        for request_id, (model, activity) in enumerate(samples, start=2):
            state = {
                "thread": {
                    "model": {"display_name": model},
                    "agent": {"state": activity},
                }
            }
            reads.append(
                {
                    "id": request_id,
                    "method": "resources/read",
                    "params": {
                        "uri": uri,
                        "_meta": {"com.openai/codex-statusline-state": state},
                    },
                }
            )

        def leaves(node):
            if isinstance(node, str):
                yield node
            elif node is not None:
                for child in node["children"]:
                    yield from leaves(child)

        _, *replies = exchange(*handshake, *reads)
        for reply, expected in zip(replies, samples, strict=True):
            (item,) = reply["contents"]
            self.assertEqual((item["uri"], item["mimeType"]), (uri, "application/json"))
            self.assertEqual(tuple(leaves(json.loads(item["text"]))), expected)


if __name__ == "__main__":
    unittest.main()
