# Codex cloud client

Reusable Rust client for native ThreadService gRPC over HTTP/2, with no TUI,
conventional app-server or private monorepo dependency. This slice supports
`Resume` and live `Attach`. It does not use the HTTP `/grpc` JSON gateway.

The caller supplies a trusted native gRPC origin (scheme/host/port, no path),
ChatGPT bearer token, selected account ID and configured `HttpClientFactory`.
HTTPS is required except for direct loopback HTTP with an unrestricted, unmanaged
factory. This exception bypasses proxies to keep plaintext credentials local.
For HTTPS, the shared HTTP client supplies proxy,
custom CA and network policy handling; redirects are rejected. Tonic supplies
gRPC framing, protobuf encoding, message bounds (64 MiB) and status/trailer handling.
Bind factory policy before loading credentials; rebuild on login/account changes.

`ResumeRequest.wait = false` acknowledges admission, not readiness. Requests are
never retried by this crate: a timeout or disconnect can leave work admitted.
`Attach` has no history snapshot or replay guarantee. Dropping it detaches without
stopping the thread or answering approvals. Streams end after their first error.
Resume and Attach setup have a 150-second local deadline; live streams have no idle
deadline. Setup does not send a gRPC deadline that would expire a healthy stream.

Event variants distinguish notifications and server requests; payloads remain the
complete nested protobuf message bytes, preserving unknown fields. These are not
ProtoJSON or app-server JSON-RPC. Native status code, message and binary details
are available explicitly; credentials, event and error diagnostics are redacted.
Full payload typing, history, approval replies and reconnect are subsequent slices.
The minimal envelopes in `wire.rs` and `ResumeRequest` mirror ThreadService field
numbers; no monorepo checkout or protobuf compiler is required to build this crate.

Provide `CODEX_CLOUD_ACCESS_TOKEN` and `CODEX_CLOUD_ACCOUNT_ID` through your environment, then run
this standalone consumer from `codex-rs`:

```sh
cargo run -p codex-cloud-client --example attach -- "$CODEX_CLOUD_GRPC_ENDPOINT" "$CODEX_CLOUD_THREAD_ID"
```

Use an existing authorized thread and native gRPC origin. No production URL is
assumed. Public native ingress is a rollout assumption, not proved by fixture
tests. This change does not publish a package to a registry.
