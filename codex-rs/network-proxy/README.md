# codex-network-proxy

`codex-network-proxy` is Codex's local network policy enforcement proxy. It runs:

- an HTTP proxy (default `127.0.0.1:3128`)
- a SOCKS5 proxy (default `127.0.0.1:8081`, enabled by default)

It enforces an allow/deny policy, a "limited" mode intended for read-only network access, and
host-specific HTTPS MITM hooks for request matching and header injection.

## Quickstart

### 1) Configure

`codex-network-proxy` reads from Codex's merged `config.toml` (via `codex-core` config loading).

Network settings live under the selected permissions profile. Example config:

```toml
default_permissions = "workspace"

[permissions.workspace.network]
enabled = true
proxy_url = "http://127.0.0.1:3128"
# SOCKS5 listener (enabled by default).
enable_socks5 = true
socks_url = "http://127.0.0.1:8081"
enable_socks5_udp = true
# When `enabled` is false, the proxy no-ops and does not bind listeners.
# When true, respect HTTP(S)_PROXY/ALL_PROXY for upstream requests (HTTP(S) proxies only),
# including CONNECT tunnels in full mode.
allow_upstream_proxy = true
# By default, non-loopback binds are clamped to loopback for safety.
# If you want to expose these listeners beyond localhost, you must opt in explicitly.
dangerously_allow_non_loopback_proxy = false
mode = "full" # default when unset; hooks can still clamp specific HTTPS hosts
# When true, HTTPS CONNECT can be terminated so limited-mode policy and host-specific MITM hooks
# can inspect inner requests.
mitm = false
# CA cert/key are managed internally under $CODEX_HOME/proxy/ (ca.pem + ca.key).

# If false, local/private networking is rejected. Explicit allowlisting of local IP literals
# (or `localhost`) is required to permit them.
# Hostnames that resolve to local/private IPs are still blocked even if allowlisted.
allow_local_binding = false

# DANGEROUS (macOS-only): bypasses unix socket allowlisting and permits any
# absolute socket path from `x-unix-socket`.
dangerously_allow_all_unix_sockets = false

# Hosts must match the allowlist (unless denied).
# Use exact hosts or scoped wildcards like `*.openai.com` or `**.openai.com`.
# The global `*` wildcard is rejected.
# If no domain entries are marked `allow`, the proxy blocks requests until an allowlist is configured.
[permissions.workspace.network.domains]
"*.openai.com" = "allow"
"localhost" = "allow"
"127.0.0.1" = "allow"
"::1" = "allow"
"evil.example" = "deny"

# macOS-only: allows proxying to a unix socket when request includes `x-unix-socket: /path`.
[permissions.workspace.network.unix_sockets]
"/tmp/example.sock" = "allow"

[[permissions.workspace.network.mitm_hooks]]
host = "api.github.com"

[permissions.workspace.network.mitm_hooks.match]
methods = ["POST", "PUT"]
path_prefixes = ["/repos/openai/"]

[permissions.workspace.network.mitm_hooks.match.headers]
"x-github-api-version" = ["2022-11-28"]

[permissions.workspace.network.mitm_hooks.actions]
strip_request_headers = ["authorization"]

[[permissions.workspace.network.mitm_hooks.actions.inject_request_headers]]
name = "authorization"
secret_env_var = "CODEX_GITHUB_TOKEN"
prefix = "Bearer "

# `match.body` is reserved for a future release. Current hooks match only on
# method/path/query/headers and can mutate outbound request headers.
# `match.path_prefixes` accepts literal prefixes by default.
# Prefix with `glob:` to opt into wildcard matching, for example
# `glob:/repos/*/codex/issues*` or `glob:*preview*`.
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
export WS_PROXY="http://127.0.0.1:3128"
export WSS_PROXY="http://127.0.0.1:3128"
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
  - `blocked-by-mitm-hook`
  - `blocked-by-method-policy`
  - `blocked-by-mitm-required`
  - `blocked-by-policy`

In "limited" mode, only `GET`, `HEAD`, and `OPTIONS` are allowed. HTTPS `CONNECT` requests require
MITM to enforce limited-mode method policy; otherwise they are blocked. Separately, hosts covered
by `mitm_hooks` are authoritative even when `mode = "full"`: hooks are evaluated in order, the
first match wins, and if no hook matches the inner HTTPS request is denied with
`blocked-by-mitm-hook`. SOCKS5 remains blocked in limited mode.

Websocket clients typically tunnel `wss://` through HTTPS `CONNECT`; those CONNECT targets still go
through the same host allowlist/denylist checks.

## Library API

`codex-network-proxy` can be embedded as a library with a thin API:

```rust
use codex_network_proxy::{NetworkProxy, NetworkDecision, NetworkPolicyRequest};

let proxy = NetworkProxy::builder()
    .http_addr("127.0.0.1:8080".parse()?)
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

When unix socket proxying is enabled (`unix_sockets` or
`dangerously_allow_all_unix_sockets`), proxy bind overrides are still clamped to loopback to
avoid turning the proxy into a remote bridge to local daemons.

### Policy hook (exec-policy mapping)

The proxy exposes a policy hook (`NetworkPolicyDecider`) that can override allowlist-only blocks.
It receives `command` and `exec_policy_hint` fields when supplied by the embedding app. This lets
core map exec approvals to network access, e.g. if a user already approved `curl *` for a session,
the decider can auto-allow network requests originating from that command.

**Important:** Explicit deny rules still win. The decider only gets a chance to override
`not_allowed` (allowlist misses), not `denied` or `not_allowed_local`.

## OTEL Audit Events (embedded/managed)

When `codex-network-proxy` is embedded in managed Codex runtime, policy decisions emit structured
OTEL-compatible events with `target=codex_otel.network_proxy`.

Event name:

- `codex.network_proxy.policy_decision`
  - emitted for each policy decision (`domain` and `non_domain`).
  - `network.policy.scope = "domain"` for host-policy evaluations (`evaluate_host_policy`).
  - `network.policy.scope = "non_domain"` for mode-guard/proxy-state checks (including unix-socket guard paths and unix-socket allow decisions).

Common fields:

- `event.name`
- `event.timestamp` (RFC3339 UTC, millisecond precision)
- optional metadata:
  - `conversation.id`
  - `app.version`
  - `user.account_id`
- policy/network:
  - `network.policy.scope` (`domain` or `non_domain`)
  - `network.policy.decision` (`allow`, `deny`, or `ask`)
  - `network.policy.source` (`baseline_policy`, `mode_guard`, `proxy_state`, `decider`)
  - `network.policy.reason`
  - `network.transport.protocol`
  - `server.address`
  - `server.port`
  - `http.request.method` (defaults to `"none"` when absent)
  - `client.address` (defaults to `"unknown"` when absent)
  - `network.policy.override` (`true` only when decider-allow overrides baseline `not_allowed`)

Unix-socket block-path audits use sentinel endpoint values:

- `server.address = "unix-socket"`
- `server.port = 0`

Audit events intentionally avoid logging full URL/path/query data.

## Platform notes

- Unix socket proxying via the `x-unix-socket` header is **macOS-only**; other platforms will
  reject unix socket requests.
- HTTPS tunneling uses rustls via Rama's `rama-tls-rustls`; this avoids BoringSSL/OpenSSL symbol
  collisions in mixed TLS dependency graphs.

## Security notes (important)

This section documents the protections implemented by `codex-network-proxy`, and the boundaries of
what it can reasonably guarantee.

- Allowlist-first policy: if `domains` has no `allow` entries, requests are blocked until an allowlist is configured.
- Domain patterns: exact hosts are supported, `*.example.com` matches subdomains only, and `**.example.com` matches the apex plus subdomains; the global `*` wildcard is only accepted when explicitly enabled for allowlist compilation and is otherwise rejected.
- Deny wins: `domains` entries marked `deny` always override the allowlist.
- Local/private network protection: when `allow_local_binding = false`, the proxy blocks loopback
  and common private/link-local ranges. Explicit allowlisting of local IP literals (or `localhost`)
  is required to permit them; hostnames that resolve to local/private IPs are still blocked even if
  allowlisted (best-effort DNS lookup).
- Limited mode enforcement:
  - only `GET`, `HEAD`, and `OPTIONS` are allowed
  - HTTPS `CONNECT` remains a tunnel; limited-mode method enforcement does not apply to HTTPS
- Listener safety defaults:
  - the HTTP proxy listener clamps non-loopback binds unless explicitly enabled via
    `dangerously_allow_non_loopback_proxy`
- when unix socket proxying is enabled, all proxy listeners are forced to loopback to avoid turning the
    proxy into a remote bridge into local daemons.
- `dangerously_allow_all_unix_sockets = true` bypasses the unix socket allowlist entirely (still
  macOS-only and absolute-path-only). Use only in tightly controlled environments.
- `enabled` is enforced at runtime; when false the proxy no-ops and does not bind listeners.
Limitations:

- DNS rebinding is hard to fully prevent without pinning the resolved IP(s) all the way down to the
  transport layer. If your threat model includes hostile DNS, enforce network egress at a lower
  layer too (e.g., firewall / VPC / corporate proxy policies).
