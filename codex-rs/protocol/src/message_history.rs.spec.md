## Overview
`protocol::message_history` defines `HistoryEntry`, the unit recorded in Codex’s persistent conversation history. It captures enough metadata to retrieve and replay prior interactions.

## Detailed Behavior
- `HistoryEntry` stores the conversation ID as a string, the timestamp (`ts`) as a Unix epoch in seconds, and the logged text. The struct derives serialization and schema traits for interop with storage layers and clients.
- The module intentionally omits behavior; higher-level crates handle recording, filtering, and privacy controls.

## Broader Context
- Used when responding to `GetHistoryEntryRequest` events and when syncing conversation logs to external storage. Keeping the type minimal allows services with different persistence backends to adapt it easily.
- Timestamps are raw integers; consuming specs should clarify whether they represent seconds or milliseconds when displaying to users.
- Context can't yet be determined for storing rich metadata (e.g., role, attachments). Extensions can add optional fields when requirements emerge.

## Technical Debt
- None observed; the struct is a straightforward data carrier.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./protocol.rs.spec.md
