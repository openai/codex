# LSP Quick Start Guide

Step-by-step guide to set up and use the Codex LSP client library.

## Prerequisites

Before starting, ensure you have:

- Rust toolchain (for rust-analyzer)
- Go toolchain (for gopls)
- Node.js (for pyright and typescript-language-server)

## Step 1: Install LSP Servers

Install the LSP servers for the languages you need.

### Rust (rust-analyzer)

```bash
# Mac/Linux
rustup component add rust-analyzer

# Verify installation
which rust-analyzer
rust-analyzer --version
```

### Go (gopls)

```bash
# Mac/Linux
go install golang.org/x/tools/gopls@latest

# Verify installation
which gopls
gopls version
```

### Python (pyright)

```bash
# Mac/Linux
npm install -g pyright

# Verify installation
which pyright-langserver
pyright --version
```

### TypeScript/JavaScript (typescript-language-server)

```bash
# Mac/Linux
npm install -g typescript-language-server typescript

# Verify installation
which typescript-language-server
typescript-language-server --version
```

## Step 2: Create Configuration Directory

```bash
# Create global config directory
mkdir -p ~/.codex

# Or create project-level config directory
mkdir -p .codex
```

## Step 3: Configuration File

Create `lsp_servers.json` in either location:
- Global: `~/.codex/lsp_servers.json`
- Project: `.codex/lsp_servers.json` (overrides global)

### Minimal Configuration

No configuration needed for built-in servers (rust-analyzer, gopls, pyright). They work out of the box.

---

## Built-in vs Custom Server

### Key Differences

| Feature | Built-in Server | Custom Server |
|---------|-----------------|---------------|
| **Server ID** | `rust-analyzer`, `gopls`, `pyright` | Any unique name (e.g., `typescript`, `clangd`) |
| **command** | Optional (auto-detected) | **Required** |
| **file_extensions** | Optional (pre-configured) | **Required** |
| **Zero Config** | Yes, works out of the box | No, must configure command & extensions |
| **Override Settings** | Only specify what you want to change | Must specify all required fields |

### Built-in Servers (Zero Config)

These servers work immediately after installation - no configuration needed:

| Server ID | Command | Extensions | Languages |
|-----------|---------|------------|-----------|
| `rust-analyzer` | `rust-analyzer` | `.rs` | `rust` |
| `gopls` | `gopls` | `.go` | `go` |
| `pyright` | `pyright-langserver --stdio` | `.py`, `.pyi` | `python` |
| `typescript-language-server` | `typescript-language-server --stdio` | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | `typescript`, `javascript` |

**Example: Override built-in (only changed values needed)**
```json
{
  "servers": {
    "rust-analyzer": {
      "max_restarts": 5
    }
  }
}
```

### Custom Servers (Must Configure)

For any other language server, you must specify `command` and `file_extensions`:

**Example: Add TypeScript server**
```json
{
  "servers": {
    "typescript": {
      "command": "typescript-language-server",
      "args": ["--stdio"],
      "file_extensions": [".ts", ".tsx", ".js", ".jsx"],
      "languages": ["typescript", "javascript"]
    }
  }
}
```

---

## Quick Start Configs (Copy & Use)

### rust-analyzer - Recommended Config

```bash
cat > ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "rust-analyzer": {
      "max_restarts": 5,
      "startup_timeout_ms": 30000,
      "request_timeout_ms": 45000,
      "initialization_options": {
        "checkOnSave": {
          "command": "clippy"
        },
        "cargo": {
          "features": "all"
        },
        "procMacro": {
          "enable": true
        }
      }
    }
  }
}
EOF
```

**What this enables:**
- Clippy checks on save (better linting)
- All cargo features enabled
- Proc macro support
- Longer timeouts for large projects
- More restart attempts

### gopls - Recommended Config

```bash
cat > ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "gopls": {
      "max_restarts": 5,
      "startup_timeout_ms": 20000,
      "initialization_options": {
        "staticcheck": true,
        "gofumpt": true,
        "usePlaceholders": true,
        "analyses": {
          "unusedparams": true,
          "shadow": true,
          "nilness": true
        },
        "hints": {
          "parameterNames": true,
          "assignVariableTypes": true
        }
      }
    }
  }
}
EOF
```

**What this enables:**
- Staticcheck integration (extra linting)
- Gofumpt formatting (stricter gofmt)
- Completion placeholders
- Additional analyses (unused params, shadowing)
- Inlay hints for parameter names

### pyright - Recommended Config

```bash
cat > ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "pyright": {
      "max_restarts": 5,
      "startup_timeout_ms": 20000,
      "initialization_options": {
        "python": {
          "analysis": {
            "typeCheckingMode": "standard",
            "autoSearchPaths": true,
            "useLibraryCodeForTypes": true,
            "diagnosticSeverityOverrides": {
              "reportMissingImports": "error",
              "reportUnusedImport": "warning",
              "reportUnusedVariable": "warning"
            }
          }
        }
      }
    }
  }
}
EOF
```

**What this enables:**
- Standard type checking (balanced strictness)
- Auto-detect import paths
- Library type inference
- Error on missing imports
- Warnings for unused imports/variables

### typescript-language-server - Recommended Config

```bash
cat > ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "typescript-language-server": {
      "max_restarts": 5,
      "startup_timeout_ms": 20000,
      "initialization_options": {
        "preferences": {
          "includeInlayParameterNameHints": "all",
          "includeInlayVariableTypeHints": true,
          "includeInlayFunctionLikeReturnTypeHints": true,
          "includeInlayPropertyDeclarationTypeHints": true
        },
        "tsserver": {
          "logVerbosity": "off"
        }
      }
    }
  }
}
EOF
```

**What this enables:**
- Inlay hints for parameter names
- Variable type hints
- Function return type hints
- Property declaration type hints

### All Four Servers - Combined Config

```bash
cat > ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "rust-analyzer": {
      "max_restarts": 5,
      "startup_timeout_ms": 30000,
      "initialization_options": {
        "checkOnSave": {"command": "clippy"},
        "cargo": {"features": "all"},
        "procMacro": {"enable": true}
      }
    },
    "gopls": {
      "max_restarts": 5,
      "initialization_options": {
        "staticcheck": true,
        "gofumpt": true,
        "analyses": {"unusedparams": true, "shadow": true}
      }
    },
    "pyright": {
      "max_restarts": 5,
      "initialization_options": {
        "python": {
          "analysis": {
            "typeCheckingMode": "standard",
            "diagnosticSeverityOverrides": {
              "reportUnusedImport": "warning"
            }
          }
        }
      }
    },
    "typescript-language-server": {
      "max_restarts": 5,
      "initialization_options": {
        "preferences": {
          "includeInlayParameterNameHints": "all",
          "includeInlayVariableTypeHints": true
        }
      }
    }
  }
}
EOF
```

