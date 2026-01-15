# codex-linux-sandbox

This crate is responsible for producing:

- a `codex-linux-sandbox` standalone executable for Linux that is bundled with the Node.js version of the Codex CLI
- a lib crate that exposes the business logic of the executable as `run_main()` so that
  - the `codex-exec` CLI can check if its arg0 is `codex-linux-sandbox` and, if so, execute as if it were `codex-linux-sandbox`
  - this should also be true of the `codex` multitool CLI

## Filesystem sandboxing (Linux)

When the sandbox policy allows workspace writes, the Linux sandbox relies on
Landlock rules to restrict write access to the configured writable roots (plus
`/dev/null`). It does not apply additional read-only bind mounts for Git or
Codex metadata.

### Quick manual test

Run the sandbox directly with a workspace-write policy (from a Git repository
root):

```bash
codex-linux-sandbox \
  --sandbox-policy-cwd "$PWD" \
  --sandbox-policy '{"type":"workspace-write"}' \
  -- bash -lc '
set -euo pipefail

echo "should fail" > /tmp/codex-sandbox-outside-root && exit 1 || true
echo "ok" > sandbox-write-test.txt
'
```

Expected behavior:

- Writing outside the writable root (for example `/tmp/codex-sandbox-outside-root`) fails.
- Writing a normal repo file succeeds.
