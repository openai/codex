## Overview
`lib.rs` bridges standard input/output with a Unix Domain Socket. It connects to the socket, copies data from the socket to stdout on a background thread, streams stdin to the socket, and shuts down gracefully once either side closes.

## Detailed Behavior
- Uses `UnixStream` (platform-specific implementations) to connect to the provided path.
- Clones the stream for simultaneous read/write; spawns a thread to copy socket output to stdout while flushing at the end.
- Streams stdin into the socket, then shuts down the write half to signal EOF.
- Waits for the reader thread to finish, propagating IO errors.
- Returns `anyhow::Result<()>` with contextual error messages.

## Broader Context
- Invoked by other Codex tooling when a simple stdio↔UDS relay is needed, e.g., to connect sandboxed helpers or language servers.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
