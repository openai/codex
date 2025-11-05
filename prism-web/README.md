# Prism - AI-Native Code Intelligence Platform

**プロダクト名**: Prism  
**バージョン**: 1.0.0  
**ライセンス**: Apache 2.0

---

## 🎯 概要

PrismはKamui4d超えの3D/4D Git可視化とマルチLLM対応（OpenAI + Anthropic Claude）を統合した、次世代コードインテリジェンスプラットフォームです。

### 主要機能

- 🌟 **3D/4D Git可視化**: 50,000コミット@35FPS
- 🤖 **マルチLLM対応**: OpenAI GPT-4 + Anthropic Claude 3
- 🔌 **Claude Code統合**: MCP Server経由
- 🔐 **完全無料**: Supabase無料枠で構築
- 💰 **BYOK方式**: ユーザー自身のAPIキー使用

---

## 🚀 クイックスタート

### 前提条件

- Node.js 18以上
- npm or pnpm
- OpenAI APIキー or Anthropic APIキー

### インストール

```bash
# Clone repository
git clone https://github.com/zapabob/prism-web.git
cd prism-web

# Install dependencies
npm install

# Set up environment variables
cp .env.example .env.local
# Edit .env.local with your Supabase credentials

# Run development server
npm run dev
```

### Supabaseセットアップ

1. https://supabase.com でプロジェクト作成
2. SQL Editorで `supabase/schema.sql` 実行
3. Settings → API から URL と Anon Keyをコピー
4. `.env.local` に設定

```bash
NEXT_PUBLIC_SUPABASE_URL=https://xxx.supabase.co
NEXT_PUBLIC_SUPABASE_ANON_KEY=eyJxxx...
ENCRYPTION_SECRET=your-32-char-secret-key-here
```

---

## 📁 プロジェクト構造

```
prism-web/
├── app/                    # Next.js 14 App Router
│   ├── (auth)/            # 認証ページ
│   ├── (dashboard)/       # ダッシュボード
│   └── (public)/          # 公開ページ
├── components/            # Reactコンポーネント
│   ├── ui/               # shadcn/ui components
│   ├── visualizations/   # 3D可視化
│   └── chat/             # AIチャット
├── lib/                  # ユーティリティ
│   ├── supabase.ts       # Supabaseクライアント
│   ├── ai/               # AI統合
│   │   ├── openai.ts
│   │   ├── anthropic.ts
│   │   └── unified.ts
│   └── encryption.ts     # APIキー暗号化
├── supabase/             # Supabase設定
│   ├── schema.sql        # DBスキーマ
│   └── functions/        # Edge Functions
└── public/               # 静的ファイル
```

---

## 🤖 使い方

### 1. アカウント作成

1. https://prism.dev にアクセス
2. "Sign Up" をクリック
3. メールアドレスとパスワードで登録

### 2. APIキー設定

1. Dashboard → Settings → API Keys
2. OpenAI APIキーを追加
3. （オプション）Anthropic APIキーを追加

**APIキーの取得**:
- OpenAI: https://platform.openai.com/api-keys
- Anthropic: https://console.anthropic.com/

### 3. リポジトリ可視化

1. Dashboard → Repositories → Add Repository
2. GitHubのURLまたはローカルパスを入力
3. "Visualize" をクリック
4. 3D表示で確認！

### 4. AIチャット

1. Dashboard → Chat
2. プロバイダーとモデルを選択
3. コードレビュー、質問、リファクタリング提案など

---

## 🔌 Claude Code統合

### MCP Server セットアップ

```bash
# Build MCP server
cd prism-mcp-server
npm install
npm run build

# Configure Claude
# Add to ~/.claude/config.json:
{
  "mcpServers": {
    "prism": {
      "command": "node",
      "args": ["/absolute/path/to/prism-mcp-server/dist/index.js"],
      "env": {
        "PRISM_SUPABASE_URL": "https://xxx.supabase.co",
        "PRISM_SUPABASE_KEY": "eyJxxx...",
        "PRISM_API_URL": "https://prism.dev"
      }
    }
  }
}
```

### Claude Codeでの使用

```
You: @prism visualize ./my-project

Claude: I'll visualize your repository in 3D.
[Calls prism MCP server]
Visualization created: https://prism.dev/share/abc123

You: @prism analyze this code for security issues

Claude: [Uses Prism's AI analysis]
Found 2 potential security issues:
1. SQL injection risk in line 42
2. Missing input validation in line 67
```

---

## 💻 開発

### ローカル開発

```bash
# Frontend
npm run dev         # http://localhost:3000

# MCP Server
cd prism-mcp-server
npm run dev         # Watch mode

# Supabase local (optional)
npx supabase start  # Requires Docker
```

### テスト

```bash
# Frontend tests
npm run test

# Type check
npm run type-check

# Lint
npm run lint
```

### ビルド

```bash
# Production build
npm run build

# Start production server
npm start
```

---

## 🌐 デプロイ

### Vercel（推奨）

```bash
# Install Vercel CLI
npm i -g vercel

# Deploy
vercel

# Add environment variables
vercel env add NEXT_PUBLIC_SUPABASE_URL
vercel env add NEXT_PUBLIC_SUPABASE_ANON_KEY
vercel env add ENCRYPTION_SECRET

# Deploy to production
vercel --prod
```

### カスタムドメイン

1. Vercel Dashboard → Settings → Domains
2. Add `prism.dev`
3. Cloudflare DNSにCNAME追加

---

## 🔐 セキュリティ

### APIキー保護

- ✅ AES-256暗号化
- ✅ Supabase Edge Functionsでサーバー側のみ処理
- ✅ Row Level Security (RLS)
- ✅ HTTPS必須

### 環境変数

**絶対にコミットしない**:
- `.env.local`
- `ENCRYPTION_SECRET`
- API Keys

---

## 📊 Supabase無料枠制限

| 項目 | 制限 |
|------|-----|
| Database | 500 MB |
| Storage | 1 GB |
| Bandwidth | 2 GB/月 |
| Edge Functions | 500K呼び出し/月 |
| Realtime | 同時接続200 |

**対策**:
- 大規模リポジトリは圧縮
- 画像最適化
- CDN使用（Vercel標準）

---

## 🤝 コントリビューション

PRs welcome! See [CONTRIBUTING.md](../CONTRIBUTING.md)

---

## 📄 ライセンス

Apache 2.0 - See [LICENSE](../LICENSE)

---

## 🆘 サポート

- Discord: https://discord.gg/prism
- GitHub Issues: https://github.com/zapabob/prism/issues
- Email: support@prism.dev