### Custom Servers - Ready to Use

**C/C++ (clangd):**
```bash
# Mac: brew install llvm
# Linux: sudo apt install clangd

cat >> ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "clangd": {
      "command": "clangd",
      "args": ["--background-index", "--clang-tidy"],
      "file_extensions": [".c", ".cpp", ".h", ".hpp"],
      "languages": ["c", "cpp"]
    }
  }
}
EOF
```

---

## Complete Configuration Reference

### Configuration File Structure

```json
{
  "servers": {
    "<server-id>": {
      // All available configuration options listed below
    }
  }
}
```

### All Available Configuration Options

Below is the **complete list** of all configuration options for each LSP server:

```json
{
  "servers": {
    "<server-id>": {
      // ========================================
      // Server Control
      // ========================================

      "disabled": false,
      // Type: boolean
      // Default: false
      // Description: Set to true to completely disable this server.
      //              Useful for temporarily disabling a built-in server.

      "command": "language-server-binary",
      // Type: string (optional for built-in, required for custom)
      // Default: null (uses built-in command for rust-analyzer/gopls/pyright)
      // Description: The executable command to start the LSP server.
      //              For built-in servers, this overrides the default command.
      //              For custom servers, this is REQUIRED.
      // Examples:
      //   - "rust-analyzer"
      //   - "gopls"
      //   - "pyright-langserver"
      //   - "typescript-language-server"
      //   - "/usr/local/bin/my-custom-lsp"

      "args": ["--stdio", "--log-level=info"],
      // Type: string[]
      // Default: []
      // Description: Command-line arguments passed to the LSP server.
      // Examples:
      //   - ["--stdio"]
      //   - ["--stdio", "--log-level=debug"]
      //   - ["--background-index", "--clang-tidy"]

      "file_extensions": [".ts", ".tsx"],
      // Type: string[]
      // Default: [] (uses built-in extensions for built-in servers)
      // Description: File extensions this server handles.
      //              REQUIRED for custom servers.
      //              For built-in servers, this extends the default list.
      // Examples:
      //   - [".rs"]
      //   - [".go", ".mod"]
      //   - [".py", ".pyi", ".pyw"]
      //   - [".c", ".cpp", ".cc", ".h", ".hpp"]

      "languages": ["typescript", "javascript"],
      // Type: string[]
      // Default: [] (uses built-in languages for built-in servers)
      // Description: Language identifiers for LSP protocol.
      //              Used in textDocument/didOpen notifications.
      // Examples:
      //   - ["rust"]
      //   - ["go"]
      //   - ["python"]
      //   - ["c", "cpp"]

      "env": {
        "RUST_LOG": "debug",
        "MY_VAR": "value"
      },
      // Type: object (string -> string)
      // Default: {}
      // Description: Environment variables set when spawning the server process.
      //              Useful for configuring server behavior or paths.
      // Examples:
      //   - {"GOPATH": "/custom/gopath"}
      //   - {"VIRTUAL_ENV": "/path/to/venv"}
      //   - {"RUST_BACKTRACE": "1"}

      // ========================================
      // LSP Protocol Options
      // ========================================

      "initialization_options": {
        "checkOnSave": {"command": "clippy"}
      },
      // Type: any JSON value
      // Default: null
      // Description: Options sent to the server during LSP initialization.
      //              Server-specific. See each server's documentation.
      // Examples for rust-analyzer:
      //   - {"checkOnSave": {"command": "clippy"}}
      //   - {"cargo": {"features": "all"}}
      //   - {"procMacro": {"enable": true}}
      // Examples for gopls:
      //   - {"staticcheck": true}
      //   - {"gofumpt": true}
      // Examples for pyright:
      //   - {"python": {"analysis": {"typeCheckingMode": "strict"}}}

      "settings": {
        "rust-analyzer": {"cargo": {"features": "all"}}
      },
      // Type: any JSON value
      // Default: null
      // Description: Workspace settings sent via workspace/didChangeConfiguration.
      //              Server-specific. See each server's documentation.
      // Note: Some servers prefer initialization_options, others prefer settings.
      //       Check your server's documentation.

      "workspace_folder": "/path/to/project",
      // Type: string (optional)
      // Default: null (auto-detected from file path)
      // Description: Explicit workspace folder path.
      //              Normally auto-detected; use this to override.
      // Examples:
      //   - "/home/user/my-project"
      //   - "/Users/dev/workspace/my-app"

      // ========================================
      // Lifecycle & Restart Settings
      // ========================================

      "max_restarts": 3,
      // Type: integer
      // Default: 3
      // Range: 0-100 (0 = never restart)
      // Description: Maximum restart attempts before giving up.
      //              After this many crashes, server enters "Failed" state.
      // Tuning:
      //   - Set 0 to disable auto-restart (for debugging)
      //   - Set 5-10 for unstable environments
      //   - Set 1-2 for quick failure detection

      "restart_on_crash": true,
      // Type: boolean
      // Default: true
      // Description: Whether to automatically restart the server on crash.
      //              Set to false for debugging server issues.

      // ========================================
      // Timeout Settings (all in milliseconds)
      // ========================================

      "startup_timeout_ms": 10000,
      // Type: integer (milliseconds)
      // Default: 10000 (10 seconds)
      // Range: 1000-300000
      // Description: Maximum time to wait for server initialization.
      //              If exceeded, throws InitializationTimeout error.
      // Tuning:
      //   - Large Rust projects: 30000-60000 (rust-analyzer needs time)
      //   - Small projects: 5000-10000
      //   - CI environments: 60000 (slower I/O)

      "shutdown_timeout_ms": 5000,
      // Type: integer (milliseconds)
      // Default: 5000 (5 seconds)
      // Range: 1000-60000
      // Description: Maximum time to wait for graceful server shutdown.
      //              After timeout, server process is forcefully killed.

      "request_timeout_ms": 30000,
      // Type: integer (milliseconds)
      // Default: 30000 (30 seconds)
      // Range: 1000-300000
      // Description: Timeout for individual LSP requests (definition, references, etc.)
      // Tuning:
      //   - Complex queries (workspace/symbol, references): 60000-120000
      //   - Simple queries (hover, definition): 10000-30000
      //   - Slow servers or large codebases: 60000+

      "health_check_interval_ms": 30000,
      // Type: integer (milliseconds)
      // Default: 30000 (30 seconds)
      // Minimum: 30000 (rate-limited internally)
      // Description: Interval between automatic health checks.
      //              Health checks verify server is still responsive.
      // Note: Actual checks may be less frequent due to rate limiting.

      // ========================================
      // Buffer & Performance Settings
      // ========================================

      "notification_buffer_size": 100
      // Type: integer
      // Default: 100
      // Range: 10-1000
      // Description: Buffer size for async notification channel.
      //              Notifications include diagnostics, progress, etc.
      // Tuning:
      //   - High-frequency updates: 200-500
      //   - Memory-constrained: 50-100
      //   - Most cases: keep default (100)
    }
  }
}
```

