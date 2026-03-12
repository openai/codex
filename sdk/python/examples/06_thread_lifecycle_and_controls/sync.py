import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from codex_app_server import Codex, TextInput


with Codex(config=runtime_config()) as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    first = thread.turn(TextInput("One sentence about structured planning.")).run()
    second = thread.turn(TextInput("Now restate it for a junior engineer.")).run()

    reopened = codex.thread_resume(thread.id)
    listing_active = codex.thread_list(limit=20, archived=False)
    reading = reopened.read(include_turns=True)

    _ = reopened.set_name("sdk-lifecycle-demo")
    _ = codex.thread_archive(reopened.id)
    listing_archived = codex.thread_list(limit=20, archived=True)
    unarchived = codex.thread_unarchive(reopened.id)

    resumed_info = "n/a"
    try:
        resumed = codex.thread_resume(
            unarchived.id,
            model="gpt-5",
            config={"model_reasoning_effort": "high"},
        )
        resumed_result = resumed.turn(TextInput("Continue in one short sentence.")).run()
        resumed_info = f"{resumed_result.turn_id} {resumed_result.status}"
    except Exception as exc:
        resumed_info = f"skipped({type(exc).__name__})"

    forked_info = "n/a"
    try:
        forked = codex.thread_fork(unarchived.id, model="gpt-5")
        forked_result = forked.turn(TextInput("Take a different angle in one short sentence.")).run()
        forked_info = f"{forked_result.turn_id} {forked_result.status}"
    except Exception as exc:
        forked_info = f"skipped({type(exc).__name__})"

    compact_info = "sent"
    try:
        _ = unarchived.compact()
    except Exception as exc:
        compact_info = f"skipped({type(exc).__name__})"

    print("Lifecycle OK:", thread.id)
    print("first:", first.turn_id, first.status)
    print("second:", second.turn_id, second.status)
    print("read.turns:", len(reading.thread.turns or []))
    print("list.active:", len(listing_active.data))
    print("list.archived:", len(listing_archived.data))
    print("resumed:", resumed_info)
    print("forked:", forked_info)
    print("compact:", compact_info)
