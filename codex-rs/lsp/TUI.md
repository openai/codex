# LSP Test TUI

A simple TUI for testing `codex-lsp` functionality and inspecting internal LSP operations.

## Quick Start

```bash
# Build with TUI feature
cargo build -p codex-lsp --features tui

# Run with a workspace directory
cargo run -p codex-lsp --features tui -- /path/to/rust/project

# Or specify an initial file
cargo run -p codex-lsp --features tui -- /path/to/project -f src/lib.rs
```

## UI Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ LSP Test TUI                                                    │
│ Server: connected | Workspace: /path/to/project                 │
├─────────────────────────────────────────────────────────────────┤
│ Mode: Menu | [0-9] Select  [d] Diagnostics  [?] Help  [q] Quit  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Select an LSP operation:                                       │
│                                                                 │
│  > 1. Go to Definition                                          │
│    2. Type Definition                                           │
│    3. Go to Declaration                                         │
│    4. Find References                                           │
│    5. Find Implementations                                      │
│    6. Hover Info                                                │
│    7. Workspace Symbol                                          │
│    8. Document Symbols                                          │
│    9. Call Hierarchy                                            │
│   10. Health Check                                              │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│ [Enter] Select  [↑↓] Navigate  [d] Diagnostics  [?] Help  [q]   │
└─────────────────────────────────────────────────────────────────┘
```

## Keyboard Shortcuts

### Global

| Key | Action |
|-----|--------|
| `Ctrl+C` | Quit application |
| `?` / `h` | Show help |

### Menu Mode

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate operations |
| `1-9`, `0` | Quick select operation (0 = 10th) |
| `Enter` | Select operation |
| `d` | View diagnostics |
| `q` | Quit |

### Input Mode (File/Symbol)

| Key | Action |
|-----|--------|
| `Enter` | Confirm input |
| `Esc` | Cancel and return to menu |
| `←` / `→` | Move cursor |
| `Ctrl+←` / `Ctrl+→` | Move cursor by word |
| `Home` | Jump to start |
| `End` | Jump to end |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |
| `Ctrl+U` | Clear line before cursor |
| `Ctrl+K` | Clear line after cursor |

### Results/Diagnostics Mode

| Key | Action |
|-----|--------|
| `↑` / `↓` | Scroll results |
| `PageUp` / `PageDown` | Scroll 10 lines |
| `Home` | Jump to top |
| `End` | Jump to bottom |
| `Esc` / `q` | Return to menu |

## LSP Operations

### 1. Go to Definition
Find where a symbol is defined.
- **Requires**: File path + Symbol name
- **Returns**: Location(s) of definition

### 2. Type Definition
Find the type's definition (e.g., struct definition for a variable).
- **Requires**: File path + Symbol name
- **Returns**: Location(s) of type definition

### 3. Go to Declaration
Find where a symbol is declared (useful in languages with separate declaration/definition).
- **Requires**: File path + Symbol name
- **Returns**: Location(s) of declaration

### 4. Find References
Find all usages of a symbol.
- **Requires**: File path + Symbol name
- **Returns**: List of locations where symbol is referenced

### 5. Find Implementations
Find implementations of a trait or interface.
- **Requires**: File path + Symbol name (trait/interface)
- **Returns**: Location(s) of implementations

### 6. Hover Info
Get documentation and type information for a symbol.
- **Requires**: File path + Symbol name
- **Returns**: Documentation, type signature, etc.

### 7. Workspace Symbol
Search for symbols across the entire workspace.
- **Requires**: Search query
- **Returns**: List of matching symbols with locations

### 8. Document Symbols
List all symbols in a file.
- **Requires**: File path
- **Returns**: List of all symbols (functions, structs, etc.)

### 9. Call Hierarchy
Show incoming (who calls this) and outgoing (what this calls) calls for a function.
- **Requires**: File path + Symbol name (function)
- **Returns**: Incoming and outgoing call lists

### 10. Health Check
Check if the language server is running and responsive.
- **Requires**: Nothing (or optional file to select server)
- **Returns**: Health status

## Usage Examples

### Example 1: Find Definition

1. Start TUI: `cargo run -p codex-lsp --features tui -- .`
2. Press `1` or `Enter` on "Go to Definition"
3. Enter file path: `src/lib.rs`
4. Press `Enter`
5. Enter symbol name: `LspClient`
6. Press `Enter`
7. View results showing definition location

### Example 2: List Document Symbols

1. Press `6` to select "Document Symbols"
2. Enter file path: `src/client.rs`
3. Press `Enter`
4. View all symbols in the file

### Example 3: Check Diagnostics

1. Press `d` to view diagnostics
2. Scroll with `↑` / `↓`
3. Press `Esc` to return to menu

## Supported Language Servers

The TUI automatically detects and connects to language servers based on file extensions:

| Language | Server | Extensions |
|----------|--------|------------|
| Rust | rust-analyzer | `.rs` |
| Go | gopls | `.go` |
| Python | pyright | `.py`, `.pyi` |
| TypeScript/JavaScript | typescript-language-server | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` |

## Troubleshooting

### Server Not Connecting

1. Ensure the language server is installed:
   - Rust: `rustup component add rust-analyzer`
   - Go: `go install golang.org/x/tools/gopls@latest`
   - Python: `npm install -g pyright`
   - TypeScript: `npm install -g typescript-language-server typescript`

2. Check logs (output to stderr):
   ```bash
   cargo run -p codex-lsp --features tui -- . 2>lsp.log
   ```

### No Results Found

- Verify the file path is correct (relative to workspace root)
- Ensure the symbol name matches exactly (case-sensitive for some operations)
- Check if the file has been saved and synced with the server

## Development

The TUI is implemented as an optional feature of the `codex-lsp` crate:

```
lsp/src/tui/
├── main.rs         # CLI entry point
├── app.rs          # Application state machine
├── event.rs        # Event types
├── ops.rs          # LSP operation wrappers
└── ui/
    ├── mod.rs          # Main render function
    ├── status_bar.rs   # Status display
    ├── menu.rs         # Operation menu
    ├── input_box.rs    # Text input
    ├── result_view.rs  # Results display
    ├── diagnostics.rs  # Diagnostics panel
    └── help.rs         # Help screen
```

### Building

```bash
# Build only the TUI binary
cargo build -p codex-lsp --features tui

# Run tests
cargo test -p codex-lsp

# Check without building
cargo check -p codex-lsp --features tui
```
