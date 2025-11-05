# Contributing to Codex / Codex への貢献

Thank you for your interest in contributing to the zapabob/codex project! This guide will help you get started.

zapabob/codexプロジェクトへの貢献に興味を持っていただき、ありがとうございます！このガイドが始めるお手伝いをします。

---

## 🌟 How to Contribute / 貢献方法

### 1. **Report Issues / 問題を報告**

If you find a bug or have a feature request:

バグを見つけた場合や機能リクエストがある場合：

- Check if the issue already exists
- Create a new issue with a clear description
- Include steps to reproduce (for bugs)
- Provide context and use cases (for features)

### 2. **Submit Pull Requests / プルリクエストを提出**

#### Steps / 手順

1. **Fork the repository** / リポジトリをフォーク
   ```bash
   # Clone your fork
   git clone https://github.com/YOUR_USERNAME/codex.git
   cd codex
   ```

2. **Set up upstream** / 上流を設定
   ```bash
   git remote add upstream https://github.com/zapabob/codex.git
   ```

3. **Create a feature branch** / フィーチャーブランチを作成
   ```bash
   git checkout -b feature/your-feature-name
   ```

4. **Make your changes** / 変更を加える
   - Write clean, well-documented code
   - Follow the project's coding standards
   - Add tests for new features

5. **Test your changes** / 変更をテスト
   ```bash
   # Rust tests
   cd codex-rs
   cargo test
   cargo clippy
   cargo fmt --check
   
   # Integration tests
   cd ..
   .\test-codex-production.ps1
   ```

6. **Commit your changes** / 変更をコミット
   ```bash
   git add .
   git commit -m "feat: Add amazing feature"
   ```

7. **Push to your fork** / フォークにプッシュ
   ```bash
   git push origin feature/your-feature-name
   ```

8. **Open a Pull Request** / プルリクエストを開く
   - Go to GitHub
   - Click "New Pull Request"
   - Provide clear description
   - Link related issues

---

## 📝 Coding Standards / コーディング規約

### Rust

```rust
// ✅ GOOD: Explicit types
fn get_user_by_id(id: u64) -> Result<User, Error> {
    database.find_user(id)
}

// ❌ BAD: No type hints
fn get_user_by_id(id) { ... }
```

**Rules**:
- Follow Clippy lints (all categories)
- Use inline format arguments
- Prefer iterators over loops
- Never use `unsafe` without justification
- Add documentation comments

### TypeScript/JavaScript

```typescript
// ✅ GOOD
async function fetchUser(id: number): Promise<User | null> {
  return await database.findUser(id);
}

// ❌ BAD
function fetchUser(id: any): any { ... }
```

**Rules**:
- Use `const` by default
- Never use `any` type
- Prefer `async/await` over `.then()`
- Use optional chaining (`?.`)

### PowerShell

```powershell
# ✅ GOOD: English, clear names
function Test-CodexFeature {
    param([string]$FeatureName)
    # ...
}

# ❌ BAD: Mixed language, unclear
function テスト {
    # ...
}
```

**Rules**:
- Use English for scripts (avoid encoding issues)
- Proper error handling
- Clear function names
- Comment complex logic

---

## 🧪 Testing Requirements / テスト要件

### Unit Tests / ユニットテスト

All new features must include unit tests:

すべての新機能にはユニットテストが必要です：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_new_feature() {
        // Arrange
        let input = ...;
        
        // Act
        let result = new_feature(input);
        
        // Assert
        assert_eq!(result, expected);
    }
}
```

### Integration Tests / 統合テスト

For MCP server features:

MCPサーバー機能の場合：

```bash
# Run integration tests
.\zapabob\scripts\test-mcp-server.ps1
```

### Coverage Goals / カバレッジ目標

- Unit tests: 80%+
- Integration tests: 100% main flows
- E2E tests: 100% critical paths

---

## 🔐 Security Guidelines / セキュリティガイドライン

### 1. **Never Commit Sensitive Data** / 機密データをコミットしない

- No API keys
- No passwords
- No private keys
- Use `.env.example` for templates

### 2. **Review AI-Generated Code** / AI生成コードのレビュー

Always review:
- Authentication logic
- SQL queries
- File operations
- Shell commands

### 3. **Use Sandbox Mode** / サンドボックスモードを使用

```bash
# Safe execution
codex --sandbox=read-only "task"
```

---

## 📚 Documentation / ドキュメント

### Required Documentation / 必須ドキュメント

When adding new features:

新機能を追加する際：

- [ ] Update README.md
- [ ] Add docstrings/comments
- [ ] Update relevant guides in `docs/`
- [ ] Add examples if applicable
- [ ] Update CHANGELOG.md

### Documentation Style / ドキュメントスタイル

- Use clear, concise language
- Provide code examples
- Include both English and Japanese (if possible)
- Add diagrams for complex features

---

## 🎯 zapabob-Specific Guidelines / zapabob固有のガイドライン

### Where to Place New Files / 新規ファイルの配置

| File Type | Location |
|-----------|----------|
| zapabob documentation | `zapabob/docs/` |
| Build/test scripts | `zapabob/scripts/` |
| IDE extensions | `zapabob/extensions/` |
| SDK code | `zapabob/sdk/` |
| Implementation logs | `_docs/` |
| Archived files | `archive/` |

### Rust Module Organization / Rustモジュール構成

```
codex-rs/
├── Official modules (minimize changes)
└── zapabob modules:
    ├── mcp-server/     # MCP server enhancements
    ├── supervisor/     # Sub-agent management
    └── deep-research/  # Deep research engine
```

---

## 🤝 Community / コミュニティ

### Communication / コミュニケーション

- **GitHub Issues**: Bug reports, feature requests
- **Pull Requests**: Code contributions
- **Discussions**: General questions, ideas

### Code of Conduct / 行動規範

- Be respectful and inclusive
- Provide constructive feedback
- Help others learn and grow
- Follow the project's guidelines

---

## ✅ Checklist Before Submitting PR / PR提出前チェックリスト

- [ ] Code follows project standards
- [ ] All tests pass
- [ ] Documentation updated
- [ ] Commit messages follow convention
- [ ] No merge conflicts
- [ ] PR description is clear
- [ ] Related issues linked

---

## 📧 Contact / 連絡先

For questions or discussions:

質問やディスカッションについては：

- Open a GitHub Issue
- Start a Discussion
- Email: [Your Contact]

---

**Thank you for contributing to Codex!**  
**Codexへの貢献ありがとうございます！**

---

**Version**: 0.48.0  
**Last Updated**: 2025-10-15

