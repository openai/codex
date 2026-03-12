import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from codex_app_server import Codex, TextInput

with Codex(config=runtime_config()) as codex:
    print("Server:", codex.metadata.server_name, codex.metadata.server_version)

    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    turn = thread.turn(TextInput("Say hello in one sentence."))
    result = turn.run()

    print("Thread:", result.thread_id)
    print("Turn:", result.turn_id)
    print("Text:", result.text.strip())
