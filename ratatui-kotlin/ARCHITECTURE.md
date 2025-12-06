# Ratatui Architecture

This document provides a comprehensive overview of Ratatui's architecture and crate
organization, introduced in version 0.30.0.

## Overview

Starting with Ratatui 0.30.0, the project was reorganized from a single monolithic crate into
a modular workspace consisting of multiple specialized crates. This architectural decision was
made to improve modularity, reduce compilation times, enable more flexible dependency
management, and provide better API stability for third-party widget libraries.

## Crate Organization

The Ratatui project is now organized as a Cargo workspace containing the following crates:

### Core Crates

#### `ratatui` (Main Crate)

- **Purpose**: The main entry point that most applications should use
- **Contents**: Re-exports everything from other crates for convenience, plus experimental features
- **Target Users**: Application developers building terminal UIs
- **Key Features**:
  - Complete widget ecosystem
  - Backend implementations
  - Layout system
  - Terminal management
  - Experimental `WidgetRef` and `StatefulWidgetRef` traits

#### `ratatui-core`

- **Purpose**: Foundational types and traits for the Ratatui ecosystem
- **Contents**: Core widget traits, text types, buffer, layout, style, and symbols
- **Target Users**: Widget library authors, minimalist projects
- **Key Features**:
  - `Widget` and `StatefulWidget` traits
  - Text rendering (`Text`, `Line`, `Span`)
  - Buffer management
  - Layout system
  - Style and color definitions
  - Symbol collections

#### `ratatui-widgets`

- **Purpose**: Built-in widget implementations
- **Contents**: All standard widgets like `Block`, `Paragraph`, `List`, `Chart`, etc.
- **Target Users**: Applications needing standard widgets, widget library authors
- **Key Features**:
  - Complete set of built-in widgets
  - Optimized implementations
  - Comprehensive documentation and examples

### Backend Crates

#### `ratatui-crossterm`

- **Purpose**: Crossterm backend implementation
- **Contents**: Cross-platform terminal backend using the `crossterm` crate
- **Target Users**: Applications targeting multiple platforms

#### `ratatui-termion`

- **Purpose**: Termion backend implementation
- **Contents**: Unix-specific terminal backend using the `termion` crate
- **Target Users**: Unix-specific applications requiring low-level control

#### `ratatui-termwiz`

- **Purpose**: Termwiz backend implementation
- **Contents**: Terminal backend using the `termwiz` crate
- **Target Users**: Applications needing advanced terminal features

### Utility Crates

#### `ratatui-macros`

- **Purpose**: Declarative macros for common patterns and boilerplate reduction
- **Contents**: Macros for common patterns and boilerplate reduction
- **Target Users**: Applications and libraries wanting macro support

## Dependency Relationships

```text
ratatui
├── ratatui-core
├── ratatui-widgets → ratatui-core
├── ratatui-crossterm → ratatui-core
├── ratatui-termion → ratatui-core
├── ratatui-termwiz → ratatui-core
└── ratatui-macros
```

### Key Dependencies

- **ratatui-core**: Foundation for all other crates
- **ratatui-widgets**: Depends on `ratatui-core` for widget traits and types
- **Backend crates**: Each depends on `ratatui-core` for backend traits and types
- **ratatui**: Depends on all other crates and re-exports their public APIs

## Design Principles

### Stability and Compatibility

The modular architecture provides different levels of API stability:

- **ratatui-core**: Designed for maximum stability to minimize breaking changes for widget
  libraries
- **ratatui-widgets**: Focused on widget implementations with moderate stability requirements
- **Backend crates**: Isolated from core changes, allowing backend-specific updates
- **ratatui**: Main crate that can evolve more freely while maintaining backward compatibility
  through re-exports

### Compilation Performance

The split architecture enables:

- **Reduced compilation times**: Widget libraries only need to compile core types
- **Parallel compilation**: Different crates can be compiled in parallel
- **Selective compilation**: Applications can exclude unused backends or widgets

### Ecosystem Benefits

- **Widget Library Authors**: Can depend on stable `ratatui-core` without frequent updates
- **Application Developers**: Can use the convenient `ratatui` crate with everything included
- **Minimalist Projects**: Can use only `ratatui-core` for lightweight applications

## Migration Guide

### For Application Developers

Most applications should continue using the main `ratatui` crate with minimal changes:

```rust
// No changes needed - everything is re-exported
use ratatui::{
    widgets::{Block, Paragraph},
    layout::{Layout, Constraint},
    Terminal,
};
```

### For Widget Library Authors

Consider migrating to `ratatui-core` for better stability:

```rust
// Before (0.29.x and earlier)
use ratatui::{
    widgets::{Widget, StatefulWidget},
    buffer::Buffer,
    layout::Rect,
};

// After (0.30.0+)
use ratatui_core::{
    widgets::{Widget, StatefulWidget},
    buffer::Buffer,
    layout::Rect,
};
```

### Backwards Compatibility

All existing code using the `ratatui` crate will continue to work unchanged, as the main crate
re-exports all public APIs from the specialized crates.

## Future Considerations

### Potential Enhancements

- **Widget-specific crates**: Further split widgets into individual crates for even more
  granular dependencies
- **Plugin system**: Enable dynamic widget loading and third-party widget ecosystems
- **Feature flags**: More granular feature flags for compile-time customization

### Version Synchronization

Currently, all crates are versioned together for simplicity. Future versions may adopt
independent versioning once the API stabilizes further.

## Related Issues and PRs

This architecture was developed through extensive discussion and implementation across multiple
PRs:

- [Issue #1388](https://github.com/ratatui/ratatui/issues/1388): Original RFC for modularization
- [PR #1459](https://github.com/ratatui/ratatui/pull/1459): Move ratatui crate into workspace
  folder
- [PR #1460](https://github.com/ratatui/ratatui/pull/1460): Move core types to ratatui-core
- [PR #1474](https://github.com/ratatui/ratatui/pull/1474): Move widgets into ratatui-widgets
  crate

## Contributing

When contributing to the Ratatui project, please consider:

- **Core changes**: Submit PRs against `ratatui-core` for fundamental improvements
- **Widget changes**: Submit PRs against `ratatui-widgets` for widget-specific improvements
- **Backend changes**: Submit PRs against the appropriate backend crate
- **Integration changes**: Submit PRs against the main `ratatui` crate

See the [CONTRIBUTING.md](CONTRIBUTING.md) guide for more details on the contribution process.
