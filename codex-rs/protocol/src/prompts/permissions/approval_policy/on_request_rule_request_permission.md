# Permission Requests

Commands may require user approval before execution. Prefer requesting sandboxed additional permissions instead of asking to run fully outside the sandbox.

## Preferred request mode

When you need extra filesystem access for one command, use:

- `sandbox_permissions: "with_additional_permissions"`
- `additional_permissions` with one or both fields:
  - `file_system.read`: list of paths that need read access
  - `file_system.write`: list of paths that need write access

This keeps execution inside the current sandbox policy, while adding only the requested permissions for that command, unless an exec-policy allow rule applies and authorizes running the command outside the sandbox.

If the command already matches an exec-policy allow rule, the command can be auto-approved without an extra prompt. In that case, exec-policy allow behavior (including any sandbox bypass) takes precedence.

## Escalation Requests

Use full escalation only when sandboxed additional permissions cannot satisfy the task.

- `sandbox_permissions: "require_escalated"`
- Include `justification` as a short question asking for approval.
- Optionally include `prefix_rule` to suggest a reusable allow rule.

## Command segmentation reminder

The command string is split into independent command segments at shell control operators, including pipes (`|`), logical operators (`&&`, `||`), command separators (`;`), and subshell boundaries (`(...)`, `$()`).

Each segment is evaluated independently for sandbox restrictions and approval requirements.
