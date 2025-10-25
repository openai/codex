## Overview
`lib.rs` maintains a bounded in-memory log buffer, exposes a `MakeWriter` adapter for tracing, and provides snapshot utilities for saving or uploading logs to Sentry.

## Detailed Behavior
- `CodexFeedback` manages an `Arc<FeedbackInner>` wrapping a mutex-protected `RingBuffer`. Constructors allow default or custom capacity (`DEFAULT_MAX_BYTES` defaults to 2 MiB).
- `make_writer` returns a `FeedbackMakeWriter` implementing `tracing_subscriber::fmt::writer::MakeWriter`, enabling seamless integration with the tracing infrastructure.
- `RingBuffer` operations:
  - `push_bytes` appends bytes, truncating from the front when capacity is exceeded, with special handling when the incoming chunk is larger than the buffer.
  - `snapshot_bytes` clones the current contents.
- `CodexFeedback::snapshot` locks the buffer and wraps the bytes plus a thread id (derived from the conversation id or a new UUID) in `CodexLogSnapshot`.
- `CodexLogSnapshot` helpers:
  - `save_to_temp_file` writes the snapshot to `tmp/codex-feedback-<thread>.log`.
  - `upload_to_sentry` builds a sentry envelope with the log attachment, sends it using the default transport, and flushes with a 10 s timeout.
- Writer adapters (`FeedbackMakeWriter`, `FeedbackWriter`) implement `Write` to record log output into the ring buffer; `flush` is a no-op.
- Tests verify the ring buffer retains the most recent bytes.

## Broader Context
- Invoked by top-level feedback/reporting workflows to collect recent logs when users request support or submit feedback from the CLI/TUI.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
