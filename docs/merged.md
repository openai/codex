# Merged Fragmented Markdown

## Source: /Users/kooshapari/temp-PRODVERCEL/485/kush/heliosHarness/clones/codex/docs
## Source: CLA.md

# Individual Contributor License Agreement (v1.0, OpenAI)

_Based on the Apache Software Foundation Individual CLA v 2.2._

By commenting **“I have read the CLA Document and I hereby sign the CLA”**
on a Pull Request, **you (“Contributor”) agree to the following terms** for any
past and future “Contributions” submitted to the **OpenAI Codex CLI project
(the “Project”)**.

---

## 1. Definitions

- **“Contribution”** – any original work of authorship submitted to the Project
  (code, documentation, designs, etc.).
- **“You” / “Your”** – the individual (or legal entity) posting the acceptance
  comment.

## 2. Copyright License

You grant **OpenAI, Inc.** and all recipients of software distributed by the
Project a perpetual, worldwide, non‑exclusive, royalty‑free, irrevocable
license to reproduce, prepare derivative works of, publicly display, publicly
perform, sublicense, and distribute Your Contributions and derivative works.

## 3. Patent License

You grant **OpenAI, Inc.** and all recipients of the Project a perpetual,
worldwide, non‑exclusive, royalty‑free, irrevocable (except as below) patent
license to make, have made, use, sell, offer to sell, import, and otherwise
transfer Your Contributions alone or in combination with the Project.

If any entity brings patent litigation alleging that the Project or a
Contribution infringes a patent, the patent licenses granted by You to that
entity under this CLA terminate.

## 4. Representations

1. You are legally entitled to grant the licenses above.
2. Each Contribution is either Your original creation or You have authority to
   submit it under this CLA.
3. Your Contributions are provided **“AS IS”** without warranties of any kind.
4. You will notify the Project if any statement above becomes inaccurate.

## 5. Miscellany

This Agreement is governed by the laws of the **State of California**, USA,
excluding its conflict‑of‑laws rules. If any provision is held unenforceable,
the remaining provisions remain in force.

---

## Source: agents_md.md

# AGENTS.md

