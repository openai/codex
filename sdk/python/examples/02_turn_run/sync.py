import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import (
    assistant_text_from_turn,
    ensure_local_sdk_src,
    find_turn_by_id,
    runtime_config,
)

ensure_local_sdk_src()

from codex_app_server import Codex, TextInput

with Codex(config=runtime_config()) as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    result = thread.turn(TextInput("Give 3 bullets about SIMD.")).run()
    persisted = thread.read(include_turns=True)
    persisted_turn = find_turn_by_id(persisted.thread.turns, result.id)

    print("thread_id:", thread.id)
    print("turn_id:", result.id)
    print("status:", result.status)
    if result.error is not None:
        print("error:", result.error)
    print("text:", assistant_text_from_turn(persisted_turn))
    print(
        "persisted.items.count:",
        0 if persisted_turn is None else len(persisted_turn.items or []),
    )
