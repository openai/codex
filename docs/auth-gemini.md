# Gemini Authentication Guide

## Overview

Codex supports two authentication methods for Google's Gemini models:
1. **API Key** (Google AI Studio) - Simple, recommended for development
2. **OAuth 2.0** (Vertex AI) - Enterprise, supports Google Account authentication

## Authentication Modes

### API Key Mode (Google AI Studio)

**Best for**: Development, testing, personal use

**Setup**:

1. Get your API key from [Google AI Studio](https://makersuite.google.com/app/apikey)

2. Set environment variable:
   ```bash
   export GEMINI_API_KEY=your-api-key-here
   # or
   export GOOGLE_AI_STUDIO_API_KEY=your-api-key-here
   ```

3. Or add to `.codex/config.toml`:
   ```toml
   [auth.gemini]
   mode = "api-key"
   provider = "ai_studio"
   ```

**Usage**:
- API key is attached as `x-goog-api-key` header
- Direct access to Generative Language API
- No additional authentication steps required

### OAuth 2.0 Mode (Vertex AI)

**Best for**: Production, enterprise, team use

**Requirements**:
- Google Cloud Project with Vertex AI API enabled
- OAuth 2.0 Client ID configured
- Appropriate IAM permissions

**Setup**:

1. **Create Google Cloud Project**:
   - Go to [Google Cloud Console](https://console.cloud.google.com/)
   - Create or select a project
   - Enable Vertex AI API

2. **Configure OAuth 2.0**:
   - Navigate to APIs & Services > Credentials
   - Create OAuth 2.0 Client ID (Desktop application)
   - Download credentials or note Client ID

3. **Set environment variables**:
   ```bash
   export GOOGLE_OAUTH_CLIENT_ID=your-client-id.apps.googleusercontent.com
   export GCP_PROJECT_ID=your-gcp-project-id
   export VERTEX_REGION=us-central1
   ```

4. **Or configure in `.codex/config.toml`**:
   ```toml
   [auth.gemini]
   mode = "oauth"
   provider = "vertex"
   project = "your-gcp-project-id"
   region = "us-central1"
   prefer_cli = true  # Use geminicli if available
   ```

**Authentication Flow**:

```bash
# Login (opens browser for Google Account authentication)
codex auth gemini login

# Check status
codex auth gemini status

# Logout
codex auth gemini logout
```

## CLI Commands

### `codex auth gemini login`

Initiates OAuth 2.0 PKCE flow:

1. Generates PKCE code verifier and challenge
2. Opens browser to Google's OAuth consent screen
3. Starts local loopback server (127.0.0.1:0)
4. Exchanges authorization code for tokens
5. Stores tokens securely

**Options**:
```bash
# Force use of geminicli (if installed)
codex auth gemini login --use-cli

# Force use of internal PKCE flow
codex auth gemini login --no-cli
```

**Security Features**:
- PKCE (Proof Key for Code Exchange) prevents authorization code interception
- CSRF state parameter validates OAuth callback
- Loopback-only server (127.0.0.1, no network exposure)
- 5-minute authentication timeout
- Secrets redacted from logs

### `codex auth gemini status`

Shows current authentication state:

```bash
codex auth gemini status
```

**Output**:
```
Gemini Authentication Status
  Mode: oauth
  Provider: vertex
  Source: keyring
  Credentials: ****...****  (masked)
  Expires: 2024-12-31 23:59:59 UTC
  Project: your-gcp-project-id
  Region: us-central1
```

### `codex auth gemini logout`

Logs out and clears stored credentials:

```bash
codex auth gemini logout
```

This will:
- Revoke OAuth tokens (if possible)
- Clear credentials from keyring/file storage
- Reset authentication state

## Credential Storage

### Secure Storage Priority

1. **OS Keyring** (preferred):
   - macOS: Keychain
   - Windows: Credential Manager
   - Linux: Secret Service (libsecret)

2. **File Fallback** (`.codex/credentials.json`):
   - Permissions: 0600 (owner read/write only)
   - Warning emitted when using file storage
   - Encrypted at rest (AES-256-GCM)

### Storage Location

Check where credentials are stored:

```bash
codex auth gemini status
# Shows "Source: keyring" or "Source: file"
```

## geminicli Integration

### Automatic Detection

If `geminicli` is installed, Codex will:
1. Detect geminicli availability
2. Prefer geminicli's OAuth flow (if `prefer_cli = true`)
3. Fall back to internal PKCE on failure

### Configuration

```toml
[auth.gemini]
prefer_cli = true  # Default: prefer geminicli if available
```

**Override**:
```bash
# Force use internal flow
codex auth gemini login --no-cli

# Force use geminicli
codex auth gemini login --use-cli
```

### Detection Logic

```rust
// Pseudo-code
if prefer_cli {
    if geminicli_installed() {
        try_geminicli_auth()
    } else {
        fallback_to_internal_pkce()
    }
} else {
    use_internal_pkce()
}
```

## OAuth 2.0 Scopes

### Vertex AI

Minimum required scope:
```
https://www.googleapis.com/auth/cloud-platform
```

### Generative Language API (AI Studio)

OAuth is supported but **API key is preferred** for AI Studio. If using OAuth:
```
https://www.googleapis.com/auth/generative-language
```

## Configuration Reference

### Complete Configuration Example

```toml
[auth.gemini]
# Authentication mode: "api-key" or "oauth"
mode = "api-key"

# Provider: "ai_studio" or "vertex"
provider = "ai_studio"

# Prefer geminicli for OAuth (default: true)
prefer_cli = true

# Required for Vertex AI
project = ""
region = ""

# Optional: OAuth client credentials
# (Usually set via environment variables)
# client_id = ""
# client_secret = ""
```

### Environment Variables

Priority: **Environment > Config File**

```bash
# API Key mode
export GEMINI_API_KEY=<key>
export GOOGLE_AI_STUDIO_API_KEY=<key>

# OAuth mode
export GOOGLE_OAUTH_CLIENT_ID=<client-id>
export GCP_PROJECT_ID=<project>
export VERTEX_REGION=<region>
```

## Security Best Practices

### 1. Never Commit Secrets

Add to `.gitignore`:
```gitignore
.codex/credentials.json
.codex/secret
.env
```

### 2. Use Keyring Storage

Prefer OS keyring over file storage:
```toml
[auth]
credentials_store_mode = "keyring"
```

### 3. Rotate Credentials Regularly

```bash
# Logout and login again
codex auth gemini logout
codex auth gemini login
```

### 4. Use Least Privilege

Grant minimum required IAM roles:
- `roles/aiplatform.user` (Vertex AI)
- Avoid `roles/owner` or `roles/editor`

### 5. Monitor OAuth Tokens

- Check token expiration: `codex auth gemini status`
- Revoke unused tokens in Google Cloud Console
- Set up token expiration policies

## Troubleshooting

### "API Key not found"

**Solution**:
```bash
export GEMINI_API_KEY=your-key
# or add to .codex/config.toml
```

### "OAuth login failed"

**Causes**:
1. Client ID not configured
2. Redirect URI not whitelisted
3. Project lacks Vertex AI API

**Solution**:
```bash
# Check configuration
codex auth gemini status

# Verify environment variables
echo $GOOGLE_OAUTH_CLIENT_ID
echo $GCP_PROJECT_ID

# Re-run login
codex auth gemini login
```

### "geminicli not found"

**Non-issue**: Codex will fall back to internal PKCE automatically.

**To install geminicli** (optional):
```bash
# Follow geminicli installation instructions
# https://github.com/google/generative-ai-cli
```

### "Permission denied" (Keyring)

**Cause**: OS keyring access denied.

**Solution**:
1. Grant keyring access when prompted
2. Or use file storage (less secure):
   ```toml
   [auth]
   credentials_store_mode = "file"
   ```

### Token Expired

**Symptoms**:
- API calls return 401 Unauthorized
- `codex auth gemini status` shows expired token

**Solution**:
```bash
codex auth gemini logout
codex auth gemini login
```

## Backward Compatibility

Existing `GEMINI_API_KEY` users:
- ✅ No changes required
- ✅ Behavior remains identical
- ✅ OAuth is opt-in only

Default mode:
- API Key if `GEMINI_API_KEY` is set
- OAuth only if explicitly configured

---

**日本語版 / Japanese Version**

## 概要

CodexはGoogleのGeminiモデルに対して2つの認証方法をサポートしています：
1. **APIキー**（Google AI Studio）- シンプル、開発に推奨
2. **OAuth 2.0**（Vertex AI）- エンタープライズ、Googleアカウント認証をサポート

## 認証モード

### APIキーモード（Google AI Studio）

**最適な用途**: 開発、テスト、個人使用

**セットアップ**:

1. [Google AI Studio](https://makersuite.google.com/app/apikey)からAPIキーを取得

2. 環境変数を設定:
   ```bash
   export GEMINI_API_KEY=your-api-key-here
   # または
   export GOOGLE_AI_STUDIO_API_KEY=your-api-key-here
   ```

### OAuth 2.0モード（Vertex AI）

**最適な用途**: 本番環境、エンタープライズ、チーム使用

**要件**:
- Vertex AI APIが有効なGoogle Cloudプロジェクト
- OAuth 2.0クライアントIDの設定
- 適切なIAM権限

**セットアップ**:

1. **Google Cloudプロジェクトを作成**:
   - [Google Cloud Console](https://console.cloud.google.com/)へ移動
   - プロジェクトを作成または選択
   - Vertex AI APIを有効化

2. **環境変数を設定**:
   ```bash
   export GOOGLE_OAUTH_CLIENT_ID=your-client-id.apps.googleusercontent.com
   export GCP_PROJECT_ID=your-gcp-project-id
   export VERTEX_REGION=us-central1
   ```

## CLIコマンド

### `codex auth gemini login`

OAuth 2.0 PKCEフローを開始します。

### `codex auth gemini status`

現在の認証状態を表示します。

### `codex auth gemini logout`

ログアウトして保存された認証情報をクリアします。

## 後方互換性

既存の`GEMINI_API_KEY`ユーザー：
- ✅ 変更不要
- ✅ 動作は同一のまま
- ✅ OAuthはオプトインのみ
