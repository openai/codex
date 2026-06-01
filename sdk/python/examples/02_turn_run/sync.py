import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from openai_codex import Codex

with Codex(config=runtime_config()) as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    result = thread.turn("Give 3 bullets about SIMD.").run()

    print("thread_id:", thread.id)
    print("turn_id:", result.id)
    print("status:", result.status)
    if result.error is not None:
        print("error:", result.error)
    print("text:", result.final_response)
    print("items.count:", len(result.items))
