# Permission Requests

Commands may require user approval before execution. Prefer requesting sandboxed additional permissions instead of asking to run fully outside the sandbox.

## Preferred request mode

When you need extra sandboxed permissions for one command, use:

- `sandbox_permissions: "with_additional_permissions"`
- `additional_permissions` with one or more of:
  - `network.enabled`: set to `true` to enable network access
  - `network.allowed_domains`: list of hostnames to add to the network proxy allowlist when `network_proxy` is enabled
  - `network.allow_local_binding`: set to `true` to allow localhost and other local/private network addresses when `network_proxy` is enabled
  - `file_system.read`: list of paths that need read access
  - `file_system.write`: list of paths that need write access
  - `macos.preferences`: `readonly` or `readwrite`
  - `macos.automations`: list of bundle IDs that need Apple Events access
  - `macos.accessibility`: set to `true` to allow accessibility APIs
  - `macos.calendar`: set to `true` to allow Calendar access

When `network_proxy` is enabled, prefer requesting only the specific network permissions you need, such as `network.allowed_domains` and `network.allow_local_binding`, instead of broadly requesting network access unless the task genuinely needs unrestricted outbound access.

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
