## Sandbox & approvals

### Choosing Codex's level of autonomy

We always recommend running Codex in its default sandbox that gives you strong guardrails around what the agent can do. The default sandbox prevents it from editing files outside its workspace, or from accessing the network.

When you launch Codex in a new folder, it detects whether the folder is version controlled and recommends one of two levels of autonomy:

#### **1. Read/write**

- Codex can run commands and write files in the workspace without approval.
- To write files in other folders, access network, update git or perform other actions protected by the sandbox, Codex will need your permission.
- By default, the workspace includes the current directory, as well as temporary directories like `/tmp`. You can see what directories are in the workspace with the `/status` command. See the docs for how to customize this behavior.
- Advanced: You can manually specify this configuration by running `codex --sandbox workspace-write --ask-for-approval on-request`
- This is the recommended default for version-controlled folders.

#### **2. Read-only**

- Codex can run read-only commands without approval.
- To edit files, access network, or perform other actions protected by the sandbox, Codex will need your permission.
- Advanced: You can manually specify this configuration by running `codex --sandbox read-only --ask-for-approval on-request`
- This is the recommended default non-version-controlled folders.

#### **3. Advanced configuration**

Codex gives you fine-grained control over the sandbox with the `--sandbox` option, and over when it requests approval with the `--ask-for-approval` option. Run `codex help` for more on these options.

### Can I run without ANY approvals?

Yes, you can disable all approval prompts with `--ask-for-approval never`. This option works with all `--sandbox` modes, so you still have full control over Codex's level of autonomy. It will make its best attempt with whatever contrainsts you provide.

### Common sandbox + approvals combinations

| Intent                                  | Flags                                                                                  | Effect                                                                                  |
| --------------------------------------- | ----------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Safe read-only browsing                 | `--sandbox read-only --ask-for-approval on-request`                                            | Codex can read files and answer questions. Codex requires approval to make edits, run commands, or access network. |
| Read-only non-interactive (CI)          | `--sandbox read-only --ask-for-approval never`                                                 | Reads only; never escalates                                                                     |
| Let it edit the repo, ask if risky      | `--sandbox workspace-write --ask-for-approval on-request`                                      | Codex can read files, make edits, and run commands in the workspace. Codex requires approval for actions outside the workspace or for network access. |
| Auto (preset)                           | `--full-auto` (equivalent to `--sandbox workspace-write` + `--ask-for-approval on-failure`)     | Codex can read files, make edits, and run commands in the workspace. Codex requires approval when a sandboxed command fails or needs escalation. |
| YOLO (not recommended)                  | `--dangerously-bypass-approvals-and-sandbox` (alias: `--yolo`)                                 | No sandbox; no prompts                                                                          |

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

### Experimenting with the Codex Sandbox

To test to see what happens when a command is run under the sandbox provided by Codex, we provide the following subcommands in Codex CLI:

```
# macOS
codex debug seatbelt [--full-auto] [COMMAND]...

# Linux
codex debug landlock [--full-auto] [COMMAND]...
```

### Platform sandboxing details

The mechanism Codex uses to implement the sandbox policy depends on your OS:

- **macOS 12+** uses **Apple Seatbelt** and runs commands using `sandbox-exec` with a profile (`-p`) that corresponds to the `--sandbox` that was specified.
- **Linux** uses a combination of Landlock/seccomp APIs to enforce the `sandbox` configuration.

Note that when running Linux in a containerized environment such as Docker, sandboxing may not work if the host/container configuration does not support the necessary Landlock/seccomp APIs. In such cases, we recommend configuring your Docker container so that it provides the sandbox guarantees you are looking for and then running `codex` with `--sandbox danger-full-access` (or, more simply, the `--dangerously-bypass-approvals-and-sandbox` flag) within your container. 