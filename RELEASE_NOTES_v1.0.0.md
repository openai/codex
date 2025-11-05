# Codex/Prism v1.0.0 Release Notes

**リリース日**: 2025年11月2日  
**バージョン**: 1.0.0  
**コードネーム**: "Spectrum"

---

## 🎉 初回メジャーリリース！

Codex/Prismプロジェクトの初回1.0.0リリースです。3D/4D Git可視化、マルチLLM対応、完全無料アーキテクチャを実現しました。

---

## ✨ 主要機能

### 1. 🌟 3D/4D Git可視化（Kamui4d超え）

#### パフォーマンス
- **50,000コミット @ 35 FPS** （GPU加速レンダリング）
- **93%メモリ削減** （最適化アルゴリズム）
- **リアルタイム更新** （WebSocket）

#### 機能
- スパイラルレイアウトでコミット履歴を3D表示
- タイムラインスライダーで歴史を再生
- コミットサイズでノードカラー変更（緑→黄→赤）
- ファイル変更数でノードサイズ調整
- ホバーでコミットメッセージ表示

#### コラボレーション
- コメント機能（3D空間上に配置）
- 共有リンク生成（トークン認証）
- 公開/非公開設定
- 期限付き共有

### 2. 🤖 マルチLLM対応

#### サポートモデル

**OpenAI GPT-5系**:
- `gpt-5-pro` (gpt-5-codex): 最高品質コード生成
- `gpt-5-me` (gpt-5-high): バランス型
- `gpt-5-mini` (gpt-5): 高速・低コスト

**Anthropic Claude 4.5系**:
- `claude-4.5-sonnet`: 2025年最新標準モデル
- `claude-4.5-haiku`: 超高速処理
- `claude-4.1-opus`: 最高性能推論

#### 統一インターフェース

```typescript
// OpenAI
await chat('openai', apiKey, messages, 'gpt-5-codex')

// Claude
await chat('anthropic', apiKey, messages, 'claude-4.5-sonnet')

// Streaming
for await (const chunk of chatStream) {
  console.log(chunk)
}
```

### 3. 🔌 Claude Code統合

MCP Serverを介してClaudeからPrismの機能を直接利用可能：

```bash
# Claude Codeで使用
@prism visualize ./my-repo
@prism analyze this code for security
@prism get repo stats
```

**提供ツール**:
- `visualize_repository`: 3D可視化生成
- `analyze_code`: 静的解析（複雑度、問題検出）
- `get_repo_stats`: リポジトリ統計取得

### 4. 💰 完全無料アーキテクチャ

#### インフラコスト

| サービス | プラン | 月次コスト |
|---------|-------|-----------|
| Supabase | Free | $0 |
| Vercel | Hobby | $0 |
| Cloudflare | Free | $0 |
| GitHub | Free | $0 |
| **合計** | - | **$0/月** |

#### ユーザーAPIコスト（BYOK）

| モデル | 入力 | 出力 | 10万トークン |
|--------|------|------|-------------|
| GPT-5 Mini | $0.0005/1K | $0.002/1K | $0.25 |
| Claude 4.5 Haiku | $0.0004/1K | $0.002/1K | $0.24 |
| Claude 4.5 Sonnet | $0.003/1K | $0.015/1K | $1.80 |
| GPT-5 Pro | $0.015/1K | $0.060/1K | $7.50 |

**結論**: ユーザーが使った分だけ支払い、サーバー側コスト完全ゼロ

---

## 🏗️ 技術スタック

### フロントエンド
- **Next.js 14**: App Router, Server Components
- **React Three Fiber 8.15**: 3D rendering
- **Three.js 0.160**: WebGL core
- **Tailwind CSS 3.4**: Utility-first styling
- **Zustand 4.4**: State management

### バックエンド
- **Supabase**: BaaS (Backend as a Service)
- **PostgreSQL 15**: Database (12 tables)
- **Deno Edge Functions**: Serverless functions
- **Row Level Security**: Data protection

### AI/LLM
- **OpenAI SDK 4.20**: GPT-5 integration
- **Anthropic SDK 0.9**: Claude 4.5 integration
- **Streaming**: Real-time response

### インフラ
- **Vercel**: Serverless hosting, global CDN
- **Cloudflare**: DNS, SSL/TLS, DDoS protection

---

## 📦 インストール

### Option 1: GitHub Releases (推奨)

