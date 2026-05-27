import json
import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from openai_codex import (
    Codex,
)
from openai_codex.types import (
    Personality,
    ReasoningSummary,
)

OUTPUT_SCHEMA = {
    "type": "object",
    "properties": {
        "summary": {"type": "string"},
        "actions": {
            "type": "array",
            "items": {"type": "string"},
        },
    },
    "required": ["summary", "actions"],
    "additionalProperties": False,
}

SUMMARY = ReasoningSummary.model_validate("concise")

PROMPT = (
    "Analyze a safe rollout plan for enabling a feature flag in production. "
    "Return JSON matching the requested schema."
)

with Codex(config=runtime_config()) as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

    turn = thread.turn(
        PROMPT,
        output_schema=OUTPUT_SCHEMA,
        personality=Personality.pragmatic,
        summary=SUMMARY,
    )
    result = turn.run()
    structured_text = result.final_response.strip()
    try:
        structured = json.loads(structured_text)
    except json.JSONDecodeError as exc:
        raise RuntimeError(
            f"Expected JSON matching OUTPUT_SCHEMA, got: {structured_text!r}"
        ) from exc

    summary = structured["summary"]
    actions = structured["actions"]
    if (
        not isinstance(summary, str)
        or not isinstance(actions, list)
        or not all(isinstance(action, str) for action in actions)
    ):
        raise RuntimeError(
            f"Expected structured output with string summary/actions, got: {structured!r}"
        )

    print("Status:", result.status)
    print("summary:", summary)
    print("actions:")
    for action in actions:
        print("-", action)
    print("Items:", len(result.items))
