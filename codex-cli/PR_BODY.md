This PR adds a lightweight “plan → diff” preview for large apply_patch operations to make code changes easier to skim before approval:

- In TerminalChatToolCallApplyPatch (src/components/chat/terminal-chat-tool-call-item.tsx), we now loop over all parsed patch ops and render a succinct plan:
  - “Add file foo.js”
  - “Delete file bar.ts”
  - “Update file baz.txt (+3/–2 lines)” 
- In getCommandConfirmation (src/components/chat/terminal-chat.tsx), if an apply_patch command has more than 5 lines, we swap in the new plan‑view instead of the generic shell‑command widget.
- Removed some unused variables and tightened up types in the patch viewer.
- All existing tests, lint rules, and type‑checks pass.

**How to test:**
1. Run the full test suite and verify:
   ```bash
   npm test && npm run lint && npm run typecheck
   ```
2. Start the CLI on any repo, ask the agent to generate a multi‑hunk patch (e.g. `apply_patch << 'EOF'…`), and confirm you see the new “Plan” summary before the raw diff.
