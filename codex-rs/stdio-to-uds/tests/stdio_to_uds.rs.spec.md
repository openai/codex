## Overview
`stdio_to_uds` integration test verifies that the `codex-stdio-to-uds` binary correctly proxies stdin/stdout over a Unix domain socket.

## Detailed Behavior
- Creates a temp directory and binds a `UnixListener` (platform-specific: `std::os::unix::net::UnixListener` on Unix, `uds_windows` on Windows).
- Spawns a server thread that accepts a single connection, captures all bytes sent by the client, responds with `response`, and forwards the captured bytes to the main thread over an `mpsc` channel.
- Runs `codex-stdio-to-uds` via `assert_cmd`, sending `request` on stdin and asserting that stdout yields `response`.
- Confirms the server received the raw request bytes, and propagates any server-side errors back into the test.
- Skips gracefully if Unix socket binding fails with `PermissionDenied` (e.g., sandbox restrictions).

## Broader Context
- Ensures the IPC bridge binary exercised in Phase 3D behaves correctly on supported platforms, providing regression coverage for the CLI integration.

## Technical Debt
- Single test covers the happy path; future regression coverage (timeouts, non-existent sockets) could be added if bugs arise.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../src/lib.rs.spec.md
