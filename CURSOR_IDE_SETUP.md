# 🎯 Cursor IDE完全統合ガイド - サブエージェント & Deep Research

## 📦 セットアップ（5分で完了）

### 1. 前提条件
- ✅ Cursor IDE インストール済み
- ✅ Node.js v20+ LTS
- ✅ Rust 1.83+（Codexビルド済み）
- ✅ Python 3.11+（オプション、Python開発時）

### 2. 環境変数設定

`.env` ファイルを作成：
```bash
# Web Search API Keys (Deep Research用)
BRAVE_API_KEY=your_brave_api_key
GOOGLE_API_KEY=your_google_api_key
GOOGLE_CSE_ID=your_google_cse_id
BING_API_KEY=your_bing_api_key

# OpenAI (オプション)
OPENAI_API_KEY=your_openai_api_key
```

### 3. MCP サーバー有効化

Cursor設定を開く：
```
Ctrl/Cmd + , → "MCP" で検索
```

以下をチェック：
- ☑️ Enable MCP Servers
- ☑️ Load .cursor/mcp.json
- ☑️ Auto-detect tools

### 4. 拡張機能インストール

Cursor内で推奨拡張を一括インストール：
```
Ctrl/Cmd + Shift + P → "Extensions: Show Recommended Extensions"
```

## 🚀 使い方

### 💬 Composerで使う

#### 1. コードレビュー
Composerを開いて：
```
@code-reviewer このファイルをレビューして
```

または特定言語専用：
```
@ts-reviewer このReactコンポーネントをレビュー
@python-reviewer このDjangoビューを確認
@unity-reviewer このMonoBehaviourスクリプトを最適化
```

#### 2. Deep Research
```
@researcher Next.js 14の最新ベストプラクティスを調査して

以下の観点で：
- Server Components vs Client Components
- App Router推奨パターン
- パフォーマンス最適化
```

実行されること：
- ✅ 複数の検索エンジンで並列検索
- ✅ 矛盾チェック・クロスバリデーション
- ✅ 引用付きMarkdownレポート生成
- ✅ `artifacts/research-YYYY-MM-DD.md` 保存

#### 3. テスト生成
```
@test-gen このモジュールのテストスイートを生成

要件：
- Unit Test
- Integration Test
- Edge cases カバー
- カバレッジ 80%以上
```

#### 4. セキュリティ監査
```
@sec-audit このプロジェクト全体をスキャン

重点：
- SQL injection
- XSS vulnerabilities
- Hardcoded secrets
- Dependency vulnerabilities
```

### 🎹 キーボードショートカット

| ショートカット | 機能 | 説明 |
|--------------|------|------|
| `Ctrl+Shift+R` | **Code Review** | 現在のファイルをレビュー |
| `Ctrl+Shift+S` | **Deep Research** | 選択テキストで調査開始 |
| `Ctrl+Shift+T` | **Test Generation** | テスト自動生成 |
| `Ctrl+Shift+A` | **Security Audit** | 脆弱性スキャン |

### 📝 Chatで使う

通常のChatウィンドウでも利用可能：

```
# 現在のファイルをTypeScript専用でレビュー
@ts-reviewer

# Pythonコードをセキュリティ観点で監査
@python-reviewer --security

# Unity スクリプトのGC最適化提案
@unity-reviewer --optimize-gc
```

### 🤖 自動実行（オプション）

`.cursor/settings.json` で有効化済み：

```json
{
  "codex.autoReview": {
    "enabled": true,
    "onSave": true,      // ファイル保存時に自動レビュー
    "onCommit": true     // Git commit前に自動レビュー
  }
}
```

## 📊 利用可能なエージェント

### 1. Code Reviewer（統合版）
- **対応言語**: TypeScript, Python, Rust, C# Unity
- **自動検出**: 拡張子ベース
- **出力**: `artifacts/code-review-YYYY-MM-DD.md`

### 2. TypeScript Reviewer（専用）
- **フレームワーク**: React, Next.js, Express, NestJS, Vue, Angular
- **特化チェック**:
  - 型安全性（`any`禁止）
  - async/await パターン
  - React Hooks規則
  - パフォーマンス最適化

### 3. Python Reviewer（専用）
- **フレームワーク**: Django, FastAPI, Flask, pytest
- **特化チェック**:
  - PEP 8準拠
  - 型ヒント（PEP 484）
  - セキュリティ（SQLインジェクション等）
  - Black フォーマット

### 4. Unity Reviewer（専用）
- **対応**: Unity 2021 LTS - 6 (latest)
- **特化チェック**:
  - GC Allocation ゼロ（Update内）
  - オブジェクトプーリング
  - ScriptableObject活用
  - VR/AR最適化

