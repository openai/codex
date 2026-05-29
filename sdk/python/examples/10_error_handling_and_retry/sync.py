import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from openai_codex import (
    Codex,
    JsonRpcError,
    ServerBusyError,
    retry_on_overload,
)
from openai_codex.types import TurnStatus

with Codex(config=runtime_config()) as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

    try:
        result = retry_on_overload(
            lambda: thread.turn("Summarize retry best practices in 3 bullets.").run(),
            max_attempts=3,
            initial_delay_s=0.25,
            max_delay_s=2.0,
        )
    except ServerBusyError as exc:
        print("Server overloaded after retries:", exc.message)
    except JsonRpcError as exc:
        print(f"JSON-RPC error {exc.code}: {exc.message}")
    else:
        if result.status == TurnStatus.failed:
            print("Turn failed:", result.error)
        else:
            print("Text:", result.final_response)
