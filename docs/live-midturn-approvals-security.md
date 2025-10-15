Title: Security implications of live mid‑turn approval changes

Summary
- Changing approval and sandbox policy mid‑turn alters safety decisions for commands and file edits that have not yet been executed in the current turn. This feature is powerful but increases blast radius if a user elevates to broader permissions while an agent is actively proposing or executing actions.

Key risks
- Trust expansion mid‑execution: switching to Auto or Full Access can immediately convert pending or subsequent tool invocations within the same turn from “ask” to “auto‑approve”.
- Patch scope: apply_patch diffs are often large; elevating mid‑turn can approve edits not fully reviewed yet.
- Sandbox coverage: Full Access disables sandboxing; accidental elevation can allow network and non‑workspace writes.
- Social engineering: a malicious diff or tool output could encourage elevation mid‑turn.

Mitigations in this PR
- Changes apply only to subsequent approval decisions; already‑displayed approval modals are not auto‑resolved.
- Workspace boundaries enforced: “Auto” uses WorkspaceWrite sandbox; writes outside workspace still require approval.
- No tool list reshaping mid‑turn: only safety decisions are updated; tool exposure remains consistent until next turn.

Operational guidance
- Prefer raising to “Auto” (WorkspaceWrite) rather than “Full Access” when possible.
- Use `/diff` and patch previews to review changes before switching modes.
- If elevation is made by mistake, immediately switch back to a more restrictive preset; new decisions will honor the tighter policy.

PR review gate
- This change MUST receive a dedicated security review before merge. Reviewers should verify that all approval and patch execution paths consult the effective mid‑turn policies and that sandbox policies are still enforced as documented.

