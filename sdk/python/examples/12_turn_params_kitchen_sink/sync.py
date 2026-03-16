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

from codex_app_server import (
    AskForApproval,
    Codex,
    Personality,
    ReasoningEffort,
    ReasoningSummary,
    SandboxPolicy,
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

SANDBOX_POLICY = SandboxPolicy.model_validate(
    {
        "type": "readOnly",
        "access": {"type": "fullAccess"},
    }
)
SUMMARY = ReasoningSummary.model_validate("concise")

PROMPT = (
    "Analyze a safe rollout plan for enabling a feature flag in production. "
    "Return JSON matching the requested schema."
)
APPROVAL_POLICY = AskForApproval.model_validate("never")

with Codex(config=runtime_config()) as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

    turn = thread.turn(
        TextInput(PROMPT),
        approval_policy=APPROVAL_POLICY,
        cwd=str(Path.cwd()),
        effort=ReasoningEffort.medium,
        model="gpt-5.4",
        output_schema=OUTPUT_SCHEMA,
        personality=Personality.pragmatic,
        sandbox_policy=SANDBOX_POLICY,
        summary=SUMMARY,
    )
    result = turn.run()
    persisted = thread.read(include_turns=True)
    persisted_turn = find_turn_by_id(persisted.thread.turns, result.id)

    print("Status:", result.status)
    print("Text:", assistant_text_from_turn(persisted_turn))
    print("Items:", 0 if persisted_turn is None else len(persisted_turn.items or []))