### Configuration Options Summary Table

| Option | Type | Default | Required | Description |
|--------|------|---------|----------|-------------|
| **Server Control** |
| `disabled` | bool | `false` | No | Disable this server |
| `command` | string | - | Custom only | Server executable command |
| `args` | string[] | `[]` | No | Command-line arguments |
| `file_extensions` | string[] | `[]` | Custom only | File extensions to handle |
| `languages` | string[] | `[]` | No | Language identifiers |
| `env` | object | `{}` | No | Environment variables |
| **LSP Protocol** |
| `initialization_options` | any | `null` | No | LSP init options (server-specific) |
| `settings` | any | `null` | No | Workspace settings (server-specific) |
| `workspace_folder` | string | auto | No | Override workspace folder |
| **Lifecycle** |
| `max_restarts` | int | `3` | No | Max restart attempts |
| `restart_on_crash` | bool | `true` | No | Auto-restart on crash |
| **Timeouts** |
| `startup_timeout_ms` | int | `10000` | No | Init timeout (ms) |
| `shutdown_timeout_ms` | int | `5000` | No | Shutdown timeout (ms) |
| `request_timeout_ms` | int | `30000` | No | Request timeout (ms) |
| `health_check_interval_ms` | int | `30000` | No | Health check interval (ms) |
| **Performance** |
| `notification_buffer_size` | int | `100` | No | Notification buffer size |

---

## Built-in Server Specific Configurations

### rust-analyzer

**Executable**: `rust-analyzer`
**Extensions**: `.rs`
**Install**: `rustup component add rust-analyzer`

**Complete Configuration with All Parameters (using defaults):**

```json
{
  "servers": {
    "rust-analyzer": {
      // =============================================
      // Codex LSP Client Parameters (all optional)
      // =============================================
      "disabled": false,                    // Default: false
      "command": null,                      // Default: null (uses "rust-analyzer")
      "args": [],                           // Default: []
      "file_extensions": [],                // Default: [] (uses [".rs"])
      "languages": [],                      // Default: [] (uses ["rust"])
      "env": {},                            // Default: {}
      "workspace_folder": null,             // Default: null (auto-detected)
      "max_restarts": 3,                    // Default: 3
      "restart_on_crash": true,             // Default: true
      "startup_timeout_ms": 10000,          // Default: 10000
      "shutdown_timeout_ms": 5000,          // Default: 5000
      "request_timeout_ms": 30000,          // Default: 30000
      "health_check_interval_ms": 30000,    // Default: 30000
      "notification_buffer_size": 100,      // Default: 100
      "settings": null,                     // Default: null

      // =============================================
      // rust-analyzer Specific: initialization_options
      // =============================================
      "initialization_options": {
        // --- Check on Save ---
        "checkOnSave": {
          "enable": true,                   // Default: true
          "command": "check",               // Default: "check", Options: "check", "clippy"
          "extraArgs": [],                  // Default: []
          "extraEnv": {},                   // Default: {}
          "features": null,                 // Default: null (use cargo default)
          "allTargets": true,               // Default: true
          "targets": null                   // Default: null
        },

        // --- Cargo Settings ---
        "cargo": {
          "autoreload": true,               // Default: true
          "buildScripts": {
            "enable": true,                 // Default: true
            "invocationStrategy": "per_workspace",  // Default: "per_workspace"
            "invocationLocation": "workspace",      // Default: "workspace"
            "overrideCommand": null,        // Default: null
            "rebuildOnSave": true,          // Default: true
            "useRustcWrapper": true         // Default: true
          },
          "cfgs": {},                       // Default: {}
          "extraArgs": [],                  // Default: []
          "extraEnv": {},                   // Default: {}
          "features": [],                   // Default: [] (empty = default features)
          "allFeatures": false,             // Default: false (set true for all features)
          "noDefaultFeatures": false,       // Default: false
          "sysroot": "discover",            // Default: "discover"
          "sysrootSrc": null,               // Default: null
          "target": null,                   // Default: null (host target)
          "targetDir": null                 // Default: null
        },

        // --- Proc Macro Support ---
        "procMacro": {
          "enable": true,                   // Default: true
          "server": null,                   // Default: null (bundled)
          "attributes": {
            "enable": true                  // Default: true
          },
          "ignored": {}                     // Default: {} (no ignored macros)
        },

        // --- Rust Files Discovery ---
        "files": {
          "excludeDirs": [],                // Default: []
          "watcher": "client"               // Default: "client"
        },

        // --- Inlay Hints ---
        "inlayHints": {
          "bindingModeHints": {
            "enable": false                 // Default: false
          },
          "chainingHints": {
            "enable": true                  // Default: true
          },
          "closingBraceHints": {
            "enable": true,                 // Default: true
            "minLines": 25                  // Default: 25
          },
          "closureReturnTypeHints": {
            "enable": "never"               // Default: "never", Options: "always", "never", "with_block"
          },
          "closureCaptureHints": {
            "enable": false                 // Default: false
          },
          "discriminantHints": {
            "enable": "never"               // Default: "never", Options: "always", "never", "fieldless"
          },
          "expressionAdjustmentHints": {
            "enable": "never",              // Default: "never"
            "hideOutsideUnsafe": false,     // Default: false
            "mode": "prefix"                // Default: "prefix"
          },
          "implicitDrops": {
            "enable": false                 // Default: false
          },
          "lifetimeElisionHints": {
            "enable": "never",              // Default: "never", Options: "always", "never", "skip_trivial"
            "useParameterNames": false      // Default: false
          },
          "maxLength": 25,                  // Default: 25 (null for unlimited)
          "parameterHints": {
            "enable": true                  // Default: true
          },
          "reborrowHints": {
            "enable": "never"               // Default: "never"
          },
          "renderColons": true,             // Default: true
          "typeHints": {
            "enable": true,                 // Default: true
            "hideClosureInitialization": false,  // Default: false
            "hideNamedConstructor": false   // Default: false
          }
        },

        // --- Diagnostics ---
        "diagnostics": {
          "enable": true,                   // Default: true
          "disabled": [],                   // Default: [] (no disabled diagnostics)
          "enableExperimental": false,      // Default: false
          "warningsAsHint": [],             // Default: []
          "warningsAsInfo": []              // Default: []
        },

        // --- Completion ---
        "completion": {
          "autoimport": {
            "enable": true                  // Default: true
          },
          "autoself": {
            "enable": true                  // Default: true
          },
          "callable": {
            "snippets": "fill_arguments"    // Default: "fill_arguments"
          },
          "fullFunctionSignatures": {
            "enable": false                 // Default: false
          },
          "limit": null,                    // Default: null (no limit)
          "postfix": {
            "enable": true                  // Default: true
          },
          "privateEditable": {
            "enable": false                 // Default: false
          },
          "snippets": {
            "custom": {}                    // Default: {} (built-in snippets)
          }
        },

        // --- Hover ---
        "hover": {
          "actions": {
            "debug": {
              "enable": true                // Default: true
            },
            "enable": true,                 // Default: true
            "gotoTypeDef": {
              "enable": true                // Default: true
            },
            "implementations": {
              "enable": true                // Default: true
            },
            "references": {
              "enable": false               // Default: false
            },
            "run": {
              "enable": true                // Default: true
            }
          },
          "documentation": {
            "enable": true,                 // Default: true
            "keywords": {
              "enable": true                // Default: true
            }
          },
          "links": {
            "enable": true                  // Default: true
          }
        },

        // --- Lens (Code Lens) ---
        "lens": {
          "enable": true,                   // Default: true
          "debug": {
            "enable": true                  // Default: true
          },
          "implementations": {
            "enable": true                  // Default: true
          },
          "references": {
            "adt": {
              "enable": false               // Default: false
            },
            "enumVariant": {
              "enable": false               // Default: false
            },
            "method": {
              "enable": false               // Default: false
            },
            "trait": {
              "enable": false               // Default: false
            }
          },
          "run": {
            "enable": true                  // Default: true
          }
        },

        // --- Rustfmt ---
        "rustfmt": {
          "extraArgs": [],                  // Default: []
          "overrideCommand": null,          // Default: null
          "rangeFormatting": {
            "enable": false                 // Default: false
          }
        },

        // --- Semantic Highlighting ---
        "semanticHighlighting": {
          "doc": {
            "comment": {
              "inject": {
                "enable": true              // Default: true
              }
            }
          },
          "nonStandardTokens": true,        // Default: true
          "operator": {
            "enable": true,                 // Default: true
            "specialization": {
              "enable": false               // Default: false
            }
          },
          "punctuation": {
            "enable": false,                // Default: false
            "separate": {
              "macro": {
                "bang": false               // Default: false
              }
            },
            "specialization": {
              "enable": false               // Default: false
            }
          },
          "strings": {
            "enable": true                  // Default: true
          }
        },

        // --- Workspace ---
        "workspace": {
          "symbol": {
            "search": {
              "kind": "only_types",         // Default: "only_types"
              "limit": 128,                 // Default: 128
              "scope": "workspace"          // Default: "workspace"
            }
          }
        }
      }
    }
  }
}
```

