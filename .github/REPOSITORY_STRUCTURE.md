# Codex Repository Structure / リポジトリ構造

**Version**: 0.48.0  
**Last Updated**: 2025-10-15

This document describes the repository structure for the zapabob/codex fork, which maintains compatibility with the official OpenAI/codex repository while adding enhanced features.

このドキュメントは、OpenAI/codex公式リポジトリとの互換性を維持しながら、拡張機能を追加したzapabob/codexフォークのリポジトリ構造を説明します。

---

## 🏗️ Repository Structure / リポジトリ構造

### Official OpenAI/codex Compatible Structure / 公式互換構造

```
codex/
├── .codex/              # Agent definitions (with zapabob extensions)
│   ├── agents/          # YAML agent definitions
│   ├── policies/        # Security policies
│   ├── prompts/         # System prompts
│   └── scripts/         # Automation scripts
│
├── .cursor/             # Cursor IDE configuration
│   ├── mcp.json         # MCP server configuration
│   ├── rules.md         # Project rules
│   └── settings.json    # IDE settings
│
├── .devcontainer/       # Development container config
│
├── .github/             # GitHub configuration
│   ├── assets/          # Images and logos
│   ├── workflows/       # GitHub Actions
│   └── ISSUE_TEMPLATE/  # Issue templates
│
├── codex-rs/            # Rust core implementation
│   ├── cli/             # Command-line interface
│   ├── core/            # Core runtime
│   ├── tui/             # Terminal user interface
│   ├── mcp-server/      # MCP server (zapabob)
│   ├── supervisor/      # Sub-agent management (zapabob)
│   ├── deep-research/   # Deep research engine (zapabob)
│   └── ...              # Other modules
│
├── codex-cli/           # Node.js CLI (official)
│
├── docs/                # Documentation (official + zapabob)
│   ├── architecture/    # Architecture docs
│   ├── guides/          # User guides
│   ├── auto-orchestration.md        # zapabob
│   ├── codex-subagents-deep-research.md  # zapabob
│   └── ...
│
├── examples/            # Example code
│
├── scripts/             # Official build scripts
│
├── completions/         # Shell completion scripts
│
└── (Root configuration files)
    ├── Cargo.toml
    ├── package.json
    ├── README.md
    ├── LICENSE
    └── VERSION
```

### zapabob-Specific Extensions / zapabob独自拡張

```
zapabob/                 # zapabob-specific extensions
├── docs/                # Additional documentation
│   ├── SNS宣伝文_自律オーケストレーション.md
│   ├── PR作成手順_OpenAI.md
│   ├── CURSOR_MCP_QUICK_GUIDE.md
│   ├── CODE_REVIEW_REPORT.md
│   └── ...
│
├── scripts/             # Build and test scripts
│   ├── build-with-progress.ps1
│   ├── test-codex-production.ps1
│   ├── test-mcp-server.ps1
│   ├── test-mcp-jsonrpc.py
│   └── ...
│
├── extensions/          # IDE extensions
│   ├── vscode-extension/
│   └── windsurf-extension/
│
├── sdk/                 # TypeScript SDK
│   └── typescript/
│
├── reports/             # Code review reports
│
└── README.md            # zapabob directory documentation
```

### Archive / アーカイブ

```
archive/                 # Archived files (not for production)
├── artifacts/           # Build artifacts
├── build-logs/          # Historical build logs
├── old-implementations/ # Deprecated implementations
├── rmcp-versions/       # Old rmcp versions
└── ...                  # Historical documents
```

### Implementation Logs / 実装ログ

```
_docs/                   # Implementation logs (zapabob)
├── 2025-10-15_*.md      # Daily implementation logs
├── build_backups/       # Build checkpoint backups
└── *.png                # Diagrams and charts
```

---

## 📊 Comparison with Official Repository / 公式との比較

### Files Identical to Official / 公式と同一のファイル

| Directory/File | Status | Notes |
|----------------|--------|-------|
| `.github/workflows/` | Same | CI/CD configuration |
| `codex-cli/` | Same | Node.js CLI |
| `LICENSE` | Same | Apache 2.0 |
| `NOTICE` | Same | Legal notices |
| `Cargo.toml` (root) | Same | Workspace definition |
| `package.json` (root) | Same | npm workspace |
| `scripts/` (official) | Same | Build scripts |
| `completions/` | Same | Shell completions |

