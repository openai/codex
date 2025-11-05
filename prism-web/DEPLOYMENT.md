# Prism デプロイメントガイド

**最終更新**: 2025年11月2日

---

## 🚀 完全無料デプロイ手順

### Phase 1: Supabase セットアップ

```bash
# 1. Supabaseアカウント作成
https://supabase.com にアクセス → Sign Up

# 2. 新規プロジェクト作成
Organization: Personal
Project name: prism-production
Database Password: (強力なパスワード生成)
Region: Northeast Asia (Tokyo)
Pricing: Free Tier ✅

# 3. SQL Editorでスキーマ実行
Dashboard → SQL Editor → New query
→ supabase/schema.sql の内容を貼り付け → Run

# 4. Storage Buckets作成
Dashboard → Storage → New bucket
  - visualizations (public)
  - avatars (public)
  - artifacts (private)

# 5. 認証設定
Dashboard → Authentication → Providers
  ✅ Email (enabled)
  ✅ GitHub OAuth (設定: https://github.com/settings/developers)

# 6. APIキー取得
Dashboard → Settings → API
  → Copy: Project URL, anon/public key
```

### Phase 2: GitHub リポジトリ

```bash
# 1. GitHubで新規リポジトリ作成
https://github.com/new
Repository name: prism
Description: AI-Native Code Intelligence Platform
Public ✅

# 2. ローカルからpush
cd prism-web
git init
git add .
git commit -m "feat: Initial Prism implementation"
git branch -M main
git remote add origin https://github.com/YOUR_USERNAME/prism.git
git push -u origin main
```

### Phase 3: Vercel デプロイ

```bash
# 1. Vercelアカウント作成
https://vercel.com → Sign up with GitHub

# 2. プロジェクトインポート
Dashboard → Add New → Project
→ GitHubリポジトリ選択: prism

# 3. 環境変数設定
Environment Variables:
  NEXT_PUBLIC_SUPABASE_URL = (Supabaseから取得)
  NEXT_PUBLIC_SUPABASE_ANON_KEY = (Supabaseから取得)
  ENCRYPTION_SECRET = (32文字以上のランダム文字列)

# 4. デプロイ設定
Framework Preset: Next.js
Build Command: npm run build
Output Directory: .next
Install Command: npm install

→ Deploy

# 5. デプロイ完了
Your project is live at: https://prism-xxx.vercel.app
```

### Phase 4: カスタムドメイン

```bash
# 1. Cloudflareでドメイン購入
https://dash.cloudflare.com → Registrar → Register Domain
Domain: prism.dev
Price: $9.77/year
Auto-renew: ON ✅

# 2. Vercelでドメイン追加
Vercel Dashboard → Settings → Domains
→ Add: prism.dev
→ Copy DNS records

# 3. Cloudflare DNS設定
Cloudflare Dashboard → DNS → Records
→ Add record:
  Type: CNAME
  Name: @
  Content: cname.vercel-dns.com
  Proxy: ON ✅

→ Add record:
  Type: CNAME
  Name: www
  Content: cname.vercel-dns.com
  Proxy: ON ✅

# 4. SSL設定
Cloudflare → SSL/TLS → Overview
  Encryption mode: Full (strict) ✅
  Always Use HTTPS: ON ✅

# 5. 確認（5-30分）
https://prism.dev にアクセス
→ 緑の南京錠🔒確認
```

### Phase 5: Edge Functions デプロイ

```bash
# 1. Supabase CLI インストール
npm install -g supabase

# 2. ログイン
supabase login

# 3. プロジェクトリンク
cd prism-web
supabase link --project-ref YOUR_PROJECT_REF

# 4. Edge Functions デプロイ
supabase functions deploy save-api-key

# 5. Secrets設定
supabase secrets set ENCRYPTION_SECRET="your-secret-here"

# 6. 確認
curl https://YOUR_PROJECT.supabase.co/functions/v1/save-api-key
```

---

## 🔧 環境変数一覧

### Vercel

