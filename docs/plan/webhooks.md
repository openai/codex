# Webhook Integration Guide

**Version**: 0.57.0

---

## Overview

plan mode can send webhook notifications to GitHub, Slack, or any HTTP endpoint when Plan events occur.

**Supported Events**:
- `Plan.created` - New Plan created
- `Plan.approved` - Plan approved for execution
- `Plan.rejected` - Plan rejected
- `Plan.exported` - Plan exported to file
- `exec.start` - Execution started
- `exec.result` - Execution completed

---

## GitHub Integration

### Setup

1. **Generate Personal Access Token** (PAT):
   - Go to GitHub Settings → Developer settings → Personal access tokens
   - Create token with `repo:status` scope

2. **Configure Codex**:

   ```json
   {
     "codex.webhooks.github.enabled": true
   }
   ```

3. **Set Webhook URL & Secret**:

   Via environment:
   ```bash
   export CODEX_GITHUB_WEBHOOK_URL="https://api.github.com/repos/user/repo/statuses/{sha}"
   export CODEX_GITHUB_WEBHOOK_SECRET="your-webhook-secret"
   ```

### Payload Format

```json
{
  "context": "codex/Plan",
  "state": "success",
  "description": "Plan bp-123 approved",
  "target_url": "https://github.com/zapabob/codex/Plans/bp-123",
  "Plan_id": "bp-123",
  "title": "Add Telemetry Feature",
  "timestamp": "2025-11-02T12:00:00Z"
}
```

### States

| Plan State | GitHub Status |
|----------------|---------------|
| `pending` | `pending` |
| `approved` | `success` |
| `rejected` | `failure` |

---

## Slack Integration

### Setup

1. **Create Incoming Webhook**:
   - Go to Slack App Directory → Incoming Webhooks
   - Add to your workspace
   - Copy webhook URL

2. **Configure Codex**:

   ```json
   {
     "codex.webhooks.slack.enabled": true
   }
   ```

3. **Set Webhook URL & Secret**:

   ```bash
   export CODEX_SLACK_WEBHOOK_URL="https://hooks.slack.com/services/T00/B00/XXX"
   export CODEX_SLACK_WEBHOOK_SECRET="your-secret"
   ```

### Message Format

**Approved**:
> ✅ **Auth System Refactor**
> Plan approved by john.doe!
> 
> **Mode**: orchestrated
> **Artifacts**: docs/Plans/2025-11-02_refactor-auth.md

**Competition Result**:
> 🏆 **Optimize DB Query**
> Competition completed!
> 
> **Winner**: Variant A (Score: 95.6)
> **Artifacts**: Performance comparison table

---

## HTTP Generic

### Setup

1. **Configure Endpoint**:

   ```json
   {
     "codex.webhooks.enabled": true
   }
   ```

2. **Set URL & Secret**:

   ```bash
   export CODEX_WEBHOOK_URL="https://your-server.com/codex/webhook"
   export CODEX_WEBHOOK_SECRET="your-hmac-secret"
   ```

### Payload Format

```json
{
  "Plan_id": "bp-2025-11-02T12:00:00Z_add-logging",
  "title": "Add Request Logging",
  "state": "approved",
  "summary": "Plan approved by john.doe",
  "score": null,
  "timestamp": "2025-11-02T12:30:00Z",
  "mode": "orchestrated",
  "artifacts": [
    "docs/Plans/2025-11-02_add-logging.md",
    "api/middleware.py"
  ]
}
```

### Headers

```
Content-Type: application/json
X-Codex-Signature: sha256=abc123def456...
X-Codex-Event: Plan.approved
```

---

## Signature Verification

### Python

```python
import hmac
import hashlib

def verify_codex_signature(body: str, signature: str, secret: str) -> bool:
    """Verify HMAC-SHA256 signature from Codex webhook"""
    expected = hmac.new(
        secret.encode('utf-8'),
        body.encode('utf-8'),
        hashlib.sha256
    ).hexdigest()
    
    expected_sig = f"sha256={expected}"
    return hmac.compare_digest(expected_sig, signature)

# Usage
if not verify_codex_signature(request.body, request.headers['X-Codex-Signature'], SECRET):
    return 401  # Unauthorized
```