For information about AGENTS.md, see [this documentation](https://developers.openai.com/codex/guides/agents-md).

## Hierarchical agents message

When the `child_agents_md` feature flag is enabled (via `[features]` in `config.toml`), Codex appends additional guidance about AGENTS.md scope and precedence to the user instructions message and emits that message even when no AGENTS.md is present.

---

## Source: authentication.md

# Authentication

For information about Codex CLI authentication, see [this documentation](https://developers.openai.com/codex/auth).

---

## Source: config.md

# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## Connecting to MCP servers

Codex can connect to MCP servers configured in `~/.codex/config.toml`. See the configuration reference for the latest MCP server options:

- https://developers.openai.com/codex/config-reference

## Apps (Connectors)

Use `$` in the composer to insert a ChatGPT connector; the popover lists accessible
apps. The `/apps` command lists available and installed apps. Connected apps appear first
and are labeled as connected; others are marked as can be installed.

## Notify

Codex can run a notification hook when the agent finishes a turn. See the configuration reference for the latest notification settings:

- https://developers.openai.com/codex/config-reference

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

## Plan mode defaults

`plan_mode_reasoning_effort` lets you set a Plan-mode-specific default reasoning
effort override. When unset, Plan mode uses the built-in Plan preset default
(currently `medium`). When explicitly set (including `none`), it overrides the
Plan preset. The string value `none` means "no reasoning" (an explicit Plan
override), not "inherit the global default". There is currently no separate
config value for "follow the global default in Plan mode".

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).

---

## Source: contributing.md

## Contributing

**External contributions are by invitation only**

At this time, the Codex team does not accept unsolicited code contributions.

If you would like to propose a new feature or a change in behavior, please open an issue describing the proposal or upvote an existing enhancement request. We prioritize new features based on community feedback, alignment with our roadmap, and consistency across all Codex surfaces (CLI, IDE extensions, web, etc.).

If you encounter a bug, please open a bug report or verify that an existing report already covers the issue. If you would like to help, we encourage you to contribute by sharing analysis, reproduction details, root-cause hypotheses, or a high-level outline of a potential fix directly in the issue thread.

The Codex team may invite an external contributor to submit a pull request when:

- the problem is well understood,
- the proposed approach aligns with the team’s intended solution, and
- the issue is deemed high-impact and high-priority.

Pull requests that have not been explicitly invited by a member of the Codex team will be closed without review.

**Why we do not generally accept external code contributions**

In the past, the Codex team accepted external pull requests for bug fixes. While we appreciated the effort and engagement from the community, this model did not scale well.

Many contributions were made without full visibility into the architectural context, system-level constraints, or near-term roadmap considerations that guide Codex development. Others focused on issues that were low priority or affected a very small subset of users. Reviewing and iterating on these PRs often took more time than implementing the fix directly, and diverted attention from higher-priority work.

The most valuable contributions consistently came from community members who demonstrated deep understanding of a problem domain. That expertise is most helpful when shared early -- through detailed bug reports, analysis, and design discussion in issues. Identifying the right solution is typically the hard part; implementing it is comparatively straightforward with the help of Codex itself.

For these reasons, we focus external contributions on discussion, analysis, and feedback, and reserve code changes for cases where a targeted invitation makes sense.

### Development workflow

If you are invited by a Codex team member to contribute a PR, here is the recommended development workflow.

- Create a _topic branch_ from `main` - e.g. `feat/interactive-prompt`.
- Keep your changes focused. Multiple unrelated fixes should be opened as separate PRs.
- Ensure your change is free of lint warnings and test failures.

### Guidance for invited code contributions

1. **Start with an issue.** Open a new one or comment on an existing discussion so we can agree on the solution before code is written.
2. **Add or update tests.** A bug fix should generally come with test coverage that fails before your change and passes afterwards. 100% coverage is not required, but aim for meaningful assertions.
3. **Document behavior.** If your change affects user-facing behavior, update the README, inline help (`codex --help`), or relevant example projects.
4. **Keep commits atomic.** Each commit should compile and the tests should pass. This makes reviews and potential rollbacks easier.

### Model metadata updates

When a change updates model catalogs or model metadata (`/models` payloads, presets, or fixtures):

- Set `input_modalities` explicitly for any model that does not support images.
- Keep compatibility defaults in mind: omitted `input_modalities` currently implies text + image support.
- Ensure client surfaces that accept images (for example, TUI paste/attach) consume the same capability signal.
- Add/update tests that cover unsupported-image behavior and warning paths.

### Opening a pull request (by invitation only)

- Fill in the PR template (or include similar information) - **What? Why? How?**
- Include a link to a bug report or enhancement request in the issue tracker
- Run **all** checks locally. Use the root `just` helpers so you stay consistent with the rest of the workspace: `just fmt`, `just fix -p <crate>` for the crate you touched, and the relevant tests (e.g., `cargo test -p codex-tui` or `just test` if you need a full sweep). CI failures that could have been caught locally slow down the process.
- Make sure your branch is up-to-date with `main` and that you have resolved merge conflicts.
- Mark the PR as **Ready for review** only when you believe it is in a merge-able state.

### Review process

1. One maintainer will be assigned as a primary reviewer.
2. If your invited PR introduces scope or behavior that was not previously discussed and approved, we may close the PR.
3. We may ask for changes. Please do not take this personally. We value the work, but we also value consistency and long-term maintainability.
4. When there is consensus that the PR meets the bar, a maintainer will squash-and-merge.

### Community values

- **Be kind and inclusive.** Treat others with respect; we follow the [Contributor Covenant](https://www.contributor-covenant.org/).
- **Assume good intent.** Written communication is hard - err on the side of generosity.
- **Teach & learn.** If you spot something confusing, open an issue or discussion with suggestions or clarifications.

### Getting help

If you run into problems setting up the project, would like feedback on an idea, or just want to say _hi_ - please open a Discussion topic or jump into the relevant issue. We are happy to help.

Together we can make Codex CLI an incredible tool. **Happy hacking!** :rocket:

### Contributor license agreement (CLA)

All contributors **must** accept the CLA. The process is lightweight:

1. Open your pull request.
2. Paste the following comment (or reply `recheck` if you've signed before):

   ```text
   I have read the CLA Document and I hereby sign the CLA
   ```

3. The CLA-Assistant bot records your signature in the repo and marks the status check as passed.

No special Git commands, email attachments, or commit footers required.

### Security & responsible AI

Have you discovered a vulnerability or have concerns about model output? Please e-mail **security@openai.com** and we will respond promptly.

---

## Source: example-config.md

# Sample configuration

For a sample configuration file, see [this documentation](https://developers.openai.com/codex/config-sample).

---

## Source: exec.md

# Non-interactive mode

For information about non-interactive mode, see [this documentation](https://developers.openai.com/codex/noninteractive).

---

## Source: execpolicy.md

# Execution policy

For an overview of execution policy rules, see [this documentation](https://developers.openai.com/codex/exec-policy).

---

## Source: exit-confirmation-prompt-design.md

# Exit and shutdown flow (tui)

This document describes how exit, shutdown, and interruption work in the Rust TUI (`codex-rs/tui`).
It is intended for Codex developers and Codex itself when reasoning about future exit/shutdown
changes.

This doc replaces earlier separate history and design notes. High-level history is summarized
below; full details are captured in PR #8936.

## Terms

- **Exit**: end the UI event loop and terminate the process.
- **Shutdown**: request a graceful agent/core shutdown (`Op::Shutdown`) and wait for
  `ShutdownComplete` so cleanup can run.
- **Interrupt**: cancel a running operation (`Op::Interrupt`).

## Event model (AppEvent)

Exit is coordinated via a single event with explicit modes:

- `AppEvent::Exit(ExitMode::ShutdownFirst)`
  - Prefer this for user-initiated quits so cleanup runs.
- `AppEvent::Exit(ExitMode::Immediate)`
  - Escape hatch for immediate exit. This bypasses shutdown and can drop
    in-flight work (e.g., tasks, rollout flush, child process cleanup).

`App` is the coordinator: it submits `Op::Shutdown` and it exits the UI loop only when
`ExitMode::Immediate` arrives (typically after `ShutdownComplete`).

## User-triggered quit flows

### Ctrl+C

Priority order in the UI layer:

1. Active modal/view gets the first chance to consume (`BottomPane::on_ctrl_c`).
   - If the modal handles it, the quit flow stops.
   - When a modal/popup handles Ctrl+C, the quit shortcut is cleared so dismissing a modal cannot
     accidentally prime a subsequent Ctrl+C to quit.
2. If the user has already armed Ctrl+C and the 1 second window has not expired, the second Ctrl+C
   triggers shutdown-first quit immediately.
3. Otherwise, `ChatWidget` arms Ctrl+C and shows the quit hint (`ctrl + c again to quit`) for
   1 second.
4. If cancellable work is active (streaming/tools/review), `ChatWidget` submits `Op::Interrupt`.

### Ctrl+D

- Only participates in quit when the composer is empty **and** no modal is active.
  - On first press, show the quit hint (same as Ctrl+C) and start the 1 second timer.
  - If pressed again while the hint is visible, request shutdown-first quit.
- With any modal/popup open, key events are routed to the view and Ctrl+D does not attempt to
  quit.

### Slash commands

- `/quit`, `/exit`, `/logout` request shutdown-first quit **without** a prompt,
  because slash commands are harder to trigger accidentally and imply clear intent to quit.

### /new

- Uses shutdown without exit (suppresses `ShutdownComplete`) so the app can
  start a fresh session without terminating.

## Shutdown completion and suppression

`ShutdownComplete` is the signal that core cleanup has finished. The UI treats it as the boundary
for exit:

- `ChatWidget` requests `Exit(Immediate)` on `ShutdownComplete`.
- `App` can suppress a single `ShutdownComplete` when shutdown is used as a
  cleanup step (e.g., `/new`).

## Edge cases and invariants

- **Review mode** counts as cancellable work. Ctrl+C should interrupt review, not
  quit.
- **Modal open** means Ctrl+C/Ctrl+D should not quit unless the modal explicitly
  declines to handle Ctrl+C.
- **Immediate exit** is not a normal user path; it is a fallback for shutdown
  completion or an emergency exit. Use it sparingly because it skips cleanup.

## Testing expectations

At a minimum, we want coverage for:

- Ctrl+C while working interrupts, does not quit.
- Ctrl+C while idle and empty shows quit hint, then shutdown-first quit on second press.
- Ctrl+D with modal open does not quit.
- `/quit` / `/exit` / `/logout` quit without prompt, but still shutdown-first.
  - Ctrl+D while idle and empty shows quit hint, then shutdown-first quit on second press.

## History (high level)

Codex has historically mixed "exit immediately" and "shutdown-first" across quit gestures, largely
due to incremental changes and regressions in state tracking. This doc reflects the current
unified, shutdown-first approach. See PR #8936 for the detailed history and rationale.

---

## Source: getting-started.md

# Getting started with Codex CLI

For an overview of Codex CLI features, see [this documentation](https://developers.openai.com/codex/cli/features#running-in-interactive-mode).

---

## Source: install.md

## Installing & building

### System requirements

| Requirement                 | Details                                                         |
| --------------------------- | --------------------------------------------------------------- |
| Operating systems           | macOS 12+, Ubuntu 20.04+/Debian 10+, or Windows 11 **via WSL2** |
| Git (optional, recommended) | 2.23+ for built-in PR helpers                                   |
| RAM                         | 4-GB minimum (8-GB recommended)                                 |

### DotSlash

The GitHub Release also contains a [DotSlash](https://dotslash-cli.com/) file for the Codex CLI named `codex`. Using a DotSlash file makes it possible to make a lightweight commit to source control to ensure all contributors use the same version of an executable, regardless of what platform they use for development.

### Build from source

```bash
# Clone the repository and navigate to the root of the Cargo workspace.
git clone https://github.com/openai/codex.git
cd codex/codex-rs

# Install the Rust toolchain, if necessary.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup component add rustfmt
rustup component add clippy
# Install helper tools used by the workspace justfile:
cargo install just
# Optional: install nextest for the `just test` helper
cargo install --locked cargo-nextest

# Build Codex.
cargo build

# Launch the TUI with a sample prompt.
cargo run --bin codex -- "explain this codebase to me"

# After making changes, use the root justfile helpers (they default to codex-rs):
just fmt
just fix -p <crate-you-touched>

# Run the relevant tests (project-specific is fastest), for example:
cargo test -p codex-tui
# If you have cargo-nextest installed, `just test` runs the test suite via nextest:
just test
# Avoid `--all-features` for routine local runs because it increases build
# time and `target/` disk usage by compiling additional feature combinations.
# If you specifically want full feature coverage, use:
cargo test --all-features
```

## Tracing / verbose logging

Codex is written in Rust, so it honors the `RUST_LOG` environment variable to configure its logging behavior.

The TUI defaults to `RUST_LOG=codex_core=info,codex_tui=info,codex_rmcp_client=info` and log messages are written to `~/.codex/log/codex-tui.log` by default. For a single run, you can override the log directory with `-c log_dir=...` (for example, `-c log_dir=./.codex-log`).

```bash
tail -F ~/.codex/log/codex-tui.log
```

By comparison, the non-interactive mode (`codex exec`) defaults to `RUST_LOG=error`, but messages are printed inline, so there is no need to monitor a separate file.

See the Rust documentation on [`RUST_LOG`](https://docs.rs/env_logger/latest/env_logger/#enabling-logging) for more information on the configuration options.

---

## Source: js_repl.md

# JavaScript REPL (`js_repl`)

`js_repl` runs JavaScript in a persistent Node-backed kernel with top-level `await`.

## Feature gate

`js_repl` is disabled by default and only appears when:

```toml
[features]
js_repl = true
```

`js_repl_tools_only` can be enabled to force direct model tool calls through `js_repl`:

```toml
[features]
js_repl = true
js_repl_tools_only = true
```

When enabled, direct model tool calls are restricted to `js_repl` and `js_repl_reset`; other tools remain available via `await codex.tool(...)` inside js_repl.

## Node runtime

`js_repl` requires a Node version that meets or exceeds `codex-rs/node-version.txt`.

Runtime resolution order:

1. `CODEX_JS_REPL_NODE_PATH` environment variable
2. `js_repl_node_path` in config/profile
3. `node` discovered on `PATH`

You can configure an explicit runtime path:

```toml
js_repl_node_path = "/absolute/path/to/node"
```

## Module resolution

`js_repl` resolves **bare** specifiers (for example `await import("pkg")`) using an ordered
search path. Path-style specifiers (`./`, `../`, absolute paths, `file:` URLs) are rejected.

Module resolution proceeds in the following order:

1. `CODEX_JS_REPL_NODE_MODULE_DIRS` (PATH-delimited list)
2. `js_repl_node_module_dirs` in config/profile (array of absolute paths)
3. Thread working directory (cwd, always included as the last fallback)

For `CODEX_JS_REPL_NODE_MODULE_DIRS` and `js_repl_node_module_dirs`, module resolution is attempted in the order provided with earlier entries taking precedence.

## Usage

- `js_repl` is a freeform tool: send raw JavaScript source text.
- Optional first-line pragma:
  - `// codex-js-repl: timeout_ms=15000`
- Top-level bindings persist across calls.
- Top-level static import declarations (for example `import x from "pkg"`) are currently unsupported; use dynamic imports with `await import("pkg")`.
- Use `js_repl_reset` to clear the kernel state.

## Helper APIs inside the kernel

`js_repl` exposes these globals:

- `codex.tmpDir`: per-session scratch directory path.
- `codex.tool(name, args?)`: executes a normal Codex tool call from inside `js_repl` (including shell tools like `shell` / `shell_command` when available).
- To share generated images with the model, write a file under `codex.tmpDir`, call `await codex.tool("view_image", { path: "/absolute/path" })`, then delete the file.

Avoid writing directly to `process.stdout` / `process.stderr` / `process.stdin`; the kernel uses a JSON-line transport over stdio.

## Vendored parser asset (`meriyah.umd.min.js`)

The kernel embeds a vendored Meriyah bundle at:

- `codex-rs/core/src/tools/js_repl/meriyah.umd.min.js`

Current source is `meriyah@7.0.0` from npm (`dist/meriyah.umd.min.js`).
Licensing is tracked in:

- `third_party/meriyah/LICENSE`
- `NOTICE`

### How this file was sourced

From a clean temp directory:

```sh
tmp="$(mktemp -d)"
cd "$tmp"
npm pack meriyah@7.0.0
tar -xzf meriyah-7.0.0.tgz
cp package/dist/meriyah.umd.min.js /path/to/repo/codex-rs/core/src/tools/js_repl/meriyah.umd.min.js
cp package/LICENSE.md /path/to/repo/third_party/meriyah/LICENSE
```

### How to update to a newer version

1. Replace `7.0.0` in the commands above with the target version.
2. Copy the new `dist/meriyah.umd.min.js` into `codex-rs/core/src/tools/js_repl/meriyah.umd.min.js`.
3. Copy the package license into `third_party/meriyah/LICENSE`.
4. Update the version string in the header comment at the top of `meriyah.umd.min.js`.
5. Update `NOTICE` if the upstream copyright notice changed.
6. Run the relevant `js_repl` tests.

---

## Source: license.md

## License

This repository is licensed under the [Apache-2.0 License](../LICENSE).

---

## Source: open-source-fund.md

## Codex open source fund

We're excited to launch a **$1 million initiative** supporting open source projects that use Codex CLI and other OpenAI models.

- Grants are awarded up to **$25,000** API credits.
- Applications are reviewed **on a rolling basis**.

**Interested? [Apply here](https://openai.com/form/codex-open-source-fund/).**

---

## Source: prompts.md

# Custom prompts

For an overview of custom prompts, see [this documentation](https://developers.openai.com/codex/custom-prompts).

---

## Source: sandbox.md

## Sandbox & approvals

For information about Codex sandboxing and approvals, see [this documentation](https://developers.openai.com/codex/security).

---

## Source: skills.md

# Skills

For information about skills, refer to [this documentation](https://developers.openai.com/codex/skills).

---

## Source: slash_commands.md

# Slash commands

For an overview of Codex CLI slash commands, see [this documentation](https://developers.openai.com/codex/cli/slash-commands).

---

## Source: tui-alternate-screen.md

# TUI Alternate Screen and Terminal Multiplexers

## Overview

This document explains the design decision behind Codex's alternate screen handling, particularly in terminal multiplexers like Zellij. This addresses a fundamental conflict between fullscreen TUI behavior and terminal scrollback history preservation.

## The Problem

### Fullscreen TUI Benefits

Codex's TUI uses the terminal's **alternate screen buffer** to provide a clean fullscreen experience. This approach:

- Uses the entire viewport without polluting the terminal's scrollback history
- Provides a dedicated environment for the chat interface
- Mirrors the behavior of other terminal applications (vim, tmux, etc.)

### The Zellij Conflict

Terminal multiplexers like **Zellij** strictly follow the xterm specification, which defines that alternate screen buffers should **not** have scrollback. This is intentional design, not a bug:

- **Zellij PR:** https://github.com/zellij-org/zellij/pull/1032
- **Rationale:** The xterm spec explicitly states that alternate screen mode disallows scrollback
- **Configurability:** This is not configurable in Zellij—there is no option to enable scrollback in alternate screen mode

When using Codex's TUI in Zellij, users cannot scroll back through the conversation history because:

1. The TUI runs in alternate screen mode (fullscreen)
2. Zellij disables scrollback in alternate screen buffers (per xterm spec)
3. The entire conversation becomes inaccessible via normal terminal scrolling

## The Solution

Codex implements a **pragmatic workaround** with three modes, controlled by `tui.alternate_screen` in `config.toml`:

### 1. `auto` (default)

- **Behavior:** Automatically detect the terminal multiplexer
- **In Zellij:** Disable alternate screen mode (inline mode, preserves scrollback)
- **Elsewhere:** Enable alternate screen mode (fullscreen experience)
- **Rationale:** Provides the best UX in each environment

### 2. `always`

- **Behavior:** Always use alternate screen mode (original behavior)
- **Use case:** Users who prefer fullscreen and don't use Zellij, or who have found a workaround

### 3. `never`

- **Behavior:** Never use alternate screen mode (inline mode)
- **Use case:** Users who always want scrollback history preserved
- **Trade-off:** Pollutes the terminal scrollback with TUI output

## Runtime Override

The `--no-alt-screen` CLI flag can override the config setting at runtime:

```bash
codex --no-alt-screen
```

This runs the TUI in inline mode regardless of the configuration, useful for:

- One-off sessions where scrollback is critical
- Debugging terminal-related issues
- Testing alternate screen behavior

## Implementation Details

### Auto-Detection

The `auto` mode detects Zellij by checking the `ZELLIJ` environment variable:

```rust
let terminal_info = codex_core::terminal::terminal_info();
!matches!(terminal_info.multiplexer, Some(Multiplexer::Zellij { .. }))
```

This detection happens in the helper function `determine_alt_screen_mode()` in `codex-rs/tui/src/lib.rs`.

### Configuration Schema

The `AltScreenMode` enum is defined in `codex-rs/protocol/src/config_types.rs` and serializes to lowercase TOML:

```toml
[tui]
# Options: auto, always, never
alternate_screen = "auto"
```

### Why Not Just Disable Alternate Screen in Zellij Permanently?

We use `auto` detection instead of always disabling in Zellij because:

1. Many Zellij users don't care about scrollback and prefer the fullscreen experience
2. Some users may use tmux inside Zellij, creating a chain of multiplexers
3. Provides user choice without requiring manual configuration

## Related Issues and References

- **Original Issue:** [GitHub #2558](https://github.com/openai/codex/issues/2558) - "No scrollback in Zellij"
- **Implementation PR:** [GitHub #8555](https://github.com/openai/codex/pull/8555)
- **Zellij PR:** https://github.com/zellij-org/zellij/pull/1032 (why scrollback is disabled)
- **xterm Spec:** Alternate screen buffers should not have scrollback

## Future Considerations

### Alternative Approaches Considered

1. **Implement custom scrollback in TUI:** Would require significant architectural changes to buffer and render all historical output
2. **Request Zellij to add a config option:** Not viable—Zellij maintainers explicitly chose this behavior to follow the spec
3. **Disable alternate screen unconditionally:** Would degrade UX for non-Zellij users

### Transcript Pager

Codex's transcript pager (opened with Ctrl+T) provides an alternative way to review conversation history, even in fullscreen mode. However, this is not as seamless as natural scrollback.

## For Developers

When modifying TUI code, remember:

- The `determine_alt_screen_mode()` function encapsulates all the logic
- Configuration is in `config.tui_alternate_screen`
- CLI flag is in `cli.no_alt_screen`
- The behavior is applied via `tui.set_alt_screen_enabled()`

If you encounter issues with terminal state after running Codex, you can restore your terminal with:

```bash
reset
```

---

## Source: tui-chat-composer.md

# Chat Composer state machine (TUI)

This note documents the `ChatComposer` input state machine and the paste-related behavior added
for Windows terminals.

Primary implementations:

- `codex-rs/tui/src/bottom_pane/chat_composer.rs`

Paste-burst detector:

- `codex-rs/tui/src/bottom_pane/paste_burst.rs`

## What problem is being solved?

On some terminals (notably on Windows via `crossterm`), _bracketed paste_ is not reliably surfaced
as a single paste event. Instead, pasting multi-line content can show up as a rapid sequence of
key events:

- `KeyCode::Char(..)` for text
- `KeyCode::Enter` for newlines

If the composer treats those events as “normal typing”, it can:

- accidentally trigger UI toggles (e.g. `?`) while the paste is still streaming,
- submit the message mid-paste when an `Enter` arrives,
- render a typed prefix, then “reclassify” it as paste once enough chars arrive (flicker).

The solution is to detect paste-like _bursts_ and buffer them into a single explicit
`handle_paste(String)` call.

## High-level state machines

`ChatComposer` effectively combines two small state machines:

1. **UI mode**: which popup (if any) is active.
   - `ActivePopup::None | Command | File | Skill`
2. **Paste burst**: transient detection state for non-bracketed paste.
   - implemented by `PasteBurst`

### Key event routing

`ChatComposer::handle_key_event` dispatches based on `active_popup`:

- If a popup is visible, a popup-specific handler processes the key first (navigation, selection,
  completion).
- Otherwise, `handle_key_event_without_popup` handles higher-level semantics (Enter submit,
  history navigation, etc).
- After handling the key, `sync_popups()` runs so popup visibility/filters stay consistent with the
  latest text + cursor.
- When a slash command name is completed and the user types a space, the `/command` token is
  promoted into a text element so it renders distinctly and edits atomically.

### History navigation (↑/↓)

Up/Down recall is handled by `ChatComposerHistory` and merges two sources:

- **Persistent history** (cross-session, fetched from `~/.codex/history.jsonl`): text-only. It
  does **not** carry text element ranges or image attachments, so recalling one of these entries
  only restores the text.
- **Local history** (current session): stores the full submission payload, including text
  elements, local image paths, and remote image URLs. Recalling a local entry rehydrates
  placeholders and attachments.

This distinction keeps the on-disk history backward compatible and avoids persisting attachments,
while still providing a richer recall experience for in-session edits.

## Config gating for reuse

`ChatComposer` now supports feature gating via `ChatComposerConfig`
(`codex-rs/tui/src/bottom_pane/chat_composer.rs`). The default config preserves current chat
behavior.

Flags:

- `popups_enabled`
- `slash_commands_enabled`
- `image_paste_enabled`

Key effects when disabled:

- When `popups_enabled` is `false`, `sync_popups()` forces `ActivePopup::None`.
- When `slash_commands_enabled` is `false`, the composer does not treat `/...` input as commands.
- When `slash_commands_enabled` is `false`, the composer does not expand custom prompts in
  `prepare_submission_text`.
- When `slash_commands_enabled` is `false`, slash-context paste-burst exceptions are disabled.
- When `image_paste_enabled` is `false`, file-path paste image attachment is skipped.
- `ChatWidget` may toggle `image_paste_enabled` at runtime based on the selected model's
  `input_modalities`; attach and submit paths also re-check support and emit a warning instead of
  dropping the draft.

Built-in slash command availability is centralized in
`codex-rs/tui/src/bottom_pane/slash_commands.rs` and reused by both the composer and the command
popup so gating stays in sync.

## Submission flow (Enter/Tab)

There are multiple submission paths, but they share the same core rules:

When steer mode is enabled, `Tab` requests queuing if a task is already running; otherwise it
submits immediately. `Enter` always submits immediately in this mode. `Tab` does not submit when
the input starts with `!` (shell command).

### Normal submit/queue path

`handle_submission` calls `prepare_submission_text` for both submit and queue. That method:

1. Expands any pending paste placeholders so element ranges align with the final text.
2. Trims whitespace and rebases element ranges to the trimmed buffer.
3. Expands `/prompts:` custom prompts:
   - Named args use key=value parsing.
   - Numeric args use positional parsing for `$1..$9` and `$ARGUMENTS`.
     The expansion preserves text elements and yields the final submission payload.
4. Prunes attachments so only placeholders that survive expansion are sent.
5. Clears pending pastes on success and suppresses submission if the final text is empty and there
   are no attachments.

The same preparation path is reused for slash commands with arguments (for example `/plan` and
`/review`) so pasted content and text elements are preserved when extracting args.

### Numeric auto-submit path

When the slash popup is open and the first line matches a numeric-only custom prompt with
positional args, Enter auto-submits without calling `prepare_submission_text`. That path still:

- Expands pending pastes before parsing positional args.
- Uses expanded text elements for prompt expansion.
- Prunes attachments based on expanded placeholders.
- Clears pending pastes after a successful auto-submit.

## Remote image rows (selection/deletion flow)

Remote image URLs are shown as `[Image #N]` rows above the textarea, inside the same composer box.
They are attachment rows, not editable textarea content.

- TUI can remove these rows, but cannot type before/between them.
- Press `Up` at textarea cursor position `0` to select the last remote image row.
- While selected, `Up`/`Down` moves selection across remote image rows.
- Pressing `Down` on the last row exits remote-row selection and returns to textarea editing.
- `Delete` or `Backspace` removes the selected remote image row.

Image numbering is unified:

- Remote image rows always occupy `[Image #1]..[Image #M]`.
- Local attached image placeholders start after that offset (`[Image #M+1]..`).
- Removing remote rows relabels local placeholders so numbering stays contiguous.

## History navigation (Up/Down) and backtrack prefill

`ChatComposerHistory` merges two kinds of history:

- **Persistent history** (cross-session, fetched from core on demand): text-only.
- **Local history** (this UI session): full draft state.

Local history entries capture:

- raw text (including placeholders),
- `TextElement` ranges for placeholders,
- local image paths,
- remote image URLs,
- pending large-paste payloads (for drafts).

Persistent history entries only restore text. They intentionally do **not** rehydrate attachments
or pending paste payloads.

For non-empty drafts, Up/Down navigation is only treated as history recall when the current text
matches the last recalled history entry and the cursor is at a boundary (start or end of the
line). This keeps multiline cursor movement intact while preserving shell-like history traversal.

### Draft recovery (Ctrl+C)

Ctrl+C clears the composer but stashes the full draft state (text elements, local image paths,
remote image URLs, and pending paste payloads) into local history. Pressing Up immediately restores
that draft, including image placeholders and large-paste placeholders with their payloads.

### Submitted message recall

After a successful submission, the local history entry stores the submitted text, element ranges,
local image paths, and remote image URLs. Pending paste payloads are cleared during submission, so
large-paste placeholders are expanded into their full text before being recorded. This means:

- Up/Down recall of a submitted message restores remote image rows plus local image placeholders.
- Recalled entries place the cursor at end-of-line to match typical shell history editing.
- Large-paste placeholders are not expected in recalled submitted history; the text is the
  expanded paste content.

### Backtrack prefill

Backtrack selections read `UserHistoryCell` data from the transcript. The composer prefill now
reuses the selected message’s text elements, local image paths, and remote image URLs, so image
placeholders and attachments rehydrate when rolling back to a prior user message.

### External editor edits

When the composer content is replaced from an external editor, the composer rebuilds text elements
and keeps only attachments whose placeholders still appear in the new text. Image placeholders are
then normalized to `[Image #M]..[Image #N]`, where `M` starts after the number of remote image
rows, to keep attachment mapping consistent after edits.

## Paste burst: concepts and assumptions

The burst detector is intentionally conservative: it only processes “plain” character input
(no Ctrl/Alt modifiers). Everything else flushes and/or clears the burst window so shortcuts keep
their normal meaning.

### Conceptual `PasteBurst` states

- **Idle**: no buffer, no pending char.
- **Pending first char** (ASCII only): hold one fast character very briefly to avoid rendering it
  and then immediately removing it if the stream turns out to be a paste.
- **Active buffer**: once a burst is classified as paste-like, accumulate the content into a
  `String` buffer.
- **Enter suppression window**: keep treating `Enter` as “newline” briefly after burst activity so
  multiline pastes remain grouped even if there are tiny gaps.

### ASCII vs non-ASCII (IME) input

Non-ASCII characters frequently come from IMEs and can legitimately arrive in quick bursts. Holding
the first character in that case can feel like dropped input.

The composer therefore distinguishes:

- **ASCII path**: allow holding the first fast char (`PasteBurst::on_plain_char`).
- **non-ASCII path**: never hold the first char (`PasteBurst::on_plain_char_no_hold`), but still
  allow burst detection. When a burst is detected on this path, the already-inserted prefix may be
  retroactively removed from the textarea and moved into the paste buffer.

To avoid misclassifying IME bursts as paste, the non-ASCII retro-capture path runs an additional
heuristic (`PasteBurst::decide_begin_buffer`) to determine whether the retro-grabbed prefix “looks
pastey” (e.g. contains whitespace or is long).

### Disabling burst detection

`ChatComposer` supports `disable_paste_burst` as an escape hatch.

When enabled:

- The burst detector is bypassed for new input (no flicker suppression hold and no burst buffering
  decisions for incoming characters).
- The key stream is treated as normal typing (including normal slash command behavior).
- Enabling the flag flushes any held/buffered burst text through the normal paste path
  (`ChatComposer::handle_paste`) and then clears the burst timing and Enter-suppression windows so
  transient burst state cannot leak into subsequent input.

### Enter handling

When paste-burst buffering is active, Enter is treated as “append `\n` to the burst” rather than
“submit the message”. This prevents mid-paste submission for multiline pastes that are emitted as
`Enter` key events.

The composer also disables burst-based Enter suppression inside slash-command context (popup open
or the first line begins with `/`) so command dispatch is predictable.

## PasteBurst: event-level behavior (cheat sheet)

This section spells out how `ChatComposer` interprets the `PasteBurst` decisions. It’s intended to
make the state transitions reviewable without having to “run the code in your head”.

### Plain ASCII `KeyCode::Char(c)` (no Ctrl/Alt modifiers)

`ChatComposer::handle_input_basic` calls `PasteBurst::on_plain_char(c, now)` and switches on the
returned `CharDecision`:

- `RetainFirstChar`: do **not** insert `c` into the textarea yet. A UI tick later may flush it as a
  normal typed char via `PasteBurst::flush_if_due`.
- `BeginBufferFromPending`: the first ASCII char is already held/buffered; append `c` via
  `PasteBurst::append_char_to_buffer`.
- `BeginBuffer { retro_chars }`: attempt a retro-capture of the already-inserted prefix:
  - call `PasteBurst::decide_begin_buffer(now, before_cursor, retro_chars)`;
  - if it returns `Some(grab)`, delete `grab.start_byte..cursor` from the textarea and then append
    `c` to the buffer;
  - if it returns `None`, fall back to normal insertion.
- `BufferAppend`: append `c` to the active buffer.

### Plain non-ASCII `KeyCode::Char(c)` (no Ctrl/Alt modifiers)

`ChatComposer::handle_non_ascii_char` uses a slightly different flow:

- It first flushes any pending transient ASCII state with `PasteBurst::flush_before_modified_input`
  (which includes a single held ASCII char).
- If a burst is already active, `PasteBurst::try_append_char_if_active(c, now)` appends `c` directly.
- Otherwise it calls `PasteBurst::on_plain_char_no_hold(now)`:
  - `BufferAppend`: append `c` to the active buffer.
  - `BeginBuffer { retro_chars }`: run `decide_begin_buffer(..)` and, if it starts buffering, delete
    the retro-grabbed prefix from the textarea and append `c`.
  - `None`: insert `c` into the textarea normally.

The extra `decide_begin_buffer` heuristic on this path is intentional: IME input can arrive as
quick bursts, so the code only retro-grabs if the prefix “looks pastey” (whitespace, or a long
enough run) to avoid misclassifying IME composition as paste.

### `KeyCode::Enter`: newline vs submit

There are two distinct “Enter becomes newline” mechanisms:

- **While in a burst context** (`paste_burst.is_active()`): `append_newline_if_active(now)` appends
  `\n` into the burst buffer so multi-line pastes stay buffered as one explicit paste.
- **Immediately after burst activity** (enter suppression window):
  `newline_should_insert_instead_of_submit(now)` inserts `\n` into the textarea and calls
  `extend_window(now)` so a slightly-late Enter keeps behaving like “newline” rather than “submit”.

Both are disabled inside slash-command context (command popup is active or the first line begins
with `/`) so Enter keeps its normal “submit/execute” semantics while composing commands.

### Non-char keys / Ctrl+modified input

Non-char input must not leak burst state across unrelated actions:

- If there is buffered burst text, callers should flush it before calling
  `clear_window_after_non_char` (see “Pitfalls worth calling out”), typically via
  `PasteBurst::flush_before_modified_input`.
- `PasteBurst::clear_window_after_non_char` clears the “recent burst” window so the next keystroke
  doesn’t get incorrectly grouped into a previous paste.

### Pitfalls worth calling out

- `PasteBurst::clear_window_after_non_char` clears `last_plain_char_time`. If you call it while
  `buffer` is non-empty and _haven’t already flushed_, `flush_if_due()` no longer has a timestamp
  to time out against, so the buffered text may never flush. Treat `clear_window_after_non_char` as
  “drop classification context after flush”, not “flush”.
- `PasteBurst::flush_if_due` uses a strict `>` comparison, so tests and UI ticks should cross the
  threshold by at least 1ms (see `PasteBurst::recommended_flush_delay`).

## Notable interactions / invariants

- The composer frequently slices `textarea.text()` using the cursor position; all code that
  slices must clamp the cursor to a UTF-8 char boundary first.
- `sync_popups()` must run after any change that can affect popup visibility or filtering:
  inserting, deleting, flushing a burst, applying a paste placeholder, etc.
- Shortcut overlay toggling via `?` is gated on `!is_in_paste_burst()` so pastes cannot flip UI
  modes while streaming.
- Mention popup selection has two payloads: visible `$name` text and hidden
  `mention_paths[name] -> canonical target` linkage. The generic
  `set_text_content` path intentionally clears linkage for fresh drafts; restore
  paths that rehydrate blocked/interrupted submissions must use the
  mention-preserving setter so retry keeps the originally selected target.

## Tests that pin behavior

The `PasteBurst` logic is currently exercised through `ChatComposer` integration tests.

- `codex-rs/tui/src/bottom_pane/chat_composer.rs`
  - `non_ascii_burst_handles_newline`
  - `ascii_burst_treats_enter_as_newline`
  - `question_mark_does_not_toggle_during_paste_burst`
  - `burst_paste_fast_small_buffers_and_flushes_on_stop`
  - `burst_paste_fast_large_inserts_placeholder_on_flush`

This document calls out some additional contracts (like “flush before clearing”) that are not yet
fully pinned by dedicated `PasteBurst` unit tests.

---

## Source: tui-request-user-input.md

# Request user input overlay (TUI)

This note documents the TUI overlay used to gather answers for
`RequestUserInputEvent`.

## Overview

The overlay renders one question at a time and collects:

- A single selected option (when options exist).
- Freeform notes (always available).

When options are present, notes are stored per selected option and the first
option is selected by default, so every option question has an answer. If a
question has no options and no notes are provided, the answer is submitted as
`skipped`.

## Focus and input routing

The overlay tracks a small focus state:

- **Options**: Up/Down move the selection and Space selects.
- **Notes**: Text input edits notes for the currently selected option.

Typing while focused on options switches into notes automatically to reduce
friction for freeform input.

## Navigation

- Enter advances to the next question.
- Enter on the last question submits all answers.
- PageUp/PageDown navigate across questions (when multiple are present).
- Esc interrupts the run in option selection mode.
- When notes are open for an option question, Tab or Esc clears notes and returns
  to option selection.

## Layout priorities

The layout prefers to keep the question and all options visible. Notes and
footer hints collapse as space shrinks, with notes falling back to a single-line
"Notes: ..." input in tight terminals.

---

## Source: tui-stream-chunking-review.md

# TUI Stream Chunking

This document explains how stream chunking in the TUI works and why it is
implemented this way.

## Problem

Streaming output can arrive faster than a one-line-per-tick animation can show
it. If commit speed stays fixed while arrival speed spikes, queued lines grow
and visible output lags behind received output.

## Design goals

- Preserve existing baseline behavior under normal load.
- Reduce display lag when backlog builds.
- Keep output order stable.
- Avoid abrupt single-frame flushes that look jumpy.
- Keep policy transport-agnostic and based only on queue state.

## Non-goals

- The policy does not schedule animation ticks.
- The policy does not depend on upstream source identity.
- The policy does not reorder queued output.

## Where the logic lives

- `codex-rs/tui/src/streaming/chunking.rs`
  - Adaptive policy, mode transitions, and drain-plan selection.
- `codex-rs/tui/src/streaming/commit_tick.rs`
  - Orchestration for each commit tick: snapshot, decide, drain, trace.
- `codex-rs/tui/src/streaming/controller.rs`
  - Queue/drain primitives used by commit-tick orchestration.
- `codex-rs/tui/src/chatwidget.rs`
  - Integration point that invokes commit-tick orchestration and handles UI
    lifecycle events.

## Runtime flow

On each commit tick:

1. Build a queue snapshot across active controllers.
   - `queued_lines`: total queued lines.
   - `oldest_age`: max age of the oldest queued line across controllers.
2. Ask adaptive policy for a decision.
   - Output: current mode and a drain plan.
3. Apply drain plan to each controller.
4. Emit drained `HistoryCell`s for insertion by the caller.
5. Emit trace logs for observability.

In `CatchUpOnly` scope, policy state still advances, but draining is skipped
unless mode is currently `CatchUp`.

## Modes and transitions

Two modes are used:

- `Smooth`
  - Baseline behavior: one line drained per baseline commit tick.
  - Baseline tick interval currently comes from
    `tui/src/app.rs:COMMIT_ANIMATION_TICK` (~8.3ms, ~120fps).
- `CatchUp`
  - Drain current queued backlog per tick via `Batch(queued_lines)`.

Entry and exit use hysteresis:

- Enter `CatchUp` when queue depth or queue age exceeds enter thresholds.
- Exit requires both depth and age to be below exit thresholds for a hold
  window (`EXIT_HOLD`).

This prevents oscillation when load hovers near thresholds.

## Current experimental tuning values

These are the current values in `streaming/chunking.rs` plus the baseline
commit tick in `tui/src/app.rs`. They are
experimental and may change as we gather more trace data.

- Baseline commit tick: `~8.3ms` (`COMMIT_ANIMATION_TICK` in `app.rs`)
- Enter catch-up:
  - `queued_lines >= 8` OR `oldest_age >= 120ms`
- Exit catch-up eligibility:
  - `queued_lines <= 2` AND `oldest_age <= 40ms`
- Exit hold (`CatchUp -> Smooth`): `250ms`
- Re-entry hold after catch-up exit: `250ms`
- Severe backlog thresholds:
  - `queued_lines >= 64` OR `oldest_age >= 300ms`

## Drain planning

In `Smooth`, plan is always `Single`.

In `CatchUp`, plan is `Batch(queued_lines)`, which drains the currently queued
backlog for immediate convergence.

## Why this design

This keeps normal animation semantics intact, while making backlog behavior
adaptive:

- Under normal load, behavior stays familiar and stable.
- Under pressure, queue age is reduced quickly without sacrificing ordering.
- Hysteresis avoids rapid mode flapping.

## Invariants

- Queue order is preserved.
- Empty queue resets policy back to `Smooth`.
- `CatchUp` exits only after sustained low pressure.
- Catch-up drains are immediate while in `CatchUp`.

## Observability

Trace events are emitted from commit-tick orchestration:

- `stream chunking commit tick`
  - `mode`, `queued_lines`, `oldest_queued_age_ms`, `drain_plan`,
    `has_controller`, `all_idle`
- `stream chunking mode transition`
  - `prior_mode`, `new_mode`, `queued_lines`, `oldest_queued_age_ms`,
    `entered_catch_up`

These events are intended to explain display lag by showing queue pressure,
selected drain behavior, and mode transitions over time.

---

## Source: tui-stream-chunking-tuning.md

# TUI Stream Chunking Tuning Guide

This document explains how to tune adaptive stream chunking constants without
changing the underlying policy shape.

## Scope

Use this guide when adjusting queue-pressure thresholds and hysteresis windows in
`codex-rs/tui/src/streaming/chunking.rs`, and baseline commit cadence in
`codex-rs/tui/src/app.rs`.

This guide is about tuning behavior, not redesigning the policy.

## Before tuning

- Keep the baseline behavior intact:
  - `Smooth` mode drains one line per baseline tick.
  - `CatchUp` mode drains queued backlog immediately.
- Capture trace logs with:
  - `codex_tui::streaming::commit_tick`
- Evaluate on sustained, bursty, and mixed-output prompts.

See `docs/tui-stream-chunking-validation.md` for the measurement process.

## Tuning goals

Tune for all three goals together:

- low visible lag under bursty output
- low mode flapping (`Smooth <-> CatchUp` chatter)
- stable catch-up entry/exit behavior under mixed workloads

## Constants and what they control

### Baseline commit cadence

- `COMMIT_ANIMATION_TICK` (`tui/src/app.rs`)
  - Lower values increase smooth-mode update cadence and reduce steady-state lag.
  - Higher values increase smoothing and can increase perceived lag.
  - This should usually move after chunking thresholds/holds are in a good range.

### Enter/exit thresholds

- `ENTER_QUEUE_DEPTH_LINES`, `ENTER_OLDEST_AGE`
  - Lower values enter catch-up earlier (less lag, more mode switching risk).
  - Higher values enter later (more lag tolerance, fewer mode switches).
- `EXIT_QUEUE_DEPTH_LINES`, `EXIT_OLDEST_AGE`
  - Lower values keep catch-up active longer.
  - Higher values allow earlier exit and may increase re-entry churn.

### Hysteresis holds

- `EXIT_HOLD`
  - Longer hold reduces flip-flop exits when pressure is noisy.
  - Too long can keep catch-up active after pressure has cleared.
- `REENTER_CATCH_UP_HOLD`
  - Longer hold suppresses rapid re-entry after exit.
  - Too long can delay needed catch-up for near-term bursts.
  - Severe backlog bypasses this hold by design.

### Severe-backlog gates

- `SEVERE_QUEUE_DEPTH_LINES`, `SEVERE_OLDEST_AGE`
  - Lower values bypass re-entry hold earlier.
  - Higher values reserve hold bypass for only extreme pressure.

## Recommended tuning order

Tune in this order to keep cause/effect clear:

1. Entry/exit thresholds (`ENTER_*`, `EXIT_*`)
2. Hold windows (`EXIT_HOLD`, `REENTER_CATCH_UP_HOLD`)
3. Severe gates (`SEVERE_*`)
4. Baseline cadence (`COMMIT_ANIMATION_TICK`)

Change one logical group at a time and re-measure before the next group.

## Symptom-driven adjustments

- Too much lag before catch-up starts:
  - lower `ENTER_QUEUE_DEPTH_LINES` and/or `ENTER_OLDEST_AGE`
- Frequent `Smooth -> CatchUp -> Smooth` chatter:
  - increase `EXIT_HOLD`
  - increase `REENTER_CATCH_UP_HOLD`
  - tighten exit thresholds (lower `EXIT_*`)
- Catch-up engages too often for short bursts:
  - increase `ENTER_QUEUE_DEPTH_LINES` and/or `ENTER_OLDEST_AGE`
  - increase `REENTER_CATCH_UP_HOLD`
- Catch-up engages too late:
  - lower `ENTER_QUEUE_DEPTH_LINES` and/or `ENTER_OLDEST_AGE`
  - lower severe gates (`SEVERE_*`) to bypass re-entry hold sooner

## Validation checklist after each tuning pass

- `cargo test -p codex-tui` passes.
- Trace window shows bounded queue-age behavior.
- Mode transitions are not concentrated in repeated short-interval cycles.
- Catch-up clears backlog quickly once mode enters `CatchUp`.

---

## Source: tui-stream-chunking-validation.md

# TUI Stream Chunking Validation Process

This document records the process used to validate adaptive stream chunking
and anti-flap behavior.

## Scope

The goal is to verify two properties from runtime traces:

- display lag is reduced when queue pressure rises
- mode transitions remain stable instead of rapidly flapping

## Trace targets

Chunking observability is emitted by:

- `codex_tui::streaming::commit_tick`

Two trace messages are used:

- `stream chunking commit tick`
- `stream chunking mode transition`

## Runtime command

Run Codex with chunking traces enabled:

```bash
RUST_LOG='codex_tui::streaming::commit_tick=trace,codex_tui=info,codex_core=info,codex_rmcp_client=info' \
  just codex --enable=responses_websockets
```

## Log capture process

Tip: for one-off measurements, run with `-c log_dir=...` to direct logs to a fresh directory and avoid mixing sessions.

1. Record the current size of `~/.codex/log/codex-tui.log` as a start offset.
2. Run an interactive prompt that produces sustained streamed output.
3. Stop the run.
4. Parse only log bytes written after the recorded offset.

This avoids mixing earlier sessions with the current measurement window.

## Metrics reviewed

For each measured window:

- `commit_ticks`
- `mode_transitions`
- `smooth_ticks`
- `catchup_ticks`
- drain-plan distribution (`Single`, `Batch(n)`)
- queue depth (`max`, `p95`, `p99`)
- oldest queued age (`max`, `p95`, `p99`)
- rapid re-entry count:
  - number of `Smooth -> CatchUp` transitions within 1 second of a
    `CatchUp -> Smooth` transition

## Interpretation

- Healthy behavior:
  - queue age remains bounded while backlog is drained
  - transition count is low relative to total ticks
  - rapid re-entry events are infrequent and localized to burst boundaries
- Regressed behavior:
  - repeated short-interval mode toggles across an extended window
  - persistent queue-age growth while in smooth mode
  - long catch-up runs without backlog reduction

## Experiment history

This section captures the major tuning passes so future work can build on
what has already been tried.

- Baseline
  - One-line smooth draining with a 50ms commit tick.
  - This preserved familiar pacing but could feel laggy under sustained
    backlog.
- Pass 1: instant catch-up, baseline tick unchanged
  - Kept smooth-mode semantics but made catch-up drain the full queued
    backlog each catch-up tick.
  - Result: queue lag dropped faster, but perceived motion could still feel
    stepped because smooth-mode cadence remained coarse.
- Pass 2: faster baseline tick (25ms)
  - Improved smooth-mode cadence and reduced visible stepping.
  - Result: better, but still not aligned with draw cadence.
- Pass 3: frame-aligned baseline tick (~16.7ms)
  - Set baseline commit cadence to approximately 60fps.
  - Result: smoother perceived progression while retaining hysteresis and
    fast backlog convergence.
- Pass 4: higher frame-aligned baseline tick (~8.3ms)
  - Set baseline commit cadence to approximately 120fps.
  - Result: further reduced smooth-mode stepping while preserving the same
    adaptive catch-up policy shape.

Current state combines:

- instant catch-up draining in `CatchUp`
- hysteresis for mode-entry/exit stability
- frame-aligned smooth-mode commit cadence (~8.3ms)

## Notes

- Validation is source-agnostic and does not rely on naming any specific
  upstream provider.
- This process intentionally preserves existing baseline smooth behavior and
  focuses on burst/backlog handling behavior.

---
