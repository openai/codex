---
name: protocol-breaking-changes
description: Test app-server protocol edits for stable client breakage, triage compatibility-lint failures, and record intentional request-schema breaks. Use for changes or CI failures involving the app-server wire format.
---

# Protocol breaking changes

Regenerate the stable schema and compare it with the merge base:

```sh
just app-server-schema-lint
```

This uses `origin/main` by default. Set `CODEX_SCHEMA_BASE_REF` to compare with another fetched base.

Treat removed methods or fields, newly required fields, and narrower types, enums, unions, or constraints as breaking.
The command exits non-zero for every detected breakage, including recorded ones.

For a failure, read its method, path, before value, and after value. Inspect the generating protocol type, especially optional or nullable inputs made mandatory. Fix accidental narrowing and rerun.

For an intentional break, agents must show the human its method, path, before/after values, and proposed justification, then get explicit permission to change the protocol and log. After approval, append the emitted `[[breakages]]` block to `codex-rs/app-server-protocol/stable-api-breakages.toml`. Discuss only wire-format rationale and client-visible compatibility action. Never include internal details, plans, codenames, timelines, customer identities, or other confidential information. Never edit, reorder, or delete existing entries. Rerun the lint to verify the entry.