### Node.js

```javascript
const crypto = require('crypto');

function verifyCodexSignature(body, signature, secret) {
    const expected = crypto
        .createHmac('sha256', secret)
        .update(body)
        .digest('hex');
    
    const expectedSig = `sha256=${expected}`;
    return crypto.timingSafeEqual(
        Buffer.from(signature),
        Buffer.from(expectedSig)
    );
}
```

### Rust

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn verify_signature(body: &str, signature: &str, secret: &str) -> bool {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(body.as_bytes());
    
    let expected = hex::encode(mac.finalize().into_bytes());
    let expected_sig = format!("sha256={}", expected);
    
    // Constant-time comparison
    expected_sig == signature
}
```

---

## Retry Logic

### Behavior

- **Max Retries**: 3 (configurable)
- **Backoff**: Exponential (1s, 2s, 4s)
- **Timeout**: 10 seconds per attempt (configurable)

### Example Log

```
[INFO] Sending webhook to https://hooks.slack.com/...
[WARN] Webhook send failed (attempt 1): Connection timeout
[INFO] Retrying webhook after 1s (attempt 1)
[WARN] Webhook send failed (attempt 2): HTTP 503 Service Unavailable
[INFO] Retrying webhook after 2s (attempt 2)
[INFO] Webhook sent successfully to https://hooks.slack.com/...
```

---

## Configuration

### Full Config Example

```json
{
  "codex.webhooks.enabled": true,
  
  "codex.webhooks.github.enabled": true,
  "codex.webhooks.github.url": "https://api.github.com/repos/user/repo/statuses/{sha}",
  
  "codex.webhooks.slack.enabled": true,
  "codex.webhooks.slack.url": "https://hooks.slack.com/services/T00/B00/XXX",
  
  "codex.webhooks.http.enabled": true,
  "codex.webhooks.http.url": "https://your-server.com/webhook",
  "codex.webhooks.http.timeout": 10,
  "codex.webhooks.http.maxRetries": 3
}
```

### Environment Variables

Recommended: Store secrets in environment, not config files.

```bash
# GitHub
export CODEX_GITHUB_WEBHOOK_SECRET="gh-secret-123"

# Slack
export CODEX_SLACK_WEBHOOK_SECRET="slack-secret-456"

# HTTP Generic
export CODEX_WEBHOOK_SECRET="generic-secret-789"
```

---

## Testing Webhooks

### Test Script

```bash
# Dry-run mode (logs webhook payloads without sending)
codex --webhook-dry-run /Plan "Test" --mode=single
codex /approve bp-test

# Check logs
tail -f .codex/logs/webhooks.log
```

### Mock Endpoint

Use RequestBin or webhook.site for testing:

```bash
# Get test URL
curl https://webhook.site/token

# Configure
export CODEX_WEBHOOK_URL="https://webhook.site/your-unique-url"

# Send test
codex /Plan "Webhook test"
codex /approve bp-test

# View at https://webhook.site/your-unique-url
```

---

## Troubleshooting

### Webhook Not Sending

1. Check config:
   ```bash
   codex config get webhooks.enabled
   ```

2. Verify secret set:
   ```bash
   echo $CODEX_WEBHOOK_SECRET
   ```

3. Check logs:
   ```bash
   tail -f .codex/logs/orchestrator.log | grep webhook
   ```

### Signature Verification Fails

1. Ensure secret matches on both sides
2. Verify body is EXACTLY as sent (no modifications)
3. Use timing-safe comparison (`hmac.compare_digest` in Python)

### Connection Timeout

1. Increase timeout:
   ```json
   {"codex.webhooks.http.timeout": 30}
   ```

2. Check firewall/network

3. Verify URL is reachable:
   ```bash
   curl -X POST https://your-server.com/webhook
   ```

---

## See Also

- [Plan README](./README.md)
- [Slash Commands](./slash-commands.md)
- [Developer Documentation](./dev/architecture.md)

---

**Made with ❤️ by zapabob**

