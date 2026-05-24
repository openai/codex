# Review Story API Foundation

Status: accepted

## Context

Large code changes are difficult to review as a flat file list. Codex already has `/review`, but that workflow is findings-oriented; a review story is a separate, read-only artifact intended to explain a change in a useful reading order.

The first reusable layer must support both the future TUI surface and an App UI surface without coupling story generation to either client.

## Decision

Introduce an app-server v2 API namespace, `reviewStory/*`, backed by shared Rust generation logic and a separate SQLite story database managed by the state layer.

`reviewStory/start` accepts a concrete source: branch comparison, uncommitted changes, or one commit. The server collects the source diff, creates deterministic file-level anchors and a source fingerprint, invokes one constrained structured model task to group anchors into ordered steps, validates anchor coverage, and persists the completed snapshot. If generation is unavailable or invalid, it persists a usable file-level fallback snapshot instead.

The API also provides `reviewStory/read` and `reviewStory/list` for stored snapshots and emits `reviewStory/snapshot/updated` when a snapshot is written. Snapshot and step status fields include values that allow a future progressive generator, but this foundation produces completed `ready` or `partial` snapshots with `ready` steps.

Story snapshots live in the story store rather than in thread transcript history. The API is the product boundary: a stacked TUI feature and a later App UI can present the same stored story artifact.

The stacked TUI layer provides `/story` and a Review picker action. It renders completed snapshots in a read-only cockpit with ordered steps, selected-step rationale, associated file patches, overview/help/contents subviews, and a compact terminal layout. It does not perform progressive enrichment or update snapshots in place.

## Deliberately Deferred

- App UI presentation.
- Progressive outline-first generation and asynchronous step enrichment.
- Hunk-level anchors, dependency graph signals, or semantic indexing.
- Staleness detection, refresh, and snapshot lineage behavior.
- Review comments, findings, or hosted review submission from a story surface.

## Consequences

This layer provides a stable, validated, reusable story artifact without requiring a UI or an asynchronous job lifecycle in the same change. Clients receive a complete snapshot, and failures degrade to full file-level coverage rather than hiding changed evidence.

Progressive loading remains compatible with the API shape but will require a separate implementation: early `building` snapshots, background enrichment, repeated snapshot writes, and client consumption of replacement notifications.
