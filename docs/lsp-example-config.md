# LSP Configuration Guide

This document provides complete configuration examples for the LSP (Language Server Protocol) integration in Codex.

## Configuration File Location

LSP servers are configured via `lsp_servers.json`. The file is loaded from:

1. **User-level**: `~/.codex/lsp_servers.json` (applies to all projects)
2. **Project-level**: `.codex/lsp_servers.json` (in project root, overrides user-level)

Project-level settings take precedence over user-level settings.

## Built-in Servers

Codex includes built-in support for three language servers:

| Server | Languages | File Extensions | Install Command |
|--------|-----------|-----------------|-----------------|
| rust-analyzer | Rust | `.rs` | `rustup component add rust-analyzer` |
| gopls | Go | `.go` | `go install golang.org/x/tools/gopls@latest` |
| pyright | Python | `.py`, `.pyi` | `npm install -g pyright` |

Built-in servers work without any configuration if the server binary is installed and available in PATH.

## Configuration Schema

### LspServerConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `disabled` | boolean | `false` | Disable this server |
| `command` | string | null | Command to execute (required for custom servers) |
| `args` | string[] | `[]` | Command-line arguments |
| `file_extensions` | string[] | `[]` | File extensions this server handles (required for custom) |
| `languages` | string[] | `[]` | Language identifiers |
| `env` | object | `{}` | Environment variables |
| `initialization_options` | object | `null` | LSP initialization options |
| `settings` | object | `null` | Workspace settings (workspace/didChangeConfiguration) |
| `workspace_folder` | string | auto | Explicit workspace folder path |
| `max_restarts` | integer | `3` | Max restart attempts before giving up |
| `restart_on_crash` | boolean | `true` | Auto-restart on crash |
| `startup_timeout_ms` | integer | `10000` | Startup/init timeout (ms) |
| `shutdown_timeout_ms` | integer | `5000` | Shutdown timeout (ms) |
| `request_timeout_ms` | integer | `30000` | Request timeout (ms) |
| `health_check_interval_ms` | integer | `30000` | Health check interval (ms) |

## Complete Example Configuration

```json
{
  "servers": {
    "rust-analyzer": {
      "initialization_options": {
        "checkOnSave": {
          "command": "clippy"
        },
        "cargo": {
          "allFeatures": true,
          "buildScripts": {
            "enable": true
          }
        },
        "procMacro": {
          "enable": true
        },
        "diagnostics": {
          "enable": true,
          "experimental": {
            "enable": true
          }
        }
      },
      "settings": {
        "rust-analyzer": {
          "inlayHints": {
            "chainingHints": true,
            "parameterHints": true,
            "typeHints": true
          }
        }
      },
      "env": {
        "RUST_LOG": "info",
        "CARGO_INCREMENTAL": "1"
      },
      "max_restarts": 5,
      "startup_timeout_ms": 15000,
      "request_timeout_ms": 60000
    },

    "gopls": {
      "initialization_options": {
        "gofumpt": true,
        "staticcheck": true,
        "analyses": {
          "unusedparams": true,
          "shadow": true,
          "nilness": true
        },
        "hints": {
          "assignVariableTypes": true,
          "compositeLiteralFields": true,
          "constantValues": true,
          "functionTypeParameters": true,
          "parameterNames": true,
          "rangeVariableTypes": true
        }
      },
      "settings": {
        "gopls": {
          "completeUnimported": true,
          "usePlaceholders": true,
          "deepCompletion": true
        }
      },
      "env": {
        "GOFLAGS": "-tags=integration"
      },
      "max_restarts": 3,
      "request_timeout_ms": 45000
    },

    "pyright": {
      "initialization_options": {
        "python": {
          "analysis": {
            "typeCheckingMode": "strict",
            "autoSearchPaths": true,
            "useLibraryCodeForTypes": true,
            "diagnosticMode": "workspace"
          }
        }
      },
      "settings": {
        "python": {
          "pythonPath": "/usr/local/bin/python3",
          "analysis": {
            "extraPaths": ["./src", "./lib"]
          }
        }
      },
      "env": {
        "PYTHONPATH": "./src:./lib"
      },
      "max_restarts": 3
    },

    "typescript": {
      "command": "typescript-language-server",
      "args": ["--stdio"],
      "file_extensions": [".ts", ".tsx", ".js", ".jsx"],
      "languages": ["typescript", "typescriptreact", "javascript", "javascriptreact"],
      "initialization_options": {
        "preferences": {
          "includeInlayParameterNameHints": "all",
          "includeInlayVariableTypeHints": true,
          "includeInlayFunctionLikeReturnTypeHints": true
        }
      },
      "settings": {
        "typescript": {
          "inlayHints": {
            "includeInlayParameterNameHints": "all"
          }
        }
      },
      "max_restarts": 3,
      "startup_timeout_ms": 15000
    },

    "clangd": {
      "command": "clangd",
      "args": [
        "--background-index",
        "--clang-tidy",
        "--completion-style=detailed",
        "--header-insertion=iwyu",
        "--suggest-missing-includes"
      ],
      "file_extensions": [".c", ".h", ".cpp", ".hpp", ".cc", ".cxx", ".hxx"],
      "languages": ["c", "cpp", "objective-c", "objective-cpp"],
      "initialization_options": {
        "clangdFileStatus": true
      },
      "env": {
        "CPATH": "/usr/local/include"
      },
      "max_restarts": 3
    },

    "lua-language-server": {
      "command": "lua-language-server",
      "args": [],
      "file_extensions": [".lua"],
      "languages": ["lua"],
      "initialization_options": {
        "settings": {
          "Lua": {
            "runtime": {
              "version": "LuaJIT"
            },
            "diagnostics": {
              "globals": ["vim"]
            },
            "workspace": {
              "library": []
            }
          }
        }
      },
      "max_restarts": 3
    },

    "zls": {
      "command": "zls",
      "args": [],
      "file_extensions": [".zig"],
      "languages": ["zig"],
      "initialization_options": {
        "enable_snippets": true,
        "enable_ast_check_diagnostics": true,
        "enable_autofix": true,
        "enable_import_embedfile": true,
        "enable_semantic_tokens": true,
        "enable_inlay_hints": true
      },
      "max_restarts": 3
    }
  }
}
```

