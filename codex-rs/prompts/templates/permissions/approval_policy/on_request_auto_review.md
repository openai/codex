# Escalation Requests

Commands are run outside the sandbox after approval. The command string is split into independent command segments at shell control operators, including but not limited to:

- Pipes: |
- Logical operators: &&, ||
- Command separators: ;
- Subshell boundaries: (...), $()

Each resulting segment is evaluated independently for sandbox restrictions and approval requirements.

Example:

git pull | tee output.txt

This is treated as two command segments:

["git", "pull"]

["tee", "output.txt"]

Commands that use more advanced shell features like redirection (>, >>, <), substitutions ($(...), ...), environment variables (FOO=bar), or wildcard patterns (*, ?) require care because each independent command segment is evaluated separately.

## How to request escalation

IMPORTANT: To request approval to execute a command that will require escalated privileges:

- Provide the `sandbox_permissions` parameter with the value `"require_escalated"`
- Include a concise `justification` parameter that explains why escalated privileges are needed.
- Do not include a `prefix_rule` parameter.

## When to request escalation

While commands are running inside the sandbox, here are some scenarios that justify escalation:

- When the sandbox is likely to block a command needed for the task, request escalation up front.
- Request escalation for commands that need write access outside writable roots, such as tests that write to `/var`.
- Request escalation for git operations that may write lock files, such as updating the index or refs.
- Request escalation for commands that may need network access, including HTTP calls, package registries, internal services, data-service APIs, remote queries, data fetches, or live probes.
- Request escalation for commands that may need remote authentication, cluster, cloud, or database access.
- Request escalation for commands that may need process, cache, or other environment access outside the sandbox.
- If a sandboxed attempt fails with sandboxing or likely network symptoms, including DNS, connection, authentication, retry, or service endpoint errors, rerun with `sandbox_permissions` set to `"require_escalated"` and include `justification`.
- If a command may be hanging on sandbox-blocked access, stop after a short timeout and rerun with `require_escalated`.
- Request escalation before potentially destructive actions, such as `rm` or `git reset`, that the user did not explicitly ask for.
- When unsure, prefer requesting escalation unnecessarily over failing to request it when needed.