```bash
# Public (ブラウザで利用可能)
NEXT_PUBLIC_SUPABASE_URL=https://xxx.supabase.co
NEXT_PUBLIC_SUPABASE_ANON_KEY=eyJxxx...
NEXT_PUBLIC_SITE_URL=https://prism.dev

# Private (サーバー側のみ)
ENCRYPTION_SECRET=your-32-char-secret-key-here
SUPABASE_SERVICE_ROLE_KEY=eyJyyy... (optional)
```

### Supabase Edge Functions

```bash
# Secrets
supabase secrets set ENCRYPTION_SECRET="xxx"
supabase secrets set OPENAI_API_KEY="sk-xxx" (optional, if server-managed)
```

### MCP Server

```bash
# ~/.claude/config.json
{
  "mcpServers": {
    "prism": {
      "env": {
        "PRISM_SUPABASE_URL": "https://xxx.supabase.co",
        "PRISM_SUPABASE_KEY": "eyJxxx...",
        "PRISM_API_URL": "https://prism.dev"
      }
    }
  }
}
```

---

## 📊 デプロイチェックリスト

### 必須項目

- [ ] Supabaseプロジェクト作成
- [ ] DBスキーマ実行
- [ ] Storage Buckets作成
- [ ] GitHub OAuth設定
- [ ] GitHubリポジトリ作成
- [ ] Vercel連携
- [ ] 環境変数設定
- [ ] 初回デプロイ成功
- [ ] ドメイン購入
- [ ] DNS設定
- [ ] SSL有効化
- [ ] カスタムドメイン確認

### 推奨項目

- [ ] GitHub Actions CI/CD
- [ ] Lighthouse スコア95+確認
- [ ] OG画像設定
- [ ] Sitemap生成
- [ ] robots.txt設定
- [ ] エラー追跡（Sentry）
- [ ] アナリティクス（Plausible）

---

## 🧪 デプロイ後テスト

```bash
# 1. HTTPSアクセス確認
curl -I https://prism.dev
→ HTTP/2 200

# 2. Supabase接続確認
→ Login/Signupページ正常動作

# 3. APIキー保存確認
→ Settings → API Keys → Save test key

# 4. 可視化確認
→ Add repository → Visualize

# 5. MCP Server確認
→ Claude Codeで @prism呼び出し
```

---

## 🔄 更新手順

```bash
# コード変更後
git add .
git commit -m "feat: Add new feature"
git push origin main

# Vercelが自動デプロイ（1-2分）
→ https://vercel.com/dashboard で進捗確認
→ デプロイ完了通知

# 確認
→ https://prism.dev で動作確認
```

---

## 📈 監視

### Vercel Analytics

```bash
Dashboard → Analytics
  - Page views
  - Unique visitors
  - Top pages
  - Real-time users
```

### Supabase Dashboard

```bash
Dashboard → Database → Statistics
  - Active connections
  - Database size
  - Queries per second

Dashboard → Storage → Usage
  - Storage used
  - Bandwidth used
```

---

## 🆘 トラブルシューティング

### デプロイ失敗

```bash
# ビルドログ確認
Vercel Dashboard → Deployments → Failed deployment → View logs

# Common issues:
1. 環境変数未設定 → Settings → Environment Variables
2. Node versionミスマッチ → package.json engines field
3. Type errors → npm run type-check
```

### Supabase接続エラー

```bash
# .env.local確認
cat .env.local
→ NEXT_PUBLIC_SUPABASE_URL correct?
→ NEXT_PUBLIC_SUPABASE_ANON_KEY correct?

# Network確認
curl https://YOUR_PROJECT.supabase.co/rest/v1/
→ 200 OKならSupabase稼働中
```

### Edge Function エラー

```bash
# Logs確認
supabase functions logs save-api-key

# ローカルテスト
supabase functions serve save-api-key
curl -X POST http://localhost:54321/functions/v1/save-api-key \
  -H "Content-Type: application/json" \
  -d '{"provider":"openai","apiKey":"sk-test"}'
```

---

**総コスト**: $0-10/月（無料枠内）  
**デプロイ時間**: 30-60分  
**準備できたで！** 🚀✨