## Minimal Configuration Examples

### Disable a Built-in Server

```json
{
  "servers": {
    "gopls": {
      "disabled": true
    }
  }
}
```

### Override Built-in Server Settings

```json
{
  "servers": {
    "rust-analyzer": {
      "initialization_options": {
        "checkOnSave": {
          "command": "clippy"
        }
      },
      "max_restarts": 5
    }
  }
}
```

### Add a Custom Server

```json
{
  "servers": {
    "my-custom-lsp": {
      "command": "/path/to/my-lsp",
      "args": ["--stdio", "--debug"],
      "file_extensions": [".xyz", ".abc"],
      "languages": ["mylang"],
      "env": {
        "MY_LSP_DEBUG": "1"
      }
    }
  }
}
```

## Notes

1. **Custom vs Built-in Detection**: A server is considered "custom" if it has a `command` field. Built-in servers use predefined commands.

2. **File Extension Matching**: Custom servers require `file_extensions` to be specified. Built-in servers have predefined extensions.

3. **Merge Behavior**: When both user-level and project-level configs exist, project-level settings completely replace user-level settings for the same server ID.

4. **Server Binary**: Ensure the LSP server binary is installed and available in PATH (or provide absolute path in `command`).

## LSP Tool Operations

The LSP tool supports these operations:

| Operation | Description | Required Fields |
|-----------|-------------|-----------------|
| `goToDefinition` | Find symbol definition | `filePath`, `symbolName` |
| `findReferences` | Find all references | `filePath`, `symbolName` |
| `hover` | Get hover information | `filePath`, `symbolName` |
| `documentSymbol` | List all symbols in file | `filePath` |
| `getDiagnostics` | Get file diagnostics | `filePath` |
| `workspaceSymbol` | Search symbols across workspace | `filePath`*, `symbolName` |
| `goToImplementation` | Find trait/interface implementations | `filePath`, `symbolName` |
| `getCallHierarchy` | Get incoming/outgoing function calls | `filePath`, `symbolName`, `direction` |
| `goToTypeDefinition` | Find the type definition of a symbol | `filePath`, `symbolName` |
| `goToDeclaration` | Find symbol declaration | `filePath`, `symbolName` |

\* For `workspaceSymbol`, `filePath` is used to determine which language server to query. Use any file with the target language extension (e.g., `src/lib.rs` for Rust).

**Note:** For `getCallHierarchy`, the `direction` field must be either `incoming` (functions that call this symbol) or `outgoing` (functions this symbol calls). This operation is not supported for Python files via pyright.

Optional `symbolKind` field helps disambiguate symbols: `function`, `struct`, `class`, `method`, `field`, `variable`, `constant`, `interface`, `enum`, `module`, `property`, `type`.
