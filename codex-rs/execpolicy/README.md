# codex-execpolicy

## Overview
- Policy engine and CLI built around `prefix_rule(pattern=[...], decision?, match?, not_match?)`.
- This release covers the prefix-rule subset of the execpolicy language; a richer language will follow.
- Tokens are matched in order; any `pattern` element may be a list to denote alternatives. `decision` defaults to `allow`; valid values: `allow`, `prompt`, `forbidden`.
- `match` / `not_match` supply example invocations that are validated at load time (think of them as unit tests); examples can be token arrays or strings (strings are tokenized with `shlex`).
- The CLI always prints the JSON serialization of the evaluation result.
- The legacy rule matcher lives in `codex-execpolicy-legacy`.

## Policy shapes
- Prefix rules use Starlark syntax:
```starlark
prefix_rule(
    pattern = ["cmd", ["alt1", "alt2"]], # ordered tokens; list entries denote alternatives
    decision = "prompt",                 # allow | prompt | forbidden; defaults to allow
    match = [["cmd", "alt1"], "cmd alt2"],           # examples that must match this rule
    not_match = [["cmd", "oops"], "cmd alt3"],       # examples that must not match this rule
)
```

## CLI
- From the Codex CLI, run `codex execpolicy check` subcommand with one or more policy files (for example `src/default.rules`) to check a command:
```bash
codex execpolicy check --rules path/to/policy.rules git status
```
- Pass multiple `--rules` flags to merge rules, evaluated in the order provided, and use `--pretty` for formatted JSON.
- You can also run the standalone dev binary directly during development:
```bash
cargo run -p codex-execpolicy -- check --rules path/to/policy.rules git status
```
- Example outcomes:
  - Match: `{"matchedRules":[{...}],"decision":"allow"}`
  - No match: `{"matchedRules":[]}`

## Response shape
```json
{
  "matchedRules": [
    {
      "prefixRuleMatch": {
        "matchedPrefix": ["<token>", "..."],
        "decision": "allow|prompt|forbidden"
      }
    }
  ],
  "decision": "allow|prompt|forbidden"
}
```
- When no rules match, `matchedRules` is an empty array and `decision` is omitted.
- `matchedRules` lists every rule whose prefix matched the command; `matchedPrefix` is the exact prefix that matched.
- The effective `decision` is the strictest severity across all matches (`forbidden` > `prompt` > `allow`).

Note: `execpolicy` commands are still in preview. The API may have breaking changes in the future.
