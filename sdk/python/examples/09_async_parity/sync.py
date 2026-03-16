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
    server_label,
)

ensure_local_sdk_src()

from codex_app_server import Codex, TextInput

with Codex(config=runtime_config()) as codex:
    print("Server:", server_label(codex.metadata))

    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    turn = thread.turn(TextInput("Say hello in one sentence."))
    result = turn.run()
    persisted = thread.read(include_turns=True)
    persisted_turn = find_turn_by_id(persisted.thread.turns, result.id)

    print("Thread:", thread.id)
    print("Turn:", result.id)
    print("Text:", assistant_text_from_turn(persisted_turn).strip())