### gopls

**Executable**: `gopls`
**Extensions**: `.go`
**Install**: `go install golang.org/x/tools/gopls@latest`

**Complete Configuration with All Parameters (using defaults):**

```json
{
  "servers": {
    "gopls": {
      // =============================================
      // Codex LSP Client Parameters (all optional)
      // =============================================
      "disabled": false,                    // Default: false
      "command": null,                      // Default: null (uses "gopls")
      "args": [],                           // Default: []
      "file_extensions": [],                // Default: [] (uses [".go"])
      "languages": [],                      // Default: [] (uses ["go"])
      "env": {},                            // Default: {}
      "workspace_folder": null,             // Default: null (auto-detected)
      "max_restarts": 3,                    // Default: 3
      "restart_on_crash": true,             // Default: true
      "startup_timeout_ms": 10000,          // Default: 10000
      "shutdown_timeout_ms": 5000,          // Default: 5000
      "request_timeout_ms": 30000,          // Default: 30000
      "health_check_interval_ms": 30000,    // Default: 30000
      "notification_buffer_size": 100,      // Default: 100
      "settings": null,                     // Default: null

      // =============================================
      // gopls Specific: initialization_options
      // =============================================
      "initialization_options": {
        // --- Build ---
        "buildFlags": [],                   // Default: [] (extra flags for go build)
        "env": {},                          // Default: {} (extra env vars)
        "directoryFilters": [],             // Default: [] (e.g., ["-node_modules", "+vendor"])
        "templateExtensions": [],           // Default: [] (e.g., [".tmpl", ".gotmpl"])
        "memoryMode": "",                   // Default: "" (Options: "", "DegradeClosed")
        "expandWorkspaceToModule": true,    // Default: true
        "standaloneTags": ["ignore"],       // Default: ["ignore"]

        // --- Formatting ---
        "local": "",                        // Default: "" (local import prefix for grouping)
        "gofumpt": false,                   // Default: false (use gofumpt instead of gofmt)

        // --- UI ---
        "codelenses": {
          "gc_details": false,              // Default: false (show GC optimization details)
          "generate": true,                 // Default: true (run go generate)
          "regenerate_cgo": true,           // Default: true (regenerate cgo)
          "run_govulncheck": false,         // Default: false (run govulncheck)
          "test": true,                     // Default: true (run tests)
          "tidy": true,                     // Default: true (run go mod tidy)
          "upgrade_dependency": true,       // Default: true (upgrade dependency)
          "vendor": true                    // Default: true (run go mod vendor)
        },
        "semanticTokens": false,            // Default: false
        "noSemanticString": false,          // Default: false
        "noSemanticNumber": false,          // Default: false

        // --- Completion ---
        "usePlaceholders": false,           // Default: false (placeholders in completions)
        "completionBudget": "100ms",        // Default: "100ms"
        "matcher": "Fuzzy",                 // Default: "Fuzzy", Options: "Fuzzy", "CaseSensitive", "CaseInsensitive"
        "experimentalPostfixCompletions": true,  // Default: true
        "completeFunctionCalls": true,      // Default: true

        // --- Diagnostic ---
        "analyses": {
          "appends": true,                  // Default: true
          "asmdecl": true,                  // Default: true
          "assign": true,                   // Default: true
          "atomic": true,                   // Default: true
          "atomicalign": true,              // Default: true
          "bools": true,                    // Default: true
          "buildtag": true,                 // Default: true
          "cgocall": true,                  // Default: true
          "composites": true,               // Default: true
          "copylocks": true,                // Default: true
          "deepequalerrors": true,          // Default: true
          "defers": true,                   // Default: true
          "deprecated": true,               // Default: true
          "directive": true,                // Default: true
          "embed": true,                    // Default: true
          "errorsas": true,                 // Default: true
          "fieldalignment": false,          // Default: false (memory alignment suggestions)
          "fillreturns": true,              // Default: true
          "fillstruct": true,               // Default: true
          "httpresponse": true,             // Default: true
          "ifaceassert": true,              // Default: true
          "infertypeargs": true,            // Default: true
          "loopclosure": true,              // Default: true
          "lostcancel": true,               // Default: true
          "nilfunc": true,                  // Default: true
          "nilness": true,                  // Default: true
          "nonewvars": true,                // Default: true
          "noresultvalues": true,           // Default: true
          "printf": true,                   // Default: true
          "shadow": false,                  // Default: false (variable shadowing)
          "shift": true,                    // Default: true
          "simplifycompositelit": true,     // Default: true
          "simplifyrange": true,            // Default: true
          "simplifyslice": true,            // Default: true
          "slog": true,                     // Default: true
          "sortslice": true,                // Default: true
          "stdmethods": true,               // Default: true
          "stdversion": true,               // Default: true
          "stringintconv": true,            // Default: true
          "structtag": true,                // Default: true
          "stubmethods": true,              // Default: true
          "testinggoroutine": true,         // Default: true
          "tests": true,                    // Default: true
          "timeformat": true,               // Default: true
          "undeclaredname": true,           // Default: true
          "unmarshal": true,                // Default: true
          "unreachable": true,              // Default: true
          "unsafeptr": true,                // Default: true
          "unusedparams": false,            // Default: false (unused parameters)
          "unusedresult": true,             // Default: true
          "unusedvariable": false,          // Default: false
          "unusedwrite": true,              // Default: true
          "useany": true                    // Default: true
        },
        "staticcheck": false,               // Default: false (enable staticcheck)
        "annotations": {
          "bounds": true,                   // Default: true
          "escape": true,                   // Default: true
          "inline": true,                   // Default: true
          "nil": true                       // Default: true
        },
        "vulncheck": "Off",                 // Default: "Off", Options: "Off", "Imports", "Govulncheck"
        "diagnosticsDelay": "1s",           // Default: "1s"
        "diagnosticsTrigger": "Edit",       // Default: "Edit", Options: "Edit", "Save"

        // --- Documentation ---
        "hoverKind": "FullDocumentation",   // Default: "FullDocumentation"
        "linkTarget": "pkg.go.dev",         // Default: "pkg.go.dev"
        "linksInHover": true,               // Default: true

        // --- Inlay Hints ---
        "hints": {
          "assignVariableTypes": false,     // Default: false
          "compositeLiteralFields": false,  // Default: false
          "compositeLiteralTypes": false,   // Default: false
          "constantValues": false,          // Default: false
          "functionTypeParameters": false,  // Default: false
          "parameterNames": false,          // Default: false
          "rangeVariableTypes": false       // Default: false
        },

        // --- Navigation ---
        "importShortcut": "Both",           // Default: "Both", Options: "Both", "Definition", "Link"
        "symbolMatcher": "FastFuzzy",       // Default: "FastFuzzy"
        "symbolStyle": "Dynamic",           // Default: "Dynamic", Options: "Dynamic", "Full", "Package"
        "symbolScope": "all",               // Default: "all", Options: "all", "workspace"

        // --- Verbosity ---
        "verboseOutput": false              // Default: false
      }
    }
  }
}
```