### zapabob Extensions / zapabob拡張

| Directory/File | Type | Purpose |
|----------------|------|---------|
| `zapabob/` | Directory | All zapabob-specific files |
| `codex-rs/mcp-server/` | Enhanced | Extended MCP server |
| `codex-rs/supervisor/` | New | Sub-agent management |
| `codex-rs/deep-research/` | New | Deep research engine |
| `docs/auto-orchestration.md` | New | Auto-orchestration docs |
| `docs/quickstart-*.md` | New | Quick start guides |
| `_docs/` | New | Implementation logs |
| `archive/` | New | Historical files |

---

## 🔄 Synchronization Strategy / 同期戦略

### Upstream Sync / 上流同期

```bash
# Fetch official changes
git fetch upstream

# Merge official changes
git merge upstream/main

# Resolve conflicts (if any)
# zapabob/ directory should not conflict
```

### Branch Strategy / ブランチ戦略

| Branch | Purpose | Sync with Official |
|--------|---------|-------------------|
| `main` | Production | Regular merge from upstream/main |
| `dev` | Development | Feature development |
| `feature/*` | Features | Individual features |
| `hotfix/*` | Urgent fixes | Critical fixes |

---

## 📦 .gitignore Configuration / .gitignore設定

### Ignored by Default

- `target/` - Rust build artifacts
- `node_modules/` - npm dependencies
- `*.log` - Log files
- `*.exe`, `*.pdb` - Binary files (examples)
- `temp-*`, `*-old-*` - Temporary files

### Tracked for zapabob

- `_docs/` - Implementation logs (tracked)
- `zapabob/` - All zapabob extensions (tracked)
- `.cursor/` - Cursor configuration (tracked)
- `.codex/` - Agent definitions (tracked)

---

## 🎯 File Organization Principles / ファイル整理原則

### 1. **Official Compatibility** / 公式互換性
- Keep official structure unchanged
- zapabob extensions in dedicated directory
- Easy to sync with upstream

### 2. **Clear Separation** / 明確な分離
- Official files: Root level
- zapabob files: `zapabob/` directory
- Historical files: `archive/` directory
- Implementation logs: `_docs/` directory

### 3. **No File Deletion** / ファイル削除なし
- Move to `archive/` instead of delete
- Preserve history and context
- Easy to restore if needed

### 4. **Documentation First** / ドキュメント優先
- README.md for each major directory
- Clear structure explanation
- Both English and Japanese

---

## 🚀 Best Practices / ベストプラクティス

### 1. **Commit Messages** / コミットメッセージ

```bash
# Format
<type>: <subject>

# Examples
feat: Add auto-orchestration to MCP server
fix: Resolve MCP server startup issue
docs: Update README with architecture diagram
refactor: Restructure codex_tools module
test: Add MCP JSONRPC integration tests
```

### 2. **Pull Request Process** / プルリクエスト

1. Create feature branch
2. Implement changes
3. Run tests
4. Update documentation
5. Submit PR with clear description

### 3. **Code Quality** / コード品質

- Run `cargo clippy` before commit
- Run `cargo fmt` for formatting
- Run all tests (`cargo test`)
- Check documentation (`cargo doc`)

---

## 📝 Maintenance / メンテナンス

### Weekly Tasks / 週次タスク

- [ ] Sync with upstream/main
- [ ] Review and merge dependency updates
- [ ] Check for security advisories
- [ ] Update documentation if needed

### Monthly Tasks / 月次タスク

- [ ] Review archive/ directory
- [ ] Clean up old build logs
- [ ] Update version numbers
- [ ] Review and update .gitignore

### Release Tasks / リリースタスク

- [ ] Update VERSION file
- [ ] Update CHANGELOG.md
- [ ] Create release tag
- [ ] Build release binaries
- [ ] Update documentation
- [ ] Announce release

---

## 🔗 References / 参考資料

- [Official OpenAI/codex](https://github.com/openai/codex)
- [GitHub Repository Best Practices](https://docs.github.com/en/repositories/creating-and-managing-repositories/best-practices-for-repositories)
- [Semantic Versioning](https://semver.org/)
- [Conventional Commits](https://www.conventionalcommits.org/)

---

**Maintained by**: zapabob  
**Based on**: OpenAI/codex  
**License**: Apache 2.0

