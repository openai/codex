# Terminal Hyperlinks

Manual smoke tests for the `terminal_hyperlinks` feature.

## Build

```sh
cd /Users/steipete/openai/codex2
just fmt
cd /Users/steipete/openai/codex2/codex-rs
cargo build -p codex-cli
```

## VS Code Integrated Terminal

Codex detects VS Code via `TERM_PROGRAM=vscode` and leaves local file references as plain text so
the integrated terminal's native file-link detection can handle them.

```sh
cd /Users/steipete/openai/codex2
just codex --enable terminal_hyperlinks
```

In the session, ask:

```text
Reply with exactly these references on separate lines:
Cargo.toml:1
codex-rs/tui/src/terminal_hyperlinks.rs:1
codex-rs/core/src/context_manager/updates.rs:1
https://example.com/docs
```

Expected results:

- `Cargo.toml:1` opens the workspace file in VS Code.
- `codex-rs/tui/src/terminal_hyperlinks.rs:1` opens that file in VS Code.
- The web URL opens in the browser.
- No absolute workspace paths should be necessary.

## iTerm2 / Ghostty / Other Native Terminals

Outside VS Code, Codex emits OSC 8 hyperlinks for URLs and local files.

```sh
cd /Users/steipete/openai/codex2
just codex --enable terminal_hyperlinks
```

Use the same prompt as above, then verify:

- Web URLs are clickable as OSC 8 hyperlinks.
- Local files are clickable as OSC 8 hyperlinks.
- Bare filenames such as `Cargo.toml:1` open the cwd-relative file.
- Longer workspace paths may render with collapsed middle directories such as
  `codex-rs/.../bottom_pane/chat_composer.rs:1` while still opening the full resolved file.

## Focused Test Commands

```sh
cd /Users/steipete/openai/codex2/codex-rs
cargo test -p codex-core terminal_hyperlinks
cargo test -p codex-tui terminal_hyperlinks
cargo test -p codex-tui file_references
```
