import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from codex_app_server import Codex, TextInput

with Codex(config=runtime_config()) as codex:
    # Create an initial thread and turn so we have a real thread to resume.
    original = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    first = original.turn(TextInput("Tell me one fact about Saturn.")).run()
    print("Created thread:", first.thread_id)

    # Resume the existing thread by ID.
    resumed = codex.thread_resume(first.thread_id)
    second = resumed.turn(TextInput("Continue with one more fact.")).run()
    print(second.text)