### pyright

**Executable**: `pyright-langserver --stdio`
**Extensions**: `.py`, `.pyi`
**Install**: `npm install -g pyright`

**Complete Configuration with All Parameters (using defaults):**

```json
{
  "servers": {
    "pyright": {
      // =============================================
      // Codex LSP Client Parameters (all optional)
      // =============================================
      "disabled": false,                    // Default: false
      "command": null,                      // Default: null (uses "pyright-langserver")
      "args": [],                           // Default: [] (uses ["--stdio"])
      "file_extensions": [],                // Default: [] (uses [".py", ".pyi"])
      "languages": [],                      // Default: [] (uses ["python"])
      "env": {},                            // Default: {}
      "workspace_folder": null,             // Default: null (auto-detected)
      "max_restarts": 3,                    // Default: 3
      "restart_on_crash": true,             // Default: true
      "startup_timeout_ms": 10000,          // Default: 10000
      "shutdown_timeout_ms": 5000,          // Default: 5000
      "request_timeout_ms": 30000,          // Default: 30000
      "health_check_interval_ms": 30000,    // Default: 30000
      "notification_buffer_size": 100,      // Default: 100
      "settings": null,                     // Default: null

      // =============================================
      // pyright Specific: initialization_options
      // =============================================
      "initialization_options": {
        "python": {
          // --- Python Environment ---
          "pythonPath": "",                 // Default: "" (auto-detected)
          "venvPath": "",                   // Default: "" (directory containing venvs)
          "venv": "",                       // Default: "" (venv name)

          // --- Analysis Settings ---
          "analysis": {
            // Type Checking Mode
            "typeCheckingMode": "standard", // Default: "standard"
                                            // Options: "off", "basic", "standard", "strict", "all"

            // Diagnostic Mode
            "diagnosticMode": "openFilesOnly",  // Default: "openFilesOnly"
                                                // Options: "openFilesOnly", "workspace"

            // Path Configuration
            "extraPaths": [],               // Default: [] (additional import search paths)
            "autoSearchPaths": true,        // Default: true
            "stubPath": "",                 // Default: "" (path to stub files)
            "typeshedPaths": [],            // Default: [] (custom typeshed paths)

            // Import Resolution
            "autoImportCompletions": true,  // Default: true
            "useLibraryCodeForTypes": true, // Default: true
            "indexing": true,               // Default: true
            "packageIndexDepths": [],       // Default: []
            "logLevel": "Information",      // Default: "Information"
                                            // Options: "Error", "Warning", "Information", "Trace"

            // Inlay Hints
            "inlayHints": {
              "variableTypes": true,        // Default: true
              "functionReturnTypes": true,  // Default: true
              "callArgumentNames": true,    // Default: true
              "pytestParameters": true      // Default: true
            },

            // --- Diagnostic Severity Overrides ---
            // Each can be: "none", "information", "warning", "error"
            "diagnosticSeverityOverrides": {
              // General Type Issues
              "reportGeneralTypeIssues": "error",           // Default: "error"
              "reportPropertyTypeMismatch": "none",         // Default: "none"
              "reportFunctionMemberAccess": "error",        // Default: "error"

              // Missing/Undefined
              "reportMissingImports": "error",              // Default: "error"
              "reportMissingModuleSource": "warning",       // Default: "warning"
              "reportMissingTypeStubs": "none",             // Default: "none"
              "reportUndefinedVariable": "error",           // Default: "error"

              // Unused
              "reportUnusedImport": "none",                 // Default: "none"
              "reportUnusedClass": "none",                  // Default: "none"
              "reportUnusedFunction": "none",               // Default: "none"
              "reportUnusedVariable": "none",               // Default: "none"
              "reportUnusedExpression": "none",             // Default: "none"

              // Type Annotations
              "reportMissingTypeArgument": "none",          // Default: "none"
              "reportInvalidTypeForm": "error",             // Default: "error"
              "reportInvalidTypeVarUse": "warning",         // Default: "warning"
              "reportUnknownParameterType": "none",         // Default: "none"
              "reportUnknownArgumentType": "none",          // Default: "none"
              "reportUnknownLambdaType": "none",            // Default: "none"
              "reportUnknownVariableType": "none",          // Default: "none"
              "reportUnknownMemberType": "none",            // Default: "none"
              "reportMissingParameterType": "none",         // Default: "none"
              "reportMissingTypeArgument": "none",          // Default: "none"

              // Arguments & Calls
              "reportArgumentType": "error",                // Default: "error"
              "reportCallIssue": "error",                   // Default: "error"
              "reportIndexIssue": "error",                  // Default: "error"
              "reportReturnType": "error",                  // Default: "error"
              "reportAssignmentType": "error",              // Default: "error"

              // Optional/None Handling
              "reportOptionalSubscript": "error",           // Default: "error"
              "reportOptionalMemberAccess": "error",        // Default: "error"
              "reportOptionalCall": "error",                // Default: "error"
              "reportOptionalIterable": "error",            // Default: "error"
              "reportOptionalContextManager": "error",      // Default: "error"
              "reportOptionalOperand": "error",             // Default: "error"

              // Type Compatibility
              "reportIncompatibleMethodOverride": "error",  // Default: "error"
              "reportIncompatibleVariableOverride": "error",// Default: "error"
              "reportInconsistentConstructor": "none",      // Default: "none"
              "reportOverlappingOverload": "error",         // Default: "error"

              // Assertions & Casts
              "reportAssertAlwaysTrue": "warning",          // Default: "warning"
              "reportUnnecessaryIsInstance": "none",        // Default: "none"
              "reportUnnecessaryCast": "none",              // Default: "none"
              "reportUnnecessaryComparison": "none",        // Default: "none"
              "reportUnnecessaryContains": "none",          // Default: "none"

              // Misc
              "reportPrivateUsage": "none",                 // Default: "none"
              "reportPrivateImportUsage": "error",          // Default: "error"
              "reportConstantRedefinition": "none",         // Default: "none"
              "reportDeprecated": "none",                   // Default: "none"
              "reportInvalidStringEscapeSequence": "warning",// Default: "warning"
              "reportInvalidStubStatement": "none",         // Default: "none"
              "reportIncompleteStub": "none",               // Default: "none"
              "reportUnsupportedDunderAll": "warning",      // Default: "warning"
              "reportUnusedCallResult": "none",             // Default: "none"
              "reportUnusedCoroutine": "error",             // Default: "error"
              "reportUntypedFunctionDecorator": "none",     // Default: "none"
              "reportUntypedClassDecorator": "none",        // Default: "none"
              "reportUntypedBaseClass": "none",             // Default: "none"
              "reportUntypedNamedTuple": "none",            // Default: "none"
              "reportSelfClsParameterName": "warning",      // Default: "warning"
              "reportImplicitStringConcatenation": "none",  // Default: "none"
              "reportMatchNotExhaustive": "none",           // Default: "none"
              "reportShadowedImports": "none",              // Default: "none"
              "reportImplicitOverride": "none"              // Default: "none"
            }
          }
        }
      }
    }
  }
}
```

