# Pin Codex development tool paths

Pins constrain absolute-path fallback to binaries trusted by each developer.

From the development shell's repository root:

```sh
mkdir -p ~/.codex/rules
python3 scripts/setup_dev_execpolicy.py \
  > ~/.codex/rules/codex_dev_tool_paths.rules
```

They constrain absolute executable paths for the repository's
[development rules](../../.codex/rules/development.rules). Rules load when a
Codex session starts.
