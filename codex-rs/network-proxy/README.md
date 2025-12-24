# codex-network-proxy

`codex-network-proxy` is Codex's local network policy enforcement proxy. It runs:

- an HTTP proxy (default `127.0.0.1:3128`)
- a SOCKS5 proxy (default `127.0.0.1:8081`)
- an admin HTTP API (default `127.0.0.1:8080`)

It enforces an allow/deny policy and a "limited" mode intended for read-only network access.

## Quickstart

### 1) Configure

`codex-network-proxy` reads from Codex's merged `config.toml` (via `codex-core` config loading).

Example config:

```toml
[network_proxy]
enabled = true
proxy_url = "http://127.0.0.1:3128"
admin_url = "http://127.0.0.1:8080"
mode = "limited" # or "full"

[network_proxy.policy]
# If allowed_domains is non-empty, hosts must match it (unless denied).
allowed_domains = ["*.openai.com"]
denied_domains = ["evil.example"]

# If false, loopback (localhost/127.0.0.1/::1) is rejected unless explicitly allowlisted.
allow_local_binding = false

# macOS-only: allows proxying to a unix socket when request includes `x-unix-socket: /path`.
allow_unix_sockets = ["/tmp/example.sock"]

[network_proxy.mitm]
# Enables CONNECT MITM for limited-mode HTTPS. If disabled, CONNECT is blocked in limited mode.
enabled = true

# When true, logs request/response body sizes (up to max_body_bytes).
inspect = false
max_body_bytes = 4096

# These are relative to the directory containing config.toml when relative.
ca_cert_path = "network_proxy/mitm/ca.pem"
ca_key_path = "network_proxy/mitm/ca.key"
```

### 2) Initialize MITM directories (optional)

This ensures the MITM directory exists (and is a good smoke test that the binary runs):

```bash
cargo run -p codex-network-proxy -- init
```

### 3) Run the proxy

```bash
cargo run -p codex-network-proxy --
```

### 4) Point a client at it

For HTTP(S) traffic:

```bash
export HTTP_PROXY="http://127.0.0.1:3128"
export HTTPS_PROXY="http://127.0.0.1:3128"
```

For SOCKS5 traffic:

```bash
export ALL_PROXY="socks5://127.0.0.1:8081"
```

### 5) Understand blocks / debugging

When a request is blocked, the proxy responds with `403` and includes:

- `x-proxy-error`: one of:
  - `blocked-by-allowlist`
  - `blocked-by-denylist`
  - `blocked-by-method-policy`
  - `blocked-by-mitm-required`
  - `blocked-by-policy`

In "limited" mode, only `GET`, `HEAD`, and `OPTIONS` are allowed. In addition, HTTPS `CONNECT`
requires MITM to be enabled to allow read-only HTTPS; otherwise the proxy blocks CONNECT with
reason `mitm_required`.

## Admin API

The admin API is a small HTTP server intended for debugging and runtime adjustments.

Endpoints:

```bash
curl -sS http://127.0.0.1:8080/health
curl -sS http://127.0.0.1:8080/config
curl -sS http://127.0.0.1:8080/patterns
curl -sS http://127.0.0.1:8080/blocked

# Switch modes without restarting:
curl -sS -X POST http://127.0.0.1:8080/mode -d '{"mode":"full"}'

# Force a config reload:
curl -sS -X POST http://127.0.0.1:8080/reload
```

## Platform notes

- Unix socket proxying via the `x-unix-socket` header is **macOS-only**; other platforms will
  reject unix socket requests.

