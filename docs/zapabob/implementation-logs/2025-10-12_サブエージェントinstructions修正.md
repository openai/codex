# Codex サブエージェント Instructions フィールド実装修正

**日時**: 2025-10-12 19:09 JST  
**作業者**: AI Assistant  
**課題**: サブエージェントの `delegate` コマンドが "Instructions are not valid" エラーで失敗

---

## 📋 問題の概要

`codex delegate code-reviewer --scope ./src` を実行すると以下のエラーが発生:

```
⚠️  Agent reported an error: unexpected status 400 Bad Request: {"detail":"Instructions are not valid"}
Error: agent 'code-reviewer' failed: unexpected status 400 Bad Request: {"detail":"Instructions are not valid"}
```

---

## 🔍 原因調査

### 1. エージェント定義ファイルの確認

`.codex/agents/*.yaml` ファイルに `instructions` フィールドが定義されていなかった。

**問題のYAML**:
```yaml
name: "code-reviewer"
goal: "Comprehensive code review with security, performance, and best practices analysis"
tools:
  mcp:
    - grep
    - read_file
# instructions フィールドが無い！
```

### 2. Rust実装の確認

#### `AgentDefinition` 構造体（`codex-rs/core/src/agents/types.rs`）
- `instructions` フィールドが定義されていなかった
- YAML からパースできない

#### `AgentRuntime::execute_agent`（`codex-rs/core/src/agents/runtime.rs`）
- システムプロンプトがハードコードされており、YAML の `instructions` を使用していなかった

---

## ✅ 実施した修正

### 1. エージェント定義ファイルに `instructions` 追加

**修正ファイル**:
- `.codex/agents/code-reviewer.yaml`
- `.codex/agents/researcher.yaml`
- `.codex/agents/test-gen.yaml`
- `.codex/agents/sec-audit.yaml`

**追加内容例** (code-reviewer):
```yaml
name: "code-reviewer"
goal: "Comprehensive code review with security, performance, and best practices analysis"
instructions: |
  You are a specialized code reviewer agent. Your role is to analyze code for:
  
  1. **Type Safety**: Check for type errors, unsafe casts, and missing type annotations
  2. **Security**: Identify vulnerabilities like SQL injection, XSS, CSRF, hardcoded secrets
  3. **Performance**: Suggest optimizations for algorithms, memory usage, and async patterns
  4. **Best Practices**: Ensure code follows language-specific conventions and patterns
  
  For each issue found:
  - Provide file path and line number
  - Explain the problem clearly
  - Suggest a concrete fix with code example
  - Rate severity (Critical/High/Medium/Low)
  
  Generate a detailed markdown report with your findings.
tools:
  mcp:
    - grep
    - read_file
    - codebase_search
  ...
```

### 2. Rust 構造体に `instructions` フィールド追加

**ファイル**: `codex-rs/core/src/agents/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentDefinition {
    pub name: String,
    pub goal: String,
    /// 詳細なインストラクション（LLMに渡されるシステムプロンプトの一部）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,  // ← 追加
    pub tools: ToolPermissions,
    pub policies: AgentPolicies,
    pub success_criteria: Vec<String>,
    pub artifacts: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
```

### 3. システムプロンプト構築ロジック修正

**ファイル**: `codex-rs/core/src/agents/runtime.rs`

```rust
async fn execute_agent(
    &self,
    agent_def: &AgentDefinition,
    goal: &str,
    inputs: HashMap<String, String>,
    _deadline: Option<u64>,
) -> Result<Vec<String>> {
    // システムプロンプト構築
    let mut system_prompt = format!(
        "You are a specialized sub-agent with the following role:\n\
         \n\
         Agent: {}\n\
         Goal: {}",
        agent_def.name, agent_def.goal,
    );

    // instructionsフィールドがあればそれを含める ← 追加
    if let Some(ref instructions) = agent_def.instructions {
        system_prompt.push_str("\n\nInstructions:\n");
        system_prompt.push_str(instructions);
    }

    system_prompt.push_str(&format!(
        "\n\
         \n\
         Success Criteria:\n{}\n\
         \n\
         Inputs provided:\n{}\n\
         \n\
         Please analyze the task and execute it according to your role.\
         Generate the required artifacts as specified.",
        agent_def.success_criteria.join("\n- "),
        inputs
            .iter()
            .map(|(k, v)| format!("- {}: {}", k, v))
            .collect::<Vec<_>>()
            .join("\n")
    ));
    
    // ... 以降のコード
}
```

