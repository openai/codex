import json
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

import asyncio

from codex_app_server import (
    AskForApproval,
    AsyncCodex,
    Personality,
    ReasoningSummary,
    TextInput,
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
APPROVAL_POLICY = AskForApproval.model_validate("never")


async def main() -> None:
    async with AsyncCodex(config=runtime_config()) as codex:
        thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

        turn = await thread.turn(
            TextInput(PROMPT),
            approval_policy=APPROVAL_POLICY,
            output_schema=OUTPUT_SCHEMA,
            personality=Personality.pragmatic,
            summary=SUMMARY,
        )
        result = await turn.run()
        persisted = await thread.read(include_turns=True)
        persisted_turn = find_turn_by_id(persisted.thread.turns, result.id)
        structured_text = assistant_text_from_turn(persisted_turn).strip()
        try:
            structured = json.loads(structured_text)
        except json.JSONDecodeError as exc:
            raise RuntimeError(f"Expected JSON matching OUTPUT_SCHEMA, got: {structured_text!r}") from exc

        summary = structured.get("summary")
        actions = structured.get("actions")
        if not isinstance(summary, str) or not isinstance(actions, list) or not all(
            isinstance(action, str) for action in actions
        ):
            raise RuntimeError(
                f"Expected structured output with string summary/actions, got: {structured!r}"
            )

        print("Status:", result.status)
        print("summary:", summary)
        print("actions:")
        for action in actions:
            print("-", action)
        print("Items:", 0 if persisted_turn is None else len(persisted_turn.items or []))


if __name__ == "__main__":
    asyncio.run(main())
