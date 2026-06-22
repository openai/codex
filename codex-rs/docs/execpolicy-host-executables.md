# Pin Codex development tool paths

Pinning prevents trusted rules from matching an unexpected executable with the same name.

From the development shell's repository root:

```sh
mkdir -p ~/.codex/rules
python3 scripts/setup_dev_execpolicy.py \
  > ~/.codex/rules/codex_dev_tool_paths.rules
```

This pins binaries for the repository's
[development rules](../../.codex/rules/development.rules). Rules load when a
Codex session starts.
