# codex-network-proxy

`codex-network-proxy` is Codex's local network policy enforcement proxy. It runs:

- an HTTP proxy (default `127.0.0.1:3128`)
- an optional SOCKS5 proxy (default `127.0.0.1:8081`, disabled by default)
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
# Optional SOCKS5 listener (disabled by default).
enable_socks5 = false
socks_url = "http://127.0.0.1:8081"
enable_socks5_udp = false
# When `enabled` is false, the proxy no-ops and does not bind listeners.
# When true, respect HTTP(S)_PROXY/ALL_PROXY for upstream requests (HTTP(S) proxies only),
# including CONNECT tunnels in full mode.
allow_upstream_proxy = false
# By default, non-loopback binds are clamped to loopback for safety.
# If you want to expose these listeners beyond localhost, you must opt in explicitly.
dangerously_allow_non_loopback_proxy = false
dangerously_allow_non_loopback_admin = false
mode = "full" # default when unset; use "limited" for read-only mode

[network_proxy.policy]
# Hosts must match the allowlist (unless denied).
# If `allowed_domains` is empty, the proxy blocks requests until an allowlist is configured.
allowed_domains = ["*.openai.com"]
denied_domains = ["evil.example"]

# If false, local/private networking is rejected. Explicit allowlisting of local IP literals
# (or `localhost`) is required to permit them.
# Hostnames that resolve to local/private IPs are still blocked even if allowlisted.
allow_local_binding = false

# macOS-only: allows proxying to a unix socket when request includes `x-unix-socket: /path`.
allow_unix_sockets = ["/tmp/example.sock"]
```

### 2) Run the proxy

```bash
cargo run -p codex-network-proxy --
```

### 3) Point a client at it

For HTTP(S) traffic:

```bash
export HTTP_PROXY="http://127.0.0.1:3128"
export HTTPS_PROXY="http://127.0.0.1:3128"
```

For SOCKS5 traffic (when `enable_socks5 = true`):

```bash
export ALL_PROXY="socks5h://127.0.0.1:8081"
```

### 4) Understand blocks / debugging

When a request is blocked, the proxy responds with `403` and includes:

- `x-proxy-error`: one of:
  - `blocked-by-allowlist`
  - `blocked-by-denylist`
  - `blocked-by-method-policy`
  - `blocked-by-policy`

In "limited" mode, only `GET`, `HEAD`, and `OPTIONS` are allowed. HTTPS `CONNECT` and SOCKS5 are
blocked because they would bypass method enforcement.

## Library API

`codex-network-proxy` can be embedded as a library with a thin API:

```rust
use codex_network_proxy::{NetworkProxy, NetworkDecision, NetworkPolicyRequest};

let proxy = NetworkProxy::builder()
    .http_addr("127.0.0.1:8080".parse()?)
    .admin_addr("127.0.0.1:9000".parse()?)
    .policy_decider(|request: NetworkPolicyRequest| async move {
        // Example: auto-allow when exec policy already approved a command prefix.
        if let Some(command) = request.command.as_deref() {
            if command.starts_with("curl ") {
                return NetworkDecision::Allow;
            }
        }
        NetworkDecision::Deny {
            reason: "policy_denied".to_string(),
        }
    })
    .build()
    .await?;

let handle = proxy.run().await?;
handle.shutdown().await?;
```

When unix socket proxying is enabled, HTTP/admin bind overrides are still clamped to loopback
to avoid turning the proxy into a remote bridge to local daemons.

### Policy hook (exec-policy mapping)

The proxy exposes a policy hook (`NetworkPolicyDecider`) that can override allowlist-only blocks.
It receives `command` and `exec_policy_hint` fields when supplied by the embedding app. This lets
core map exec approvals to network access, e.g. if a user already approved `curl *` for a session,
the decider can auto-allow network requests originating from that command.

**Important:** Explicit deny rules still win. The decider only gets a chance to override
`not_allowed` (allowlist misses), not `denied` or `not_allowed_local`.

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
- HTTPS tunneling uses BoringSSL via Rama's `rama-tls-boring`; building the proxy requires a
  native toolchain and CMake on macOS/Linux/Windows.

## Security notes (important)

This section documents the protections implemented by `codex-network-proxy`, and the boundaries of
what it can reasonably guarantee.

- Allowlist-first policy: if `allowed_domains` is empty, requests are blocked until an allowlist is configured.
- Deny wins: entries in `denied_domains` always override the allowlist.
- Local/private network protection: when `allow_local_binding = false`, the proxy blocks loopback
  and common private/link-local ranges. Explicit allowlisting of local IP literals (or `localhost`)
  is required to permit them; hostnames that resolve to local/private IPs are still blocked even if
  allowlisted (best-effort DNS lookup).
- Limited mode enforcement:
  - only `GET`, `HEAD`, and `OPTIONS` are allowed
  - HTTPS `CONNECT` remains a tunnel; limited-mode method enforcement does not apply to HTTPS
- Listener safety defaults:
  - the admin API is unauthenticated; non-loopback binds are clamped unless explicitly enabled via
    `dangerously_allow_non_loopback_admin`
- the HTTP proxy listener similarly clamps non-loopback binds unless explicitly enabled via
    `dangerously_allow_non_loopback_proxy`
- when unix socket proxying is enabled, both listeners are forced to loopback to avoid turning the
    proxy into a remote bridge into local daemons.
- `enabled` is enforced at runtime; when false the proxy no-ops and does not bind listeners.
Limitations:

- DNS rebinding is hard to fully prevent without pinning the resolved IP(s) all the way down to the
  transport layer. If your threat model includes hostile DNS, enforce network egress at a lower
  layer too (e.g., firewall / VPC / corporate proxy policies).
