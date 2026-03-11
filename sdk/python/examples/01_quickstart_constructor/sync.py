import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src

ensure_local_sdk_src()

from codex_app_server import Codex, TextInput

with Codex() as codex:
    print("Server:", codex.metadata.server_name, codex.metadata.server_version)

    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    result = thread.turn(TextInput("Say hello in one sentence.")).run()
    print("Status:", result.status)
    print("Text:", result.text)
