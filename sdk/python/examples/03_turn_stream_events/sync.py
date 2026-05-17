import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from openai_codex import Codex, TextInput

with Codex(config=runtime_config()) as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    turn = thread.turn(TextInput("Explain SIMD in 3 short bullets."))

    event_count = 0
    saw_started = False
    saw_delta = False
    completed_status = None
    completed_texts = []

    for event in turn.stream():
        event_count += 1
        if event.method == "turn/started":
            saw_started = True
            print("stream.started")
            continue
        if event.method == "item/agentMessage/delta":
            delta = event.payload.delta
            if delta:
                if not saw_delta:
                    print("assistant> ", end="", flush=True)
                print(delta, end="", flush=True)
                saw_delta = True
            continue
        if event.method == "item/completed":
            root = event.payload.item.root
            if root.type == "agentMessage":
                completed_texts.append(root.text)
            continue
        if event.method == "turn/completed":
            completed_status = event.payload.turn.status.value

    if completed_status is None:
        raise RuntimeError("stream ended without turn/completed")
    if saw_delta:
        print()
    else:
        final_text = "".join(completed_texts).strip()
        print("assistant>", final_text)

    print("stream.started.seen:", saw_started)
    print("stream.completed:", completed_status)
    print("events.count:", event_count)
