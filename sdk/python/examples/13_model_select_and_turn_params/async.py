import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src

ensure_local_sdk_src()

import asyncio

from codex_app_server import (
    AskForApproval,
    AsyncCodex,
    Personality,
    ReasoningEffort,
    ReasoningSummary,
    SandboxPolicy,
    TextInput,
)

REASONING_RANK = {
    "none": 0,
    "minimal": 1,
    "low": 2,
    "medium": 3,
    "high": 4,
    "xhigh": 5,
}


def _pick_highest_model(models):
    visible = [m for m in models if not m.hidden] or models
    known_names = {m.id for m in visible} | {m.model for m in visible}
    top_candidates = [m for m in visible if not (m.upgrade and m.upgrade in known_names)]
    pool = top_candidates or visible
    return max(pool, key=lambda m: (m.model, m.id))


def _pick_highest_turn_effort(model) -> ReasoningEffort:
    if not model.supported_reasoning_efforts:
        return ReasoningEffort.medium

    best = max(
        model.supported_reasoning_efforts,
        key=lambda option: REASONING_RANK.get(option.reasoning_effort.value, -1),
    )
    return ReasoningEffort(best.reasoning_effort.value)


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

SANDBOX_POLICY = SandboxPolicy.model_validate(
    {
        "type": "readOnly",
        "access": {"type": "fullAccess"},
    }
)
APPROVAL_POLICY = AskForApproval.model_validate("never")


async def main() -> None:
    async with AsyncCodex() as codex:
        models = await codex.models(include_hidden=True)
        selected_model = _pick_highest_model(models.data)
        selected_effort = _pick_highest_turn_effort(selected_model)

        print("selected.model:", selected_model.model)
        print("selected.effort:", selected_effort.value)

        thread = await codex.thread_start(
            model=selected_model.model,
            config={"model_reasoning_effort": selected_effort.value},
        )

        first_turn = await thread.turn(
            TextInput("Give one short sentence about reliable production releases."),
            model=selected_model.model,
            effort=selected_effort,
        )
        first = await first_turn.run()

        print("agent.message:", first.text)
        print("usage:", first.usage)

        second_turn = await thread.turn(
            TextInput("Return JSON for a safe feature-flag rollout plan."),
            approval_policy=APPROVAL_POLICY,
            cwd=str(Path.cwd()),
            effort=selected_effort,
            model=selected_model.model,
            output_schema=OUTPUT_SCHEMA,
            personality=Personality.pragmatic,
            sandbox_policy=SANDBOX_POLICY,
            summary=ReasoningSummary.model_validate("concise"),
        )
        second = await second_turn.run()

        print("agent.message.params:", second.text)
        print("usage.params:", second.usage)


if __name__ == "__main__":
    asyncio.run(main())
