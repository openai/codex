## Sandbox & approvals

What Codex is allowed to do is governed by a combination of **sandbox modes** (what Codex is allowed to do without supervision) and **approval policies** (when you must confirm an action). This page explains the options, how they interact, and how the sandbox behaves on each platform.

### Approval policies

Codex starts conservatively. Until you explicitly tell it a working directory is trusted, the CLI defaults to **read-only**. Codex can inspect files and answer questions, but every edit or command requires approval.

When you mark a working directory as trusted (for example via the onboarding prompt or `/approvals` → “Trust this directory”), Codex upgrades the default preset to **Agent**, which allows writes inside the workspace. Codex only interrupts you when it needs to leave the workspace or rerun something outside the sandbox. Note that the workspace includes the working directory plus temporary directories like `/tmp`. Use `/status` to confirm the exact writable roots.

If you want maximum guardrails for a trusted repo, switch back to Read Only from the `/approvals` picker. If you truly need hands-off automation, use `Full Access`—but be deliberate, because that skips both the sandbox and approvals.

### Can I run without ANY approvals?

Yes, you can disable all approval prompts with `--ask-for-approval never`. This option works with all `--sandbox` modes, so you still have full control over Codex's level of autonomy. It will make its best attempt with whatever constraints you provide.

### Common sandbox + approvals combinations

| Intent                             | Flags                                                                                       | Effect                                                                                                                                                |
| ---------------------------------- | ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Safe read-only browsing            | `--sandbox read-only --ask-for-approval on-request`                                         | Codex can read files and answer questions. Codex requires approval to make edits, run commands, or access network.                                    |
| Read-only non-interactive (CI)     | `--sandbox read-only --ask-for-approval never`                                              | Reads only; never escalates                                                                                                                           |
| Let it edit the repo, ask if risky | `--sandbox workspace-write --ask-for-approval on-request`                                   | Codex can read files, make edits, and run commands in the workspace. Codex requires approval for actions outside the workspace or for network access. |
| Auto (preset; trusted repos)       | `--full-auto` (equivalent to `--sandbox workspace-write` + `--ask-for-approval on-request`) | Codex runs sandboxed commands that can write inside the workspace without prompting. Escalates only when it must leave the sandbox.                   |
| YOLO (not recommended)             | `--dangerously-bypass-approvals-and-sandbox` (alias: `--yolo`)                              | No sandbox; no prompts                                                                                                                                |

> Note: In `workspace-write`, network is disabled by default unless enabled in config (`[sandbox_workspace_write].network_access = true`).

#### Fine-tuning in `config.toml`

```toml
# approval mode
approval_policy = "untrusted"
sandbox_mode    = "read-only"

# full-auto mode
approval_policy = "on-request"
sandbox_mode    = "workspace-write"

# Optional: allow network in workspace-write mode
[sandbox_workspace_write]
network_access = true
```

You can also save presets as **profiles**:

```toml
[profiles.full_auto]
approval_policy = "on-request"
sandbox_mode    = "workspace-write"

[profiles.readonly_quiet]
approval_policy = "never"
sandbox_mode    = "read-only"
```

### Sandbox mechanics by platform

The mechanism Codex uses to enforce the sandbox policy depends on your OS:

#### macOS 12+

Uses **Apple Seatbelt**. Codex invokes `sandbox-exec` with a profile that corresponds to the selected `--sandbox` mode, constraining filesystem and network access at the OS level.

#### Linux

Combines **Landlock** and **seccomp** APIs to approximate the same guarantees. Kernel support is required; older kernels may not expose the necessary features.

In containerized Linux environments (for example Docker), sandboxing may not work when the host or container configuration does not expose Landlock/seccomp. In those cases, configure the container to provide the isolation you need and run Codex with `--sandbox danger-full-access` (or the shorthand `--dangerously-bypass-approvals-and-sandbox`) inside that container.

#### Windows

Windows sandbox support remains experimental. How it works:

- Launches commands inside a restricted token derived from an AppContainer profile.
- Grants only specifically requested filesystem capabilities by attaching capability SIDs to that profile.
- Disables outbound network access by overriding proxy-related environment variables and inserting stub executables for common network tools.

Its primary limitation is that it cannot prevent file writes, deletions, or creations in any directory where the Everyone SID already has write permissions (for example, world-writable folders).
See more discussion and limitations at [Windows Sandbox Security Details](./windows_sandbox_security.md).

## Experimenting with the Codex Sandbox

To test how commands behave under Codex's sandbox, use the CLI helpers:

```
# macOS
codex sandbox macos [--full-auto] [COMMAND]...

# Linux
codex sandbox linux [--full-auto] [COMMAND]...

# Legacy aliases
codex debug seatbelt [--full-auto] [COMMAND]...
codex debug landlock [--full-auto] [COMMAND]...
```
