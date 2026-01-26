# TUI themes

Codex TUI supports a small set of color themes selectable at runtime.

Usage:

```bash
codex --theme default
codex --theme ivory-ember
codex --theme thames-fog
```

In the running TUI, use:

```text
/theme
/theme thames-fog
/thame thames-fog
```

Notes:

- Themes remap ANSI colors used by the TUI (no custom RGB), so they adapt to your terminal palette.
- The default theme matches the existing color scheme.
