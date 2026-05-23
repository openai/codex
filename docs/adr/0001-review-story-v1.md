(eval):5: parse error near `end'
# Review Story v1

Status: accepted

Codex will introduce a Review Story workflow that explains a concrete code change as an ordered, anchored story for reviewers. The story is a navigation and understanding artifact, distinct from findings-oriented code review, and v1 will expose it through app-server v2 plus a TUI surface.

## Context

Large PRs are hard to review when the reviewer only sees an alphabetical file list or a flat diff. Recent review tools, especially CodeRabbit Change Stack and PR Walkthroughs, show that reviewers benefit from a logical reading order, grouped explanations, and range-specific summaries. Codex already has `/review`, but that flow is optimized for finding issues and producing verdicts, not for helping a human understand the shape of a change step by step.

## Decision

Review Story v1 will use a shared app-server v2 API namespace, `reviewStory/*`, backed by a reusable Rust implementation and a separate SQLite story database managed by the state layer. The TUI will be the first Story Surface and will provide a read-only Story Overlay; the App UI can later consume the same API and snapshot model.

A Review Story is generated from a Concrete Story Source: branch comparison, uncommitted changes, or a single commit. Custom review instructions are out of scope for v1 unless they resolve to a concrete source. Each Story Source receives a deterministic Source Fingerprint so saved Story Snapshots can be marked stale instead of silently rewritten.

Story generation is progressive. Codex first builds a deterministic Evidence Graph with stable Anchor Ids for files and hunks. A model then creates a strict JSON outline that orders Story Steps using only those Anchor Ids. After the outline is validated, Codex exposes a Progressive Story Snapshot and enriches steps in small batches with bounded parallelism. Story Surfaces receive full Snapshot Updates and replace their local snapshot with the newest version.

Story Snapshots are persisted in a Story Store, not embedded wholesale in thread history. Thread history records lifecycle events that point to snapshot ids. Refreshing a stale story creates a new snapshot linked to the previous one, preserving Snapshot Lineage.

## Considered Options

- Reuse `/review`: rejected because findings and correctness verdicts should remain separate from explanatory navigation.
- Store snapshots only in thread history: rejected because snapshots are structured, mutable during generation, and shared by multiple surfaces.
- Generate the whole story in one model call: rejected because users should be able to navigate once a validated outline exists.
- Let the model invent file paths and line ranges: rejected because Change Anchors must be system-validated.
- Fully normalize the story database schema in v1: rejected because the API usually reads full snapshots, while indexed lookup/status fields are enough initially.

## V1 Shape

The initial `StorySnapshot` should expose snapshot identity, thread id, source, source fingerprint, status, timestamps, optional previous snapshot id, title, overview, steps, anchors, and staleness.

Each `StoryStep` should expose a stable id, display index, title, goal, summary, dependency rationale, anchor ids, review focus, readiness, and optional error. Step readiness can be `outline`, `enriching`, `ready`, or `failed`.

The Story Overlay should show an ordered step list, a diff view filtered to the selected step's Change Anchors, and the selected step's goal, summary, dependency rationale, and Review Focus. V1 is read-only: no GitHub comments or review submission from the overlay.

## Consequences

This design gives reviewers fast, trustworthy navigation without turning Review Story into another finding engine. It also creates durable product boundaries: app-server v2 owns the contract, the Story Database owns structured persistence, and Story Surfaces render snapshots without re-deriving the diff story themselves.

The trade-off is more infrastructure in v1: a story database, source fingerprints, snapshot lifecycle notifications, and structured model schemas are required before the feature feels complete. The payoff is that TUI and App UI can share one artifact, and future features such as comments, diagrams, snapshot comparison, or findings attached to Story Steps can be added without replacing the core model.
