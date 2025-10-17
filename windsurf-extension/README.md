# Codex Multi-Agent for Windsurf

Windsurf IDE integration for Codex Sub-Agents and Deep Research.

## Features

- **Deep Research** (`Ctrl+Shift+S`): AI-powered research with web search
- **Code Review** (`Ctrl+Shift+R`): Multi-language code review (TS/Python/Rust/Unity)
- **Test Generation** (`Ctrl+Shift+T`): Auto-generate test suites
- **Security Audit** (`Ctrl+Shift+A`): Vulnerability scanning

## Installation

1. Build the extension:
   ```bash
   cd windsurf-extension
   npm install
   npm run compile
   ```

2. Install in Windsurf:
   - Open Windsurf
   - Extensions → Install from VSIX
   - Select `windsurf-extension` directory

## Usage

### Deep Research
1. Press `Ctrl+Shift+S`
2. Enter research topic
3. Select depth (1-5)
4. View generated report in `artifacts/`

### Code Review
1. Open a file
2. Press `Ctrl+Shift+R`
3. View review in output panel

## Configuration

```json
{
  "codex.mcpServerPath": "${workspaceFolder}/codex-rs/mcp-server/dist/index.js",
  "codex.researchDepth": 3,
  "codex.autoReview": true
}
```

## Requirements

- Windsurf IDE v1.0+
- Node.js v18+
- Codex binaries built (`codex-tui`)

## License

MIT