### 5. Researcher（調査）
- **検索エンジン**: Brave, DuckDuckGo, Google, Bing
- **深度**: 1-5（デフォルト3）
- **機能**:
  - 矛盾検出
  - 引用必須
  - 軽量フォールバック

### 6. Test Generator（テスト生成）
- **対応**: Jest, Vitest, pytest, cargo test
- **自動生成**:
  - Unit Test
  - Integration Test
  - E2E Test
  - Mock/Stub

### 7. Security Auditor（監査）
- **スキャン対象**:
  - CVE データベース
  - 依存関係脆弱性
  - コード静的解析
  - 設定ミス検出

## 🛠️ トラブルシューティング

### MCPサーバーが起動しない
```bash
# ログ確認
tail -f ~/.cursor/logs/mcp-server.log

# 手動起動テスト
node codex-rs/mcp-server/dist/index.js
```

### エージェントが見つからない
```bash
# エージェント定義確認
ls -la .codex/agents/

# 再読み込み
Ctrl+Shift+P → "Reload Window"
```

### Deep Researchが失敗する
```bash
# API キー確認
echo $BRAVE_API_KEY
echo $GOOGLE_API_KEY

# .env ファイル読み込み確認
source .env
```

### TypeScriptエラー
```bash
# npm install再実行
cd vscode-extension
npm install
npm run compile
```

## 🎨 カスタマイズ

### 独自エージェント作成

`.codex/agents/my-custom-agent.yaml`:
```yaml
name: "My Custom Agent"
goal: "カスタムタスク実行"

tools:
  mcp:
    - custom_tool
  fs:
    read: true
    write:
      - "./my-output"
  shell:
    exec:
      - my-command

policies:
  net:
    allow:
      - "https://api.example.com"
  context:
    max_tokens: 20000

success_criteria:
  - "タスク完了"
  - "品質基準達成"
```

Composerで使用：
```
@my-custom-agent タスク実行
```

### MCPツール追加

`.cursor/mcp.json` に追加：
```json
{
  "mcpServers": {
    "my-tool": {
      "command": "node",
      "args": ["path/to/my-tool.js"],
      "capabilities": {
        "tools": ["my_custom_tool"]
      }
    }
  }
}
```

## 📈 パフォーマンス Tips

### 1. キャッシュ活用
```json
{
  "codex.cache.enabled": true,
  "codex.cache.ttl": 3600
}
```

### 2. 並列実行
```
@code-reviewer src/ & @test-gen src/ & @sec-audit src/
```

### 3. スコープ限定
```
# ❌ 遅い
@code-reviewer .

# ✅ 高速
@code-reviewer src/components/Button.tsx
```

## 🔒 セキュリティベストプラクティス

1. **API キーは環境変数で管理**
   ```bash
   # ❌ コミットしない
   .env
   
   # ✅ .gitignore に追加済み
   ```

2. **権限最小化**
   ```yaml
   # エージェント定義
   fs:
     write:
       - "./artifacts"  # 限定的
   ```

3. **定期監査**
   ```bash
   # 毎週実行
   codex delegate sec-audit --scope .
   ```

## 📚 リファレンス

### コマンド一覧
```bash
# CLI
codex delegate <agent> --scope <path>
codex research "<query>" --depth <1-5>

# Composer
@code-reviewer
@researcher
@test-gen
@sec-audit
@ts-reviewer
@python-reviewer
@unity-reviewer
```

### 設定ファイル
```
.cursorrules          # Cursor IDE ルール
.cursor/
  ├── mcp.json        # MCPサーバー設定
  ├── settings.json   # Cursor設定
  └── extensions.json # 推奨拡張
.codex/
  ├── agents/         # エージェント定義
  ├── policies/       # ポリシー設定
  └── README.md       # 詳細ドキュメント
```

### 出力ディレクトリ
```
artifacts/
  ├── code-review-*.md       # コードレビュー結果
  ├── research-*.md          # Deep Research レポート
  ├── test-suite-*.spec.ts   # 生成テスト
  └── security-audit-*.md    # セキュリティ監査
```

## 🎉 まとめ

### ✅ 完了項目
- [x] Cursor IDE統合
- [x] MCP サーバー設定
- [x] Quick Actions設定
- [x] Composer統合
- [x] Chat統合
- [x] 7つのエージェント利用可能
- [x] 自動レビュー機能
- [x] キーボードショートカット

### 🚀 次のステップ
1. `.env` ファイルにAPI キー設定
2. Cursor IDE再起動
3. `Ctrl+Shift+R` でコードレビュー試行
4. Composerで `@researcher` 試行
5. `artifacts/` ディレクトリ確認

### 📞 サポート
- GitHub: https://github.com/zapabob/codex
- Issues: https://github.com/zapabob/codex/issues
- Docs: `.codex/README.md`

---

**セットアップ完了！** 🎊  
Cursor IDEで快適なAI駆動開発を！