### typescript-language-server

**Executable**: `typescript-language-server --stdio`
**Extensions**: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`
**Install**: `npm install -g typescript-language-server typescript`

**Complete Configuration with All Parameters (using defaults):**

```json
{
  "servers": {
    "typescript-language-server": {
      // =============================================
      // Codex LSP Client Parameters (all optional)
      // =============================================
      "disabled": false,                    // Default: false
      "command": null,                      // Default: null (uses "typescript-language-server")
      "args": [],                           // Default: [] (uses ["--stdio"])
      "file_extensions": [],                // Default: [] (uses [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"])
      "languages": [],                      // Default: [] (uses ["typescript", "typescriptreact", "javascript", "javascriptreact"])
      "env": {},                            // Default: {}
      "workspace_folder": null,             // Default: null (auto-detected)
      "max_restarts": 3,                    // Default: 3
      "restart_on_crash": true,             // Default: true
      "startup_timeout_ms": 10000,          // Default: 10000
      "shutdown_timeout_ms": 5000,          // Default: 5000
      "request_timeout_ms": 30000,          // Default: 30000
      "health_check_interval_ms": 30000,    // Default: 30000
      "notification_buffer_size": 100,      // Default: 100
      "settings": null,                     // Default: null

      // =============================================
      // typescript-language-server Specific: initialization_options
      // =============================================
      "initialization_options": {
        // --- Host Info ---
        "hostInfo": "codex-lsp",            // Default: null (client name)

        // --- TSServer Options ---
        "tsserver": {
          "logDirectory": "",               // Default: "" (no logging)
          "logVerbosity": "off",            // Default: "off", Options: "off", "terse", "normal", "verbose"
          "trace": "off",                   // Default: "off", Options: "off", "messages", "verbose"
          "path": "",                       // Default: "" (bundled tsserver)
          "useSyntaxServer": "auto",        // Default: "auto", Options: "auto", "always", "never"
          "fallbackPath": ""                // Default: "" (fallback tsserver path)
        },

        // --- Preferences (Editor Settings) ---
        "preferences": {
          // Inlay Hints
          "includeInlayParameterNameHints": "none",           // Default: "none", Options: "none", "literals", "all"
          "includeInlayParameterNameHintsWhenArgumentMatchesName": false,  // Default: false
          "includeInlayFunctionParameterTypeHints": false,    // Default: false
          "includeInlayVariableTypeHints": false,             // Default: false
          "includeInlayVariableTypeHintsWhenTypeMatchesName": false,       // Default: false
          "includeInlayPropertyDeclarationTypeHints": false,  // Default: false
          "includeInlayFunctionLikeReturnTypeHints": false,   // Default: false
          "includeInlayEnumMemberValueHints": false,          // Default: false

          // Import Organization
          "importModuleSpecifierPreference": "shortest",      // Default: "shortest", Options: "shortest", "project-relative", "relative", "non-relative"
          "importModuleSpecifierEnding": "auto",              // Default: "auto", Options: "auto", "minimal", "index", "js"
          "allowIncompleteCompletions": true,                 // Default: true
          "allowRenameOfImportPath": true,                    // Default: true

          // Code Actions
          "providePrefixAndSuffixTextForRename": true,        // Default: true
          "allowTextChangesInNewFiles": true,                 // Default: true
          "generateReturnInDocTemplate": true,                // Default: true

          // Quote Style
          "quotePreference": "auto",                          // Default: "auto", Options: "auto", "single", "double"

          // JSX
          "jsxAttributeCompletionStyle": "auto",              // Default: "auto", Options: "auto", "braces", "none"

          // Organize Imports
          "organizeImportsIgnoreCase": "auto",                // Default: "auto"
          "organizeImportsCollation": "ordinal",              // Default: "ordinal", Options: "ordinal", "unicode"
          "organizeImportsCollationLocale": "en",             // Default: "en"

          // Auto Import
          "autoImportFileExcludePatterns": [],                // Default: []
          "preferTypeOnlyAutoImports": false,                 // Default: false

          // Completions
          "useLabelDetailsInCompletionEntries": true,         // Default: true
          "includeAutomaticOptionalChainCompletions": true,   // Default: true
          "includeCompletionsForModuleExports": true,         // Default: true
          "includeCompletionsForImportStatements": true,      // Default: true
          "includeCompletionsWithInsertText": true,           // Default: true
          "includeCompletionsWithClassMemberSnippets": true,  // Default: true
          "includeCompletionsWithObjectLiteralMethodSnippets": true,  // Default: true

          // Display
          "displayPartsForJSDoc": true,                       // Default: true
          "disableLineTextInReferences": false                // Default: false
        },

        // --- Maximum TS Server Memory ---
        "maxTsServerMemory": 4096,          // Default: 4096 (MB)

        // --- Plugins ---
        "plugins": [],                      // Default: [] (e.g., [{"name": "@styled/typescript-styled-plugin"}])

        // --- Completion Disable Features ---
        "completionDisableFilterText": false,  // Default: false
        "disableAutomaticTypingAcquisition": false  // Default: false
      }
    }
  }
}
```

---

## Custom Server Examples

### C/C++ (clangd)

```bash
# Mac
brew install llvm

