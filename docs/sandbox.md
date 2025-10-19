## Sandbox & approvals

What Codex is allowed to do is governed by a combination of **sandbox modes** (what Codex is allowed to do without supervision) and **approval policies** (when you must confirm an action). This page explains the options, how they interact, and how the sandbox behaves on each platform.

### Approval policies

We've chosen a powerful default for how Codex works on your computer: `Auto`. Under this approval policy, Codex can read files, make edits, and run commands in the working directory automatically. However, Codex will need your approval to work outside the working directory or access network.

When you just want to chat, or if you want to plan before diving in, you can switch to `Read Only` mode with the `/approvals` command.

If you need Codex to read files, make edits, and run commands with network access, without approval, you can use `Full Access`. Exercise caution before doing so.

#### Defaults and recommendations

- Codex runs in a sandbox by default with strong guardrails: it prevents editing files outside the workspace and blocks network access unless enabled.
- On launch, Codex detects whether the folder is version-controlled and recommends:
  - Version-controlled folders: `Auto` (workspace write + on-request approvals)
  - Non-version-controlled folders: `Read Only`
- The workspace includes the current directory and temporary directories like `/tmp`. Use the `/status` command to see which directories are in the workspace.
- You can set these explicitly:
  - `codex --sandbox workspace-write --ask-for-approval on-request`
  - `codex --sandbox read-only --ask-for-approval on-request`

### Can I run without ANY approvals?

Yes, you can disable all approval prompts with `--ask-for-approval never`. This option works with all `--sandbox` modes, so you still have full control over Codex's level of autonomy. It will make its best attempt with whatever constraints you provide.

### Common sandbox + approvals combinations

| Intent                             | Flags                                                                                       | Effect                                                                                                                                                |
| ---------------------------------- | ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Safe read-only browsing            | `--sandbox read-only --ask-for-approval on-request`                                         | Codex can read files and answer questions. Codex requires approval to make edits, run commands, or access network.                                    |
| Read-only non-interactive (CI)     | `--sandbox read-only --ask-for-approval never`                                              | Reads only; never escalates                                                                                                                           |
| Let it edit the repo, ask if risky | `--sandbox workspace-write --ask-for-approval on-request`                                   | Codex can read files, make edits, and run commands in the workspace. Codex requires approval for actions outside the workspace or for network access. |
| Auto (preset)                      | `--full-auto` (equivalent to `--sandbox workspace-write` + `--ask-for-approval on-failure`) | Codex can read files, make edits, and run commands in the workspace. Codex requires approval when a sandboxed command fails or needs escalation.      |
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

### Sandbox mechanics by platform {#platform-sandboxing-details}

The mechanism Codex uses to enforce the sandbox policy depends on your OS:

- **macOS 12+** uses **Apple Seatbelt**. Codex invokes `sandbox-exec` with a profile that corresponds to the selected `--sandbox` mode, constraining filesystem and network access at the OS level.
- **Linux** combines **Landlock** and **seccomp** APIs to approximate the same guarantees. Kernel support is required; older kernels may not expose the necessary features.

In containerized Linux environments (for example Docker), sandboxing may not work when the host or container configuration does not expose Landlock/seccomp. In those cases, configure the container to provide the isolation you need and run Codex with `--sandbox danger-full-access` (or the shorthand `--dangerously-bypass-approvals-and-sandbox`) inside that container.

### Experimenting with the Codex Sandbox

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
