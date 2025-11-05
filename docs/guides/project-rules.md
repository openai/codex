# 🎯 Codex Project Rules - OpenAI Best Practices + zapabob Extensions

**Version**: 0.47.0-alpha.1  
**Last Updated**: 2025-10-12  
**Based on**: [OpenAI/codex official recommendations](https://github.com/openai/codex) + [community issues](https://github.com/openai/codex/issues)

> 📘 **Full Documentation**: See `.cursor/rules.md` for comprehensive guidelines

---

## 📋 Quick Reference

### OpenAI Official CLI Commands

Based on [OpenAI/codex CLI usage](https://github.com/openai/codex/blob/main/docs/getting-started.md#cli-usage):

| Command | Purpose | Example |
|---------|---------|---------|
| `codex` | Interactive TUI | `codex` |
| `codex "..."` | Initial prompt for TUI | `codex "fix lint errors"` |
| `codex exec "..."` | Non-interactive mode | `codex exec "explain utils.ts"` |
| `codex resume` | Session picker UI | `codex resume` |
| `codex resume --last` | Resume most recent | `codex resume --last` |

**Key flags**: `--model/-m`, `--ask-for-approval/-a`, `--sandbox`

### zapabob Extended Commands

```bash
# Code review
codex delegate code-reviewer --scope ./src

# Parallel execution (3x faster)
codex delegate-parallel code-reviewer,test-gen --scopes ./src,./tests

# Deep research with citations
codex research "React Server Components best practices" --depth 3

# Custom agent creation
codex agent-create "Find all TODO comments and create summary"
```

---

## 🚨 Critical Security Notice

⚠️ **Remote Code Execution Vulnerability** ([#5121](https://github.com/openai/codex/issues/5121))

**ALWAYS**:
- ✅ Use sandbox mode (`--sandbox=read-only` or `workspace-write`)
- ✅ Set approval policy to `on-request` for untrusted code
- ✅ Review all generated shell commands before execution
- ✅ Never use `--approval never` with `--sandbox=danger-full-access`

```bash
# Safe execution
codex --sandbox=read-only --ask-for-approval on-request "task"
```

---

## 🤖 Model Selection Strategy

| Task Type | Model | Reasoning |
|-----------|-------|-----------|
| Quick fixes | `gpt-4o-mini` | Fast, cost-effective |
| Standard development | `gpt-4o` | Balanced performance |
| Complex refactoring | `gpt-4o` | Strong code understanding |
| Algorithm design | `o1-preview` | Superior reasoning |

```bash
codex --model gpt-4o-mini "Rename variable foo to bar"
codex --model gpt-4o "Implement JWT authentication"
codex --model o1-preview "Optimize sorting algorithm"
```

---

## 🔒 Security Checklist

Before deploying AI-generated code:

- [ ] Reviewed all file operations
- [ ] Verified input validation
- [ ] Checked for SQL injection vectors
- [ ] Validated authentication logic
- [ ] Confirmed error handling
- [ ] Tested edge cases
- [ ] Ran security linter (`cargo-audit`, `npm audit`)
- [ ] Reviewed audit logs
- [ ] Verified sandbox was enabled
- [ ] Confirmed no hardcoded secrets

---

## 💻 Coding Standards

### TypeScript/JavaScript

```typescript
// ✅ GOOD: Explicit types
function getUserById(id: number): Promise<User | null> {
  return database.findUser(id);
}

// ❌ BAD: any type
function getUserById(id: any): any { ... }

// ✅ GOOD: Optional chaining
const userName = user?.profile?.name ?? 'Anonymous';

// ❌ BAD: Nested conditionals
const userName = user && user.profile && user.profile.name ? ... : 'Anonymous';
```

**Rules**:
- ✅ Use `const` by default, `let` only when reassignment needed
- ✅ Prefer `async/await` over `.then()` chains
- ✅ Use optional chaining (`?.`) and nullish coalescing (`??`)
- ❌ NEVER use `any` type
- ❌ NEVER use `var`

### Python

```python
# ✅ GOOD: Type hints
def calculate_total(items: list[Item]) -> Decimal:
    return sum(item.price for item in items)

# ✅ GOOD: pathlib
from pathlib import Path
config_path = Path.home() / ".config" / "app.toml"
```

**Rules**:
- ✅ Follow PEP 8 style guide
- ✅ Use type hints (PEP 484)
- ✅ Use `pathlib` instead of `os.path`
- ✅ Format with Black

### Rust

```rust
// ✅ GOOD: Inline format arguments
println!("User {name} has {count} items");

// ✅ GOOD: Iterator chains
let total: i32 = numbers.iter().filter(|&&x| x > 0).sum();

// ✅ GOOD: Method reference
items.iter().map(Item::price)
```

**Rules**:
- ✅ Follow Clippy lints (all categories)
- ✅ Use inline format arguments (`println!("{name}")`)
- ✅ Prefer iterators over explicit loops
- ❌ NEVER use `unsafe` without justification and review

**Build Process** (CRITICAL):

```powershell
# After Rust code changes
cd codex-rs
cargo clean
cargo build --release -p codex-cli
cargo install --path cli --force
codex --version  # Verify: codex-cli 0.47.0-alpha.1
```

### C# Unity

```csharp
// ✅ GOOD: SerializeField with private
[SerializeField] private float speed = 5f;

// ✅ GOOD: Object pooling
private Queue<GameObject> bulletPool = new();
```

**Rules**:
- ✅ Use `[SerializeField]` for inspector-visible fields
- ✅ Implement object pooling for frequently created objects
- ❌ NEVER allocate in `Update()` or `FixedUpdate()`
- ❌ NEVER use `GetComponent()` in `Update()`

---

## 🐛 Known Issues & Workarounds

Based on [OpenAI/codex Issues](https://github.com/openai/codex/issues) (2025-10-12):

### 🔴 Critical: Security

**[#5121](https://github.com/openai/codex/issues/5121) Remote Code Execution**
- Always use sandbox mode
- Set approval policy to `on-request`
- Review all shell commands

### 🟡 IDE Integration

**[#5114](https://github.com/openai/codex/issues/5114) VS Code: Slash commands not working**
- Workaround: Use CLI instead
```bash
codex exec "/review src/main.ts"
```

**[#5113](https://github.com/openai/codex/issues/5113) Japanese: /review ignores language**
- Workaround: Explicitly specify language
```bash
codex "Review this code in Japanese: [code]"
```

### 🟡 Model Behavior

**[#5117](https://github.com/openai/codex/issues/5117) Model gives up early**
- Break tasks into smaller chunks
- Use explicit continuation prompts

**[#5103](https://github.com/openai/codex/issues/5103) Model changes API style**
- Use `--model gpt-4o` (better instruction following)
- Review diffs carefully

### 🟡 CLI Issues

**[#5107](https://github.com/openai/codex/issues/5107) macOS: OSC palette reply**
- Use iTerm2 or update Terminal.app preferences

---

## 🤖 Sub-Agent System

### Available Agents

| Agent | Purpose | Token Budget |
|-------|---------|--------------|
| `code-reviewer` | Security, performance, best practices | 40,000 |
| `test-gen` | Unit/Integration/E2E test generation | 30,000 |
| `sec-audit` | CVE scanning, dependency audit | 50,000 |
| `researcher` | Deep research with citations | 60,000 |

### Usage

```bash
# Single agent
codex delegate code-reviewer --scope ./src

# Parallel (3x faster)
codex delegate-parallel code-reviewer,test-gen --scopes ./src,./tests

# Custom agent
codex agent-create "Task description"
```

### Composer Integration

```
@code-reviewer このファイルをレビューして
@researcher Reactの最新ベストプラクティスを調査
@test-gen このモジュールのテストを生成
@sec-audit セキュリティ脆弱性をチェック
```

---

## 🔍 Deep Research

```bash
# Quick research
codex research "React Server Components best practices"

# Deep dive
codex research "Rust async error handling" --depth 5 --strategy comprehensive

# Broad survey
codex research "Modern web frameworks" --strategy exploratory
```

**Features**:
- Multi-source research (DuckDuckGo, Brave, Google, Bing)
- Contradiction detection
- Cited reports with confidence scores

---

## 🧪 Testing Requirements

### Coverage Goals

- **Unit Test**: 80%+
- **Integration Test**: 100% main flows
- **E2E Test**: 100% critical paths

### Test Frameworks

- **TypeScript**: Jest, Vitest, React Testing Library
- **Python**: pytest, unittest
- **Rust**: `cargo test`
- **Unity**: Unity Test Framework, NUnit

---

## 🛡️ Security Best Practices

### 1. Never Run Untrusted Code Without Review

```bash
# ❌ DANGEROUS
codex --approval never "download and execute script"

# ✅ SAFE
codex --approval on-request "download and execute script"
```

### 2. Sandbox All File Operations

```toml
# ~/.codex/config.toml
[sandbox]
default_mode = "read-only"  # CRITICAL

[sandbox_permissions]
workspace_write = true
disk_full_read_access = false  # NO full disk access
network_access = false  # NO network by default
```

### 3. API Key Management

```bash
# ✅ CORRECT
export OPENAI_API_KEY="sk-..."

# ❌ WRONG
# api_key = "sk-..."  # NEVER hardcode!
```

### 4. Code Review AI-Generated Changes

**Never blindly accept**:
- Authentication code
- Cryptographic operations
- SQL queries
- File system operations
- Network requests

---

## 📦 Configuration

### Recommended config.toml

```toml
# ~/.codex/config.toml
model = "gpt-4o"

[model_providers.openai]
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "chat"

[sandbox]
default_mode = "read-only"

[approval]
policy = "on-request"

[mcp_servers.codex-agent]
command = "codex"
args = ["mcp-server"]
```

---

## 📝 Commit Convention

**Conventional Commits**:

```bash
feat: 新機能追加
fix: バグ修正
docs: ドキュメント更新
style: コードフォーマット
refactor: リファクタリング
test: テスト追加
chore: ビルド・設定変更
```

**Examples**:

```bash
git commit -m "feat: TypeScript code reviewer with React hooks validation"
git commit -m "fix: SQL injection vulnerability in user query"
```

---

## 🚀 Performance Optimization

### TypeScript/React

- `useMemo` / `useCallback` 適切使用
- `React.lazy` でコード分割
- Bundle size監視（< 200KB推奨）

### Python

- リスト内包表記（ループより高速）
- `functools.lru_cache` でキャッシュ
- 非同期IO（asyncio）活用

### Unity

- **Update内禁止**: `new`, `GetComponent`, `Find`
- Object Pooling実装
- Addressables使用

---

## 🎯 Best Practices

1. **Sub-Agent活用**: 専門タスクは専用エージェントに委譲
2. **Deep Research**: 未知の技術調査時に必ず使用
3. **Security First**: コード変更時は必ず脆弱性チェック
4. **Test Driven**: 実装前にテスト生成で仕様明確化
5. **Continuous Improvement**: レビュー結果を次回実装に反映

---

## 📚 Resources

### Documentation

- `.cursor/rules.md` - Complete project rules
- `INSTALL_SUBAGENTS.md` - Installation guide
- `_docs/` - Implementation logs
- [OpenAI/codex](https://github.com/openai/codex) - Official repository
- [OpenAI/codex Issues](https://github.com/openai/codex/issues) - Known issues

### Sample Commands

```bash
# Multi-language review
codex delegate code-reviewer --scope ./src

# Deep Research (depth 3)
codex research "React Server Components best practices" --depth 3

# Security audit
codex delegate sec-audit --scope ./

# Parallel execution
codex delegate-parallel code-reviewer,test-gen --scopes ./src,./tests
```

---

## ⚠️ Common Pitfalls

### ❌ Don't Do This

```bash
# BAD: Auto-approve + full access
codex --approval never --sandbox danger-full-access "task"

# BAD: Hardcode API key
api_key = "sk-..."

# BAD: No sandbox
codex "execute untrusted script"
```

### ✅ Do This Instead

```bash
# GOOD: Safe execution
codex --approval on-request --sandbox read-only "task"

# GOOD: Environment variable
export OPENAI_API_KEY="sk-..."

# GOOD: Sandbox + approval
codex --sandbox workspace-write --approval on-request "safe task"
```

---

## 📊 Project Structure

```
codex-main/
├── codex-rs/          # Rust core implementation
│   ├── cli/           # Command-line interface
│   ├── core/          # Core runtime
│   └── tui/           # Terminal UI
├── .codex/            # Agent definitions
│   └── agents/        # Sub-agent YAML files
├── .cursor/           # Cursor IDE configuration
│   └── rules.md       # Complete project rules
├── _docs/             # Implementation logs
└── .cursorrules       # Quick reference (Cursor IDE)
```

---

**Version**: 0.47.0-alpha.1  
**Project**: zapabob/codex  
**Maintained by**: zapabob  
**Based on**: OpenAI/codex official + community issues  
**Status**: ✅ Production Ready

**🔗 Links**:
- [Full Rules](.cursor/rules.md)
- [OpenAI Official](https://github.com/openai/codex)
- [Security Issue #5121](https://github.com/openai/codex/issues/5121)