# Linux (Ubuntu/Debian)
sudo apt install clangd

# Linux (Fedora)
sudo dnf install clang-tools-extra
```

```json
{
  "servers": {
    "clangd": {
      "command": "clangd",
      "args": [
        "--background-index",
        "--clang-tidy",
        "--completion-style=detailed",
        "--header-insertion=iwyu",
        "-j=4"
      ],
      "file_extensions": [".c", ".cpp", ".cc", ".cxx", ".h", ".hpp", ".hxx"],
      "languages": ["c", "cpp"],
      "initialization_options": {
        "clangdFileStatus": true
      }
    }
  }
}
```

### Java (jdtls)

```bash
# Download Eclipse JDT Language Server
# https://download.eclipse.org/jdtls/snapshots/
```

```json
{
  "servers": {
    "jdtls": {
      "command": "jdtls",
      "args": ["-data", "/path/to/workspace"],
      "file_extensions": [".java"],
      "languages": ["java"],
      "initialization_options": {
        "settings": {
          "java": {
            "home": "/path/to/jdk",
            "configuration": {
              "runtimes": [
                {"name": "JavaSE-17", "path": "/path/to/jdk-17"}
              ]
            }
          }
        }
      }
    }
  }
}
```

### Lua (lua-language-server)

```bash
# Mac
brew install lua-language-server

# Linux
# Download from https://github.com/LuaLS/lua-language-server/releases
```

```json
{
  "servers": {
    "lua": {
      "command": "lua-language-server",
      "args": [],
      "file_extensions": [".lua"],
      "languages": ["lua"],
      "initialization_options": {
        "settings": {
          "Lua": {
            "runtime": {"version": "LuaJIT"},
            "diagnostics": {"globals": ["vim"]},
            "workspace": {"library": ["/path/to/neovim/runtime"]}
          }
        }
      }
    }
  }
}
```

---

### Override Built-in Server Settings

```bash
cat > ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "rust-analyzer": {
      "initialization_options": {
        "checkOnSave": { "command": "clippy" }
      },
      "max_restarts": 5,
      "startup_timeout_ms": 15000
    }
  }
}
EOF
```

### Disable a Built-in Server

```bash
cat > ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "gopls": {
      "disabled": true
    }
  }
}
EOF
```

### Add Custom Server (TypeScript)

```bash
# First install the server
npm install -g typescript-language-server typescript

# Then configure
cat > ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "typescript": {
      "command": "typescript-language-server",
      "args": ["--stdio"],
      "file_extensions": [".ts", ".tsx", ".js", ".jsx"],
      "languages": ["typescript", "javascript"]
    }
  }
}
EOF
```

### Add Custom Server (clangd for C/C++)

```bash
# Mac
brew install llvm

# Linux (Ubuntu/Debian)
sudo apt install clangd