### 4. ビルド依存関係修正

**ファイル**: `codex-rs/rmcp-client/Cargo.toml`

```toml
[dependencies]
# ...
codex-protocol = { path = "../protocol" }  # ← 追加（欠けていた依存関係）
# ...
```

---

## 🏗️ ビルド結果

```bash
$ cargo build --release -p codex-cli
   Compiling codex-core v0.47.0-alpha.1
   Compiling codex-cli v0.47.0-alpha.1
    Finished `release` profile [optimized] target(s) in 12m 29s
```

**ビルド成功！** ✅

---

## ⚠️ 残存する問題

### API エラーが継続

修正後も同じエラーが発生:

```
⚠️  Agent reported an error: unexpected status 400 Bad Request: {"detail":"Instructions are not valid"}
```

### 考えられる原因

1. **キャッシュ問題**: 古いバイナリが実行されている可能性
   - `target/release/codex.exe` が更新されていないかも

2. **API互換性問題**: OpenAI Responses API が特定のフォーマットを要求している可能性
   - `base_instructions_override` が正しく処理されていない
   - APIバージョンの不一致

3. **環境設定**: ユーザーの設定ファイルが Responses API を使用している可能性
   - デフォルトは Chat Completions API (`wire_api = "chat"`)
   - ユーザー設定で上書きされている可能性

### 確認済み事項

✅ `get_full_instructions()` は正しく `base_instructions_override` を処理  
✅ Chat Completions API はシステムメッセージとして送信  
✅ YAML パースは正常に動作（`instructions` フィールド読み込み可能）

---

## 📝 次のステップ（未実施）

### 1. インストール実行

```bash
cargo install --path ./codex-rs/cli --force
```

**問題**: `ring` クレートのビルドエラー（MSVC コンパイラクラッシュ）
```
error: failed to run custom build command for `ring v0.17.14`
exit code: 0xc0000005 (ACCESS_VIOLATION)
```

### 2. デバッグログ確認

```powershell
$env:RUST_LOG="debug"
codex delegate code-reviewer --scope ./codex-rs/cli/src
```

→ ログ出力されず（環境変数が反映されていない可能性）

### 3. API 通信内容の確認

リクエストボディに `instructions` が正しく含まれているか確認する必要がある。

---

## 🎯 推奨される追加調査

1. **ネットワークログ取得**:
   ```rust
   // reqwest クライアントのログを有効化
   env_logger::init();
   ```

2. **API レスポンス詳細確認**:
   エラーレスポンスの完全なボディを取得

3. **モデルプロバイダー確認**:
   ```bash
   codex --help | grep model
   cat ~/.codex/config.toml | grep wire_api
   ```

4. **代替テスト**:
   - Chat Completions API を明示的に指定
   - 最小限の `instructions` でテスト
   - 既存の動作する機能（`codex research`）と比較

---

## 📌 まとめ

### 完了した作業

✅ エージェント定義ファイルに `instructions` フィールド追加（4ファイル）  
✅ `AgentDefinition` 構造体に `instructions` フィールド追加  
✅ システムプロンプト構築ロジックに `instructions` 統合  
✅ ビルド依存関係修正（`codex-rmcp-client`）  
✅ リリースビルド成功

### 未完了の作業

❌ エラーの根本原因特定  
❌ 動作確認（APIエラーのため）  
❌ `cargo install` 実行（`ring` ビルドエラー）

### 技術的負債

⚠️ `ring` クレートの MSVC ビルド問題
⚠️ API エラーの詳細調査不足  
⚠️ デバッグログ環境の未整備

---

## 📚 関連ドキュメント

- [AGENTS.md](../AGENTS.md) - サブエージェントの概要
- [docs/codex-subagents-deep-research.md](../docs/codex-subagents-deep-research.md) - 詳細仕様
- [INSTALL_SUBAGENTS.md](../INSTALL_SUBAGENTS.md) - インストール手順

---

**ステータス**: 🚧 部分完了（コード修正済み、動作未確認）  
**次回作業者へ**: API エラーの詳細調査とデバッグログ環境の整備を推奨

