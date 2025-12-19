# Codex Network Proxy Quickstart (Local)

This is a compact guide to build and validate the Codex network proxy locally.

## Build

From the Codex repo:

```bash
cd /Users/viyatb/code/codex/codex-rs
cargo build -p codex-network-proxy
```

For MITM support:

```bash
cargo build -p codex-network-proxy --features mitm
```

## Configure

Add this to `~/.codex/config.toml`:

```toml
[network_proxy]
enabled = true
proxy_url = "http://127.0.0.1:3128"
admin_url = "http://127.0.0.1:8080"
mode = "limited" # or "full"
prompt_on_block = true
poll_interval_ms = 1000

[network_proxy.policy]
allowed_domains = ["example.com", "*.github.com"]
denied_domains = ["metadata.google.internal", "169.254.*"]

[network_proxy.mitm]
enabled = false
```

## Run the proxy

```bash
cd /Users/viyatb/code/codex/codex-rs
cargo run -p codex-network-proxy -- proxy
```

With MITM:

```bash
cargo run -p codex-network-proxy --features mitm -- proxy
```

## Test with curl

HTTP/HTTPS via proxy:

```bash
export HTTP_PROXY="http://127.0.0.1:3128"
export HTTPS_PROXY="http://127.0.0.1:3128"
curl -sS https://example.com
```

Limited mode + HTTPS requires MITM. If MITM is on, trust the generated CA:

```bash
security add-trusted-cert -d -r trustRoot \
  -k ~/Library/Keychains/login.keychain-db \
  ~/.codex/network_proxy/mitm/ca.pem
```

Or pass the CA directly:

```bash
curl --cacert ~/.codex/network_proxy/mitm/ca.pem -sS https://example.com
```

## Admin endpoints

Reload config after edits:

```bash
curl -fsS -X POST http://127.0.0.1:8080/reload
```

Switch modes:

```bash
curl -fsS -X POST http://127.0.0.1:8080/mode -d '{"mode":"full"}'
```

## Codex integration sanity check

1) Start the proxy.  
2) Launch Codex with the proxy enabled in config.  
3) Run a network command (e.g., `curl https://example.com`).  
4) Confirm you see the allow/deny prompt and that the proxy logs reflect the decision.
