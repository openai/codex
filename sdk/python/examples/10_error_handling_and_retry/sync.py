import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src

ensure_local_sdk_src()

from codex_app_server import (
    Codex,
    JsonRpcError,
    ServerBusyError,
    TextInput,
    TurnStatus,
    retry_on_overload,
)

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})

    try:
        result = retry_on_overload(
            lambda: thread.turn(TextInput("Summarize retry best practices in 3 bullets.")).run(),
            max_attempts=3,
            initial_delay_s=0.25,
            max_delay_s=2.0,
        )
    except ServerBusyError as exc:
        print("Server overloaded after retries:", exc.message)
        print("Text:")
    except JsonRpcError as exc:
        print(f"JSON-RPC error {exc.code}: {exc.message}")
        print("Text:")
    else:
        if result.status == TurnStatus.failed:
            print("Turn failed:", result.error)
        print("Text:", result.text)