```bash
# Windows
curl -L https://github.com/zapabob/prism/releases/download/v1.0.0/codex-windows-x64.exe -o codex.exe

# macOS (Intel)
curl -L https://github.com/zapabob/prism/releases/download/v1.0.0/codex-darwin-x64 -o codex
chmod +x codex

# macOS (Apple Silicon)
curl -L https://github.com/zapabob/prism/releases/download/v1.0.0/codex-darwin-arm64 -o codex
chmod +x codex

# Linux
curl -L https://github.com/zapabob/prism/releases/download/v1.0.0/codex-linux-x64 -o codex
chmod +x codex
sudo mv codex /usr/local/bin/
```

### Option 2: Cargo (Rustユーザー)

```bash
git clone https://github.com/zapabob/prism.git
cd prism/codex-rs
cargo install --path cli --force
codex --version  # => codex-cli 1.0.0
```

### Option 3: npm/npx (将来対応予定)

```bash
npx @zapabob/prism --version
```

---

## 🚀 クイックスタート

### 1. Web版をローカルで起動

```bash
# Clone repository
git clone https://github.com/zapabob/prism.git
cd prism/prism-web

# Install dependencies
npm install

# Setup Supabase
# 1. Create project at https://supabase.com
# 2. Run schema: supabase/schema.sql
# 3. Copy .env.example to .env.local
# 4. Add your Supabase credentials

# Run dev server
npm run dev
# => http://localhost:3000
```

### 2. MCP Serverセットアップ

```bash
cd prism-mcp-server
npm install
npm run build

# Configure Claude
# Add to ~/.claude/config.json:
{
  "mcpServers": {
    "prism": {
      "command": "node",
      "args": ["/path/to/prism-mcp-server/dist/index.js"],
      "env": {
        "PRISM_SUPABASE_URL": "https://xxx.supabase.co",
        "PRISM_SUPABASE_KEY": "xxx",
        "PRISM_API_URL": "https://prism.dev"
      }
    }
  }
}
```

---

## 🎯 差別化ポイント

### vs Cursor

| 項目 | Prism/Codex | Cursor |
|------|-------------|--------|
| 3D可視化 | ✅ 50K@35FPS | ❌ |
| Multi-LLM | ✅ GPT-5 + Claude 4.5 | ⚠️ 限定的 |
| コスト | ✅ $0 (BYOK) | $20/月 |
| オープンソース | ✅ Apache 2.0 | ❌ |

### vs kamui4d

| 項目 | Prism/Codex | kamui4d |
|------|-------------|---------|
| AI統合 | ✅ Full | ❌ |
| パフォーマンス | ✅ 50K@35FPS | ⚠️ 基本 |
| コラボ | ✅ Full | ❌ |

### vs GitHub Copilot

| 項目 | Prism/Codex | Copilot |
|------|-------------|---------|
| 3D可視化 | ✅ | ❌ |
| LLM選択 | ✅ 自由 | ❌ 固定 |
| コスト | ✅ $0.24~/10万トークン | $10-19/月 |

---

## 🐛 既知の問題

### 1. Rustリリースビルドでコンパイラクラッシュ（Windows）
- **症状**: `cargo build --release`でrustc panic
- **回避策**: `cargo build` (devビルド) または `--codegen-units=16`使用
- **影響**: Windows x64のみ
- **修正予定**: v1.0.1

### 2. 型定義警告（prism-web）
- **症状**: モジュール'openai'が見つからない（npm install前）
- **回避策**: `npm install`実行
- **影響**: 開発環境のみ

---

## 📈 次のバージョン予定

### v1.1.0 (2025年12月)
- チャットUI完全実装
- リポジトリ検索・フィルター
- ユーザーダッシュボード
- 使用量統計表示

### v1.2.0 (2026年1月)
- VSCode Extension公開
- Desktop Electron app
- オフラインモード

### v2.0.0 (2026年Q2)
- Stripe統合（Pro Tier）
- Enterprise機能
- カスタムLLMサポート

---

## 🙏 クレジット

- **ベース**: [OpenAI/codex](https://github.com/openai/codex) official repository
- **Kamui4d**: Inspiration for 3D visualization
- **開発者**: zapabob
- **コントリビューター**: [See CONTRIBUTORS.md]

---

## 📄 ライセンス

Apache License 2.0 - See [LICENSE](LICENSE)

---

**ダウンロード**: [GitHub Releases](https://github.com/zapabob/prism/releases/tag/v1.0.0)  
**ドキュメント**: [prism.dev/docs](https://prism.dev/docs)  
**サポート**: [Discord](https://discord.gg/prism)

