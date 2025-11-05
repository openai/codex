# Stripe決済システムセットアップガイド

**目的**: Codex Cloud SaaSの決済処理システムを構築

**最終更新**: 2025年11月2日

---

## 📋 概要

Stripeを使用してCodex Pro/Team/Enterpriseティアのサブスクリプション決済を処理します。

### 料金プラン

| Tier | 価格 | 請求サイクル | Stripe Product ID |
|------|------|------------|------------------|
| Pro | $15/月 | Monthly | `prod_pro_monthly` |
| Pro Annual | $144/年 | Annual (20% off) | `prod_pro_annual` |
| Team | $50/月 | Monthly | `prod_team_monthly` |
| Team Annual | $480/年 | Annual (20% off) | `prod_team_annual` |
| Enterprise | Custom | Custom | Contact Sales |

---

## 🚀 Phase 1: Stripeアカウント作成

### 1.1 アカウント登録

1. https://dashboard.stripe.com/register にアクセス
2. メールアドレス、パスワード、会社名を入力
3. メール確認リンクをクリック

### 1.2 ビジネス情報登録

**重要**: 本番環境でペイアウトを受けるために必須

```
会社名: Codex Inc. (or your entity name)
ビジネスタイプ: Software as a Service (SaaS)
業種: Computer Software
所在地: (your address)
Tax ID/EIN: (your tax ID)
```

### 1.3 銀行口座接続

- Dashboard → Settings → Bank accounts and scheduling
- 銀行口座情報を追加（口座番号、ルーティング番号）
- マイクロデポジットで確認（2-3営業日）

---

## 🔧 Phase 2: 製品とプラン作成

### 2.1 テストモード vs 本番モード

Stripeには2つのモード:
- **テストモード**: 開発・テスト用（テストカード使用）
- **本番モード**: 実際の決済処理

**最初はテストモードで開発し、後で本番モードに移行**

### 2.2 製品作成

Dashboard → Products → Add product

#### Pro Tier (Monthly)

```
Product Name: Codex Pro (Monthly)
Description: Professional AI coding assistant with kernel optimization
Pricing Model: Recurring
Price: $15.00 USD
Billing Period: Monthly
Tax Behavior: Taxable (digital services)
```

#### Pro Tier (Annual)

```
Product Name: Codex Pro (Annual)
Description: Professional AI coding assistant - Annual plan (20% off)
Pricing Model: Recurring
Price: $144.00 USD ($12/month billed annually)
Billing Period: Yearly
Tax Behavior: Taxable (digital services)
```

#### Team Tier (Monthly)

```
Product Name: Codex Team (Monthly)
Description: Team collaboration features for up to 5 users
Pricing Model: Recurring
Price: $50.00 USD
Billing Period: Monthly
Metadata:
  - base_seats: 5
  - additional_seat_price: 10
Tax Behavior: Taxable (digital services)
```

#### Team Tier (Annual)

```
Product Name: Codex Team (Annual)
Description: Team collaboration - Annual plan (20% off)
Pricing Model: Recurring
Price: $480.00 USD ($40/month billed annually)
Billing Period: Yearly
Tax Behavior: Taxable (digital services)
```

### 2.3 価格ID取得

各製品作成後、Price IDをメモ:

```bash
# Example Price IDs (replace with actual)
STRIPE_PRICE_PRO_MONTHLY=price_1ABC...
STRIPE_PRICE_PRO_ANNUAL=price_2DEF...
STRIPE_PRICE_TEAM_MONTHLY=price_3GHI...
STRIPE_PRICE_TEAM_ANNUAL=price_4JKL...
```

---

## 🔑 Phase 3: API Keys取得

### 3.1 テストキー

Dashboard → Developers → API keys (Test mode toggle ON)

```bash
# Test Keys
STRIPE_TEST_PUBLISHABLE_KEY=pk_test_51...
STRIPE_TEST_SECRET_KEY=sk_test_51...
```

### 3.2 本番キー

Dashboard → Developers → API keys (Test mode toggle OFF)

```bash
# Live Keys (keep secret!)
STRIPE_LIVE_PUBLISHABLE_KEY=pk_live_51...
STRIPE_LIVE_SECRET_KEY=sk_live_51...
```

### 3.3 環境変数設定

```bash
# .env file (never commit!)
STRIPE_SECRET_KEY=sk_test_51... # or sk_live_51... for production
STRIPE_PUBLISHABLE_KEY=pk_test_51... # or pk_live_51... for production
STRIPE_WEBHOOK_SECRET=whsec_... # from webhook setup
```

---

## 🌐 Phase 4: Checkout Integration

### 4.1 Stripe Checkout (推奨)

