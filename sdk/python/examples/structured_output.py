from __future__ import annotations

from codex import Codex, TurnOptions


SCHEMA = {
    "type": "object",
    "properties": {
        "summary": {"type": "string"},
        "status": {"type": "string", "enum": ["ok", "action_required"]},
    },
    "required": ["summary", "status"],
    "additionalProperties": False,
}


def main() -> None:
    thread = Codex().start_thread()
    turn = thread.run("Summarize repository status", TurnOptions(output_schema=SCHEMA))
    print(turn.final_response)


if __name__ == "__main__":
    main()
