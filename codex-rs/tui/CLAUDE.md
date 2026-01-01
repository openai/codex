# TUI Development Guidelines

**Goal: Minimize upstream merge conflicts.**

## Development Principles

1. **ALWAYS use `*_ext.rs` pattern** for new functionality
2. **Keep upstream code unchanged** - don't modify existing code style
3. **Minimal integration** in original files (1-2 lines import/call)

## Code Style

See `styles.md`. **NEVER** use `.white()`.
