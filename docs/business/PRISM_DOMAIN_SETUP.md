# Prism ドメイン取得ガイド

**プロダクト名**: Prism  
**推奨ドメイン**: `prism.dev`  
**更新日**: 2025年11月2日

---

## 🎯 ドメイン候補

### 優先順位

1. **prism.dev** - 最優先（開発者向け、短い、覚えやすい）
2. **useprism.com** - 代替案（動詞形、SaaS的）
3. **prismcode.dev** - 代替案2（明示的）
4. **getprism.dev** - 代替案3

### ドメイン可用性チェック

```bash
# Cloudflare Registrarで確認
https://www.cloudflare.com/products/registrar/

# または Namecheap
https://www.namecheap.com/domains/registration/results/?domain=prism.dev
```

---

## 💰 コスト比較

| レジストラ | .dev | .com | .ai | 特徴 |
|----------|------|------|-----|------|
| **Cloudflare** | $10/年 | $10/年 | $60/年 | 最安、無料DNS、DNSSEC |
| **Namecheap** | $15/年 | $12/年 | $80/年 | 使いやすい、WhoisGuard無料 |
| **Google Domains** | $12/年 | $12/年 | $60/年 | Google統合 |
| **GoDaddy** | $20/年 | $20/年 | $100/年 | 高い（非推奨） |

**推奨**: Cloudflare Registrar（最安 + 無料機能豊富）

---

## 🚀 Cloudflareでのドメイン購入手順

### Step 1: Cloudflareアカウント作成

1. https://dash.cloudflare.com/sign-up にアクセス
2. メールアドレス、パスワード入力
3. メール確認

### Step 2: ドメイン検索

1. Dashboard → Domain Registration
2. 検索: `prism.dev`
3. カートに追加

### Step 3: 購入

```
ドメイン: prism.dev
期間: 1年（自動更新推奨）
価格: $9.77/年

支払い方法:
- クレジットカード
- PayPal
```

### Step 4: DNS設定

```
自動設定:
✅ Cloudflare DNS (無料)
✅ DNSSEC (無料)
✅ SSL/TLS (無料)
```

---

## 🔧 DNS設定（Vercel連携）

### Vercelカスタムドメイン設定

1. Vercel Dashboard → Project → Settings → Domains
2. "Add Domain" → `prism.dev` 入力
3. Vercelが提供するDNSレコードをコピー

### Cloudflare DNS設定

```
Type: CNAME
Name: @
Content: cname.vercel-dns.com
Proxy status: Proxied (オレンジクラウド)
TTL: Auto

Type: CNAME
Name: www
Content: cname.vercel-dns.com
Proxy status: Proxied
TTL: Auto
```

### 検証

```bash
# DNS伝播確認（5-30分）
dig prism.dev
nslookup prism.dev

# HTTPSアクセステスト
curl -I https://prism.dev
```

---

## 🔐 SSL/TLS設定

### Cloudflare設定

1. SSL/TLS → Overview
2. Encryption mode: **Full (strict)** 推奨
3. Edge Certificates → Always Use HTTPS: ON
4. Minimum TLS Version: 1.2

### Vercel側

自動でLet's Encrypt証明書発行（無料）

---

## 📧 メール設定（オプション）

### Cloudflare Email Routing（無料）

```
1. Email → Email Routing
2. カスタムアドレス作成:
   - support@prism.dev → your-email@gmail.com
   - hello@prism.dev → your-email@gmail.com
   - no-reply@prism.dev → your-email@gmail.com
```

### SendGrid統合（将来）

```
Type: TXT
Name: _dmarc
Content: v=DMARC1; p=none; rua=mailto:dmarc@prism.dev

Type: TXT
Name: @
Content: v=spf1 include:sendgrid.net ~all
```

---

## 🎨 ブランディング資産

### ロゴファイル

```
/branding/
├── logo.svg              # ベクターロゴ
├── logo-dark.svg         # ダークモード用
├── logo-light.svg        # ライトモード用
├── favicon.ico           # 16x16, 32x32, 48x48
├── favicon.svg           # モダンブラウザ用
├── apple-touch-icon.png  # iOS用 180x180
└── og-image.png          # SNS共有用 1200x630
```

### カラーパレット

```css
/* Prism Brand Colors */
:root {
  --prism-primary: #667eea;
  --prism-secondary: #764ba2;
  --prism-accent: #f093fb;
  --prism-dark: #0f0f23;
  --prism-gradient: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}
```

### フォント

```
Primary: Inter (Google Fonts, 無料)
Monospace: JetBrains Mono (無料)
```

---

## 📊 ドメイン管理

### 更新リマインダー

```
Auto-renewal: ON（推奨）
Expiration: 2026年11月2日
Renewal notice: 30日前にメール通知
```

### Whois Privacy

```
Cloudflare: 無料で自動有効化
個人情報保護: ✅ 完全
```

### トランスファーロック

```
Transfer Lock: ON
不正移管防止: ✅
```

---

## 🚀 チェックリスト

- [ ] Cloudflareアカウント作成
- [ ] prism.dev 購入（$10/年）
- [ ] DNS Cloudflare設定
- [ ] Vercel カスタムドメイン追加
- [ ] SSL/TLS Full (strict)
- [ ] Email Routing設定（support@prism.dev）
- [ ] ブランディング資産作成
- [ ] HTTPSアクセス確認

---

**次のステップ**: Supabase無料プロジェクト作成 → DB Schema実装

**総コスト**: $10/年（ドメインのみ）