# Configure
cat > ~/.codex/lsp_servers.json << 'EOF'
{
  "servers": {
    "clangd": {
      "command": "clangd",
      "args": ["--background-index"],
      "file_extensions": [".c", ".cpp", ".cc", ".h", ".hpp"],
      "languages": ["c", "cpp"]
    }
  }
}
EOF
```

## Step 4: Key Parameters Reference

### Timeout Parameters

| Parameter | Default | Recommended Range | Description |
|-----------|---------|-------------------|-------------|
| `startup_timeout_ms` | 10000 | 5000-30000 | Time to wait for server initialization |
| `shutdown_timeout_ms` | 5000 | 3000-10000 | Time to wait for graceful shutdown |
| `request_timeout_ms` | 30000 | 10000-60000 | Timeout for individual LSP requests |
| `health_check_interval_ms` | 30000 | 30000-60000 | Interval between health checks |

**Tuning Tips:**
- Increase `startup_timeout_ms` for large workspaces (rust-analyzer may take 15-30s)
- Increase `request_timeout_ms` for complex operations (references, workspace search)

### Restart Parameters

| Parameter | Default | Recommended Range | Description |
|-----------|---------|-------------------|-------------|
| `max_restarts` | 3 | 3-10 | Max restart attempts before giving up |
| `restart_on_crash` | true | - | Auto-restart when server crashes |

**Tuning Tips:**
- Set `max_restarts: 5-10` for unstable environments
- Set `restart_on_crash: false` to debug server issues

### Buffer Parameters

| Parameter | Default | Recommended Range | Description |
|-----------|---------|-------------------|-------------|
| `notification_buffer_size` | 100 | 50-500 | Buffer size for async notifications |

## Step 5: Verify Installation

### Check LSP Server Binaries

```bash
# All servers
echo "=== LSP Server Check ==="
echo "rust-analyzer: $(which rust-analyzer 2>/dev/null || echo 'NOT FOUND')"
echo "gopls: $(which gopls 2>/dev/null || echo 'NOT FOUND')"
echo "pyright: $(which pyright-langserver 2>/dev/null || echo 'NOT FOUND')"
```

### Check Configuration File

```bash
# Check if config exists
ls -la ~/.codex/lsp_servers.json 2>/dev/null || echo "Global config not found"
ls -la .codex/lsp_servers.json 2>/dev/null || echo "Project config not found"

# Validate JSON syntax
cat ~/.codex/lsp_servers.json 2>/dev/null | python3 -m json.tool > /dev/null && echo "Config is valid JSON" || echo "Config has JSON errors"
```

## Step 6: Common Configurations

### Performance-Optimized (Large Codebase)

```json
{
  "servers": {
    "rust-analyzer": {
      "initialization_options": {
        "cargo": { "buildScripts": { "enable": false } },
        "procMacro": { "enable": false }
      },
      "startup_timeout_ms": 60000,
      "request_timeout_ms": 60000,
      "max_restarts": 5
    }
  }
}
```

### Development with Clippy

```json
{
  "servers": {
    "rust-analyzer": {
      "initialization_options": {
        "checkOnSave": { "command": "clippy" },
        "cargo": { "features": "all" }
      }
    }
  }
}
```

### Go with Custom GOPATH

```json
{
  "servers": {
    "gopls": {
      "env": {
        "GOPATH": "/custom/gopath"
      },
      "initialization_options": {
        "staticcheck": true
      }
    }
  }
}
```

### Python with Virtual Environment

```json
{
  "servers": {
    "pyright": {
      "env": {
        "VIRTUAL_ENV": "/path/to/venv"
      },
      "initialization_options": {
        "python": {
          "pythonPath": "/path/to/venv/bin/python"
        }
      }
    }
  }
}
```

## Troubleshooting

### Server Not Found

```
Error: ServerNotFound - LSP binary not in PATH
```

**Solution:**
```bash
# Check if binary is installed
which rust-analyzer

# If using rustup, ensure it's in PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Reload shell
source ~/.bashrc  # or ~/.zshrc
```

### Initialization Timeout

```
Error: InitializationTimeout - Server took too long to initialize
```

**Solution:**
```json
{
  "servers": {
    "rust-analyzer": {
      "startup_timeout_ms": 30000
    }
  }
}
```

### Server Keeps Crashing

```
Error: ServerFailed - Crashed after max restart attempts
```

**Solution:**
1. Increase `max_restarts`
2. Check server logs
3. Update the LSP server to latest version

```bash
# Update rust-analyzer
rustup update

# Update gopls
go install golang.org/x/tools/gopls@latest

# Update pyright
npm update -g pyright
```

### Request Timeout

```
Error: RequestTimeout - Request exceeded timeout
```

**Solution:**
```json
{
  "servers": {
    "rust-analyzer": {
      "request_timeout_ms": 60000
    }
  }
}
```

### Permission Issues (Mac)

```bash
# If LSP server can't access files
xattr -d com.apple.quarantine $(which rust-analyzer)
```

### Debug Logging

Enable debug logging to troubleshoot issues:

```bash
# Set RUST_LOG for detailed logs
export RUST_LOG=codex_lsp=debug

# Run your application
```

## Complete Example Configuration

```json
{
  "servers": {
    "rust-analyzer": {
      "initialization_options": {
        "checkOnSave": { "command": "clippy" },
        "cargo": { "features": "all" }
      },
      "max_restarts": 5,
      "startup_timeout_ms": 30000,
      "request_timeout_ms": 45000
    },
    "gopls": {
      "initialization_options": {
        "staticcheck": true,
        "gofumpt": true
      }
    },
    "pyright": {
      "initialization_options": {
        "python": {
          "analysis": {
            "typeCheckingMode": "strict"
          }
        }
      }
    },
    "typescript": {
      "command": "typescript-language-server",
      "args": ["--stdio"],
      "file_extensions": [".ts", ".tsx", ".js", ".jsx"],
      "languages": ["typescript", "javascript"],
      "max_restarts": 3
    }
  }
}
```

## Quick Reference

### Built-in Servers

| Server | Extensions | Binary | Install |
|--------|------------|--------|---------|
| rust-analyzer | `.rs` | `rust-analyzer` | `rustup component add rust-analyzer` |
| gopls | `.go` | `gopls` | `go install golang.org/x/tools/gopls@latest` |
| pyright | `.py`, `.pyi` | `pyright-langserver` | `npm install -g pyright` |
| typescript-language-server | `.ts`, `.tsx`, `.js`, `.jsx` | `typescript-language-server` | `npm install -g typescript-language-server typescript` |

### Config Priority

1. `.codex/lsp_servers.json` (project, highest priority)
2. `~/.codex/lsp_servers.json` (global)
3. Built-in defaults (lowest priority)

### Timeout Defaults

| Parameter | Default |
|-----------|---------|
| `startup_timeout_ms` | 10,000 ms (10s) |
| `shutdown_timeout_ms` | 5,000 ms (5s) |
| `request_timeout_ms` | 30,000 ms (30s) |
| `health_check_interval_ms` | 30,000 ms (30s) |
