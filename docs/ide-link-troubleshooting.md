# IDE Link Troubleshooting (Windows)

If local file links in the Codex IDE sidebar open in your browser (or fail with a `file+.vscode-resource.vscode-cdn.net/...` URL), use the checks and workarounds below.

This has been reported by multiple users in:

- https://github.com/openai/codex/issues/12661
- https://github.com/openai/codex/issues/12984

## Symptoms

- Clicking a local file link opens Edge/Chrome instead of an editor tab.
- Clicking a link opens a URL like:
  - `https://file+.vscode-resource.vscode-cdn.net/c%3A/...`
- The URL may include `#` and does not open the expected file.

## Quick checks

1. Confirm the issue is in the IDE extension webview (not terminal output links).
2. Confirm the same link opens correctly if pasted as a plain local path in:
   - VS Code/Cursor command palette: `File: Open File...`
3. If this only happens in one old thread, try a new thread to rule out stale webview state.

## Workarounds

### Workaround A: Use plain absolute file paths in responses

When sharing file references, prefer plain absolute paths:

```text
C:\Users\name\project\src\app.ts:42
```

Avoid relying on markdown-style local links in affected builds.

### Workaround B: Decode `file+.vscode-resource...` links and open locally

PowerShell helper:

```powershell
$u = "https://file+.vscode-resource.vscode-cdn.net/c%3A/Users/name/project/src/app.ts#"
$p = [System.Uri]::UnescapeDataString(($u -replace '^https://file\+\.vscode-resource\.vscode-cdn\.net/', '')) `
  -replace '[?#].*$','' `
  -replace '/', '\'
code --goto $p
```

If you use Cursor CLI instead of VS Code CLI:

```powershell
cursor --goto $p
```

## When reporting

Include:

- OS and version
- IDE and version (VS Code/Cursor/Windsurf)
- Extension version (for example `openai.chatgpt 0.5.78`)
- One failing sample URL and expected file path
- Whether the issue reproduces in a brand-new thread
