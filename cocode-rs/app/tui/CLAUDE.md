# CLAUDE.md - cocode-tui Development Guide

## Architecture

The TUI follows **The Elm Architecture (TEA)** with async event handling:

```
┌─────────────────────────────────────────────────────────────────┐
│                         TUI Layer                                │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │  Model   │◄───│  Update  │◄───│  Render  │◄───│  Events  │  │
│  │(AppState)│    │(update.rs)│   │(render.rs)│   │(stream.rs)│  │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘  │
└─────────────────────────────────────────────────────────────────┘
         ▲                                              │
         └──────────────────────────────────────────────┘
```

## Key Files

| File | Purpose |
|------|---------|
| `app.rs` | Main `App` struct, async run loop with `tokio::select!` |
| `command.rs` | `UserCommand` enum (TUI → Core) |
| `update.rs` | `handle_command()`, `handle_agent_event()` |
| `render.rs` | `render()` function, overlay rendering |
| `state/mod.rs` | `AppState`, `SessionState`, `UiState` |
| `event/mod.rs` | `TuiEvent`, `TuiCommand` |
| `widgets/` | `ChatWidget`, `InputWidget`, `StatusBar`, `ToolPanel` |

## Communication Channels

```rust
// Core → TUI: LoopEvent (streaming, tools, approvals)
let (agent_tx, agent_rx) = mpsc::channel::<LoopEvent>(32);

// TUI → Core: UserCommand (input, interrupts, settings)
let (command_tx, command_rx) = mpsc::channel::<UserCommand>(32);
```

**UserCommand variants:** `SubmitInput`, `Interrupt`, `SetPlanMode`, `SetThinkingLevel`, `SetModel`, `ApprovalResponse`, `Shutdown`

## Keyboard Shortcuts

| Key | Action | TuiCommand |
|-----|--------|------------|
| Tab | Toggle plan mode | `TogglePlanMode` |
| Ctrl+T | Cycle thinking | `CycleThinkingLevel` |
| Ctrl+M | Model picker | `CycleModel` |
| Ctrl+C | Interrupt | `Interrupt` |
| Ctrl+Q | Quit | `Quit` |
| Enter | Submit | `SubmitInput` |
| Shift+Enter | Newline | `InsertNewline` |
| Esc | Cancel/close | `Cancel` |

## Styling Rules

```rust
// GOOD - use Stylize trait
"text".dim()
"text".bold().cyan()
Span::raw("text").green()

// BAD - manual Style
Span::styled("text", Style::default().fg(Color::Cyan))

// NEVER use .white() - breaks themes
```

## State Structure

```rust
pub struct AppState {
    pub session: SessionState,  // model, thinking_level, plan_mode, messages, tools
    pub ui: UiState,            // input, scroll, focus, overlay, streaming
    pub running: RunningState,  // Running | Done
}
```

## Adding New Features

1. **New keyboard shortcut**: Add to `event/handler.rs` → `TuiCommand`
2. **New overlay**: Add variant to `state/ui.rs::Overlay`, render in `render.rs`
3. **Handle new LoopEvent**: Add case in `update.rs::handle_agent_event()`
4. **New widget**: Create in `widgets/`, use in `render.rs`

## Development Commands

```bash
# From codex/ directory
cargo check -p cocode-tui --manifest-path cocode-rs/Cargo.toml
cargo test -p cocode-tui --manifest-path cocode-rs/Cargo.toml
cargo build --manifest-path cocode-rs/Cargo.toml  # Pre-commit REQUIRED
```

## Code Conventions

**DO:**
- Use `i32`/`i64` (never `u32`/`u64`)
- Inline format args: `format!("{var}")`
- Chain Stylize helpers
- Filter `KeyEventKind::Press` for cross-platform

**DON'T:**
- Use `.unwrap()` in non-test code
- Use `.white()` (breaks themes)
- Block the render loop
