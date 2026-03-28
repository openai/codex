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
