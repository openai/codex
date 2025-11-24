# Execpolicy quickstart

Codex can enforce your own rules-based execution policy before it runs shell commands. Policies live in Starlark `.codexpolicy` files under `~/.codex/policy`.

## Create a policy

1. Create a policy directory: `mkdir -p ~/.codex/policy`.
2. Add one or more `.codexpolicy` files in that folder. Codex automatically loads every `.codexpolicy` file in there on startup.
3. Write `prefix_rule` entries to describe the commands you want to allow, prompt, or block:

```starlark
prefix_rule(
    pattern = ["git", ["push", "fetch"]],
    decision = "prompt",  # allow | prompt | forbidden
    match = [["git", "push", "origin", "main"]],  # examples that must match
    not_match = [["git", "status"]],              # examples that must not match
)
```

- `pattern` is a list of shell tokens, evaluated from left to right; wrap tokens in a nested list to express alternatives (for example, match both `push` and `fetch`).
- `decision` sets the severity; Codex picks the strictest decision when multiple rules match (forbidden > prompt > allow).
- `match` and `not_match` act as optional unit tests. Codex validates them when it loads your policy, so you get feedback if an example has unexpected behavior.

In this example rule, if Codex wants to run commands with the prefix `git push` or `git fetch`, it will first ask for user approval.

## Preview decisions

Use the `codex execpolicy check` subcommand to preview decisions before you save a rule (see the [`codex-execpolicy` README](../codex-rs/execpolicy/README.md) for syntax details):

```shell
codex execpolicy check --policy ~/.codex/policy/default.codexpolicy git push origin main
```

Pass multiple `--policy` flags to test how several files combine, and use `--pretty` for formatted JSON output. See the [`codex-rs/execpolicy` README](../codex-rs/execpolicy/README.md) for a more detailed walkthrough of the available syntax.

## Status

`execpolicy` commands are still in preview. The API may have breaking changes in the future.