最も簡単な統合方法。Stripeがホストするチェックアウトページ。

#### バックエンド (Rust + axum)

```rust
// Cargo.toml
[dependencies]
stripe = "0.26"
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

```rust
use stripe::{
    Client, CheckoutSession, CheckoutSessionMode,
    CreateCheckoutSession, CreateCheckoutSessionLineItems,
};
use axum::{
    extract::Json,
    response::{IntoResponse, Redirect},
    Router, routing::post,
};

#[derive(serde::Deserialize)]
struct CreateCheckoutRequest {
    price_id: String,
    customer_email: String,
}

async fn create_checkout_session(
    Json(req): Json<CreateCheckoutRequest>,
) -> impl IntoResponse {
    let client = Client::new(std::env::var("STRIPE_SECRET_KEY").unwrap());
    
    let mut params = CreateCheckoutSession::new();
    params.mode = Some(CheckoutSessionMode::Subscription);
    params.customer_email = Some(&req.customer_email);
    params.line_items = Some(vec![CreateCheckoutSessionLineItems {
        price: Some(req.price_id),
        quantity: Some(1),
        ..Default::default()
    }]);
    params.success_url = Some("https://codex.ai/success?session_id={CHECKOUT_SESSION_ID}");
    params.cancel_url = Some("https://codex.ai/pricing");
    
    match CheckoutSession::create(&client, params).await {
        Ok(session) => {
            // Return checkout URL to client
            Json(serde_json::json!({
                "checkout_url": session.url
            })).into_response()
        }
        Err(e) => {
            eprintln!("Stripe error: {}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Error").into_response()
        }
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/create-checkout-session", post(create_checkout_session));
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

#### フロントエンド (JavaScript)

```javascript
// pricing page
document.querySelectorAll('[data-price-id]').forEach(button => {
    button.addEventListener('click', async (e) => {
        const priceId = e.target.dataset.priceId;
        const email = getUserEmail(); // Get from auth
        
        const response = await fetch('/create-checkout-session', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ 
                price_id: priceId,
                customer_email: email
            })
        });
        
        const { checkout_url } = await response.json();
        window.location.href = checkout_url; // Redirect to Stripe
    });
});
```

### 4.2 Success Page

```html
<!-- success.html -->
<!DOCTYPE html>
<html>
<head>
    <title>Payment Successful - Codex</title>
</head>
<body>
    <h1>🎉 Welcome to Codex Pro!</h1>
    <p>Your subscription is now active.</p>
    <p>Session ID: <span id="session-id"></span></p>
    <a href="/dashboard">Go to Dashboard</a>
    
    <script>
        const urlParams = new URLSearchParams(window.location.search);
        const sessionId = urlParams.get('session_id');
        document.getElementById('session-id').textContent = sessionId;
        
        // Optionally verify session on backend
        fetch(`/verify-session?session_id=${sessionId}`)
            .then(res => res.json())
            .then(data => {
                console.log('Subscription confirmed:', data);
            });
    </script>
</body>
</html>
```

---

## 🔔 Phase 5: Webhook設定

### 5.1 Webhookエンドポイント作成

```rust
use stripe::{Event, EventObject, EventType};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};

async fn stripe_webhook(
    State(webhook_secret): State<String>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let payload = std::str::from_utf8(&body).unwrap();
    let signature = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    match stripe::Webhook::construct_event(payload, signature, &webhook_secret) {
        Ok(event) => {
            match event.type_ {
                EventType::CheckoutSessionCompleted => {
                    // Handle successful checkout
                    println!("✅ Checkout completed!");
                    // 1. Create user account
                    // 2. Send welcome email
                    // 3. Grant access to Pro features
                }
                EventType::CustomerSubscriptionCreated => {
                    println!("✅ Subscription created!");
                }
                EventType::CustomerSubscriptionUpdated => {
                    println!("🔄 Subscription updated!");
                }
                EventType::CustomerSubscriptionDeleted => {
                    println!("❌ Subscription canceled!");
                    // Revoke access
                }
                EventType::InvoicePaymentSucceeded => {
                    println!("💰 Payment succeeded!");
                }
                EventType::InvoicePaymentFailed => {
                    println!("❌ Payment failed!");
                    // Send payment retry email
                }
                _ => {
                    println!("Unhandled event type: {:?}", event.type_);
                }
            }
            StatusCode::OK
        }
        Err(e) => {
            eprintln!("Webhook error: {}", e);
            StatusCode::BAD_REQUEST
        }
    }
}
```

### 5.2 Stripe Dashboard設定

1. Dashboard → Developers → Webhooks → Add endpoint
2. Endpoint URL: `https://api.codex.ai/stripe/webhook`
3. Events to listen:
   - `checkout.session.completed`
   - `customer.subscription.created`
   - `customer.subscription.updated`
   - `customer.subscription.deleted`
   - `invoice.payment_succeeded`
   - `invoice.payment_failed`
4. Webhook secretをコピー: `whsec_...`

### 5.3 ローカルテスト

```bash
# Stripe CLIインストール
brew install stripe/stripe-cli/stripe
# or
scoop install stripe

# Stripeにログイン
stripe login

# Webhookをローカルにフォワード
stripe listen --forward-to localhost:3000/stripe/webhook

# 別のターミナルでテストイベント送信
stripe trigger checkout.session.completed
```

---

## 💳 Phase 6: テストカード

### 6.1 成功するテストカード

```
Card Number: 4242 4242 4242 4242
Expiry: Any future date (e.g., 12/25)
CVC: Any 3 digits (e.g., 123)
ZIP: Any 5 digits (e.g., 12345)
```

### 6.2 失敗シナリオのテストカード

```
# Payment declined
4000 0000 0000 0002

# Insufficient funds
4000 0000 0000 9995

# Card expired
4000 0000 0000 0069

# Processing error
4000 0000 0000 0119

# 3D Secure required
4000 0025 0000 3155
```

---

## 🔐 Phase 7: セキュリティ

### 7.1 APIキー保護

```bash
# ✅ GOOD: 環境変数
export STRIPE_SECRET_KEY=sk_live_...

# ❌ BAD: ソースコードに直接記述
const apiKey = "sk_live_..."; // NEVER DO THIS!
```

### 7.2 Webhook署名検証

**必須**: すべてのWebhookリクエストで署名検証

```rust
// Always verify webhook signature
match stripe::Webhook::construct_event(payload, signature, &webhook_secret) {
    Ok(event) => { /* process */ }
    Err(_) => return StatusCode::BAD_REQUEST, // Reject invalid signature
}
```

### 7.3 HTTPS必須

- 本番環境では必ずHTTPS使用
- Let's Encryptで無料SSL証明書

---

## 📊 Phase 8: ダッシュボード監視

### 8.1 重要メトリクス

Dashboard → Home

- **MRR** (Monthly Recurring Revenue): 月次経常収益
- **Churn Rate**: 解約率
- **Failed Payments**: 失敗した決済
- **New Subscriptions**: 新規登録数

### 8.2 アラート設定

Dashboard → Settings → Email notifications

有効化推奨:
- ✅ Successful payments
- ✅ Failed payments
- ✅ Disputes opened
- ✅ Large charges (>$1000)

---

## 🧪 Phase 9: テストチェックリスト

- [ ] Pro Monthly購読テスト
- [ ] Pro Annual購読テスト
- [ ] Team購読テスト
- [ ] 決済成功フロー
- [ ] 決済失敗ハンドリング
- [ ] Webhook受信確認
- [ ] サブスクリプション更新
- [ ] サブスクリプションキャンセル
- [ ] 請求書生成
- [ ] 返金処理

---

## 🚀 Phase 10: 本番環境移行

### 10.1 チェックリスト

- [ ] ビジネス情報完全登録
- [ ] 銀行口座認証完了
- [ ] 本番APIキー取得
- [ ] Webhook本番エンドポイント設定
- [ ] 税金設定（必要に応じて）
- [ ] 利用規約・プライバシーポリシー準備
- [ ] カスタマーサポートメール設定
- [ ] テストモードで完全テスト済み

### 10.2 本番モードへ切り替え

1. Dashboard右上のToggleを"Live"に変更
2. 環境変数を本番キーに更新
3. Webhookエンドポイントを本番URLに設定
4. 最初のテスト決済実行（自分のカードで）

---

## 📚 追加リソース

### 公式ドキュメント

- Stripe API: https://stripe.com/docs/api
- Checkout Session: https://stripe.com/docs/payments/checkout
- Subscriptions: https://stripe.com/docs/billing/subscriptions/overview
- Webhooks: https://stripe.com/docs/webhooks

### Rustライブラリ

- stripe-rs: https://github.com/arlyon/async-stripe

### サポート

- Stripe Support: https://support.stripe.com
- Discord: Stripeコミュニティ

---

## 💡 ベストプラクティス

1. **常にテストモードから開始**
2. **Webhook署名を必ず検証**
3. **APIキーを絶対にコミットしない**
4. **失敗した決済を監視＆リトライ**
5. **明確な請求明細を提供**
6. **簡単なキャンセルプロセス**
7. **返金ポリシーを明確に**

---

**次のステップ**: API設計書作成 → AWS GPU クラスター見積もり

