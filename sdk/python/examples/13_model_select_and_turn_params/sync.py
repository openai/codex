import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from openai_codex import (
    Codex,
    TextInput,
)
from openai_codex.types import (
    Personality,
    ReasoningEffort,
    ReasoningSummary,
    SandboxPolicy,
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
    visible = [m for m in models if not m.hidden]
    if not visible:
        raise RuntimeError("models response did not include visible models")

    known_names = {m.id for m in visible} | {m.model for m in visible}
    top_candidates = [m for m in visible if not (m.upgrade and m.upgrade in known_names)]
    if not top_candidates:
        raise RuntimeError("models response did not include top-level visible models")
    return max(top_candidates, key=lambda m: (m.model, m.id))


def _pick_highest_turn_effort(model) -> ReasoningEffort:
    if not model.supported_reasoning_efforts:
        raise RuntimeError(f"{model.model} did not advertise supported reasoning efforts")

    best = max(
        model.supported_reasoning_efforts,
        key=lambda option: REASONING_RANK[option.reasoning_effort.value],
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


with Codex(config=runtime_config()) as codex:
    models = codex.models(include_hidden=True)
    selected_model = _pick_highest_model(models.data)
    selected_effort = _pick_highest_turn_effort(selected_model)

    print("selected.model:", selected_model.model)
    print("selected.effort:", selected_effort.value)

    thread = codex.thread_start(
        model=selected_model.model,
        config={"model_reasoning_effort": selected_effort.value},
    )

    first = thread.turn(
        TextInput("Give one short sentence about reliable production releases."),
        model=selected_model.model,
        effort=selected_effort,
    ).run()

    print("agent.message:", first.final_response)
    print("items:", len(first.items))

    second = thread.turn(
        TextInput("Return JSON for a safe feature-flag rollout plan."),
        cwd=str(Path.cwd()),
        effort=selected_effort,
        model=selected_model.model,
        output_schema=OUTPUT_SCHEMA,
        personality=Personality.pragmatic,
        sandbox_policy=SANDBOX_POLICY,
        summary=ReasoningSummary.model_validate("concise"),
    ).run()

    print("agent.message.params:", second.final_response)
    print("items.params:", len(second.items))
