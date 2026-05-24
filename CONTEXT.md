# Codex Review Experience

Language for features that help people understand and navigate code changes in Codex.

## Implemented Language

**Review Story**:
A structured explanation of a change that organizes the diff into a small number of ordered, cohesive steps for reviewer understanding. A **Review Story** is a navigation and explanation artifact, not a finding engine or correctness verdict.
_Avoid_: Review module, PR story, change story

**Story Source**:
The change set that a **Review Story** explains. A **Story Source** may be a branch comparison, uncommitted changes, or a single commit; it is not limited to a hosted pull request.
_Avoid_: Pull request, GitHub PR

**Concrete Story Source**:
A **Story Source** that resolves to a deterministic diff and **Source Fingerprint**, such as a branch comparison, uncommitted changes, or a single commit. Review Story creation requires a **Concrete Story Source**.
_Avoid_: Custom instructions

**Source Fingerprint**:
A deterministic hash of the selected source and its collected diff. It is stored with the snapshot so future refresh or staleness behavior can compare source identity without depending on model output.
_Avoid_: Model context hash

**Story Step**:
One ordered unit inside a **Review Story**. A **Story Step** has a title, goal, summary, dependency rationale, review focus, and references to the changed evidence that supports it.
_Avoid_: Cohort, layer, phase

**Step Goal**:
The reason a **Story Step** belongs in the story. A goal explains the role of that step; its summary explains what changed.

**Review Focus**:
Non-verdict guidance on what a reviewer should pay attention to while reading a **Story Step**. It may highlight assumptions or dependencies, but it is not a finding.
_Avoid_: Finding, issue, warning

**Change Anchor**:
A system-created piece of changed evidence referenced by a **Story Step**. The current implementation creates one anchor for each file diff and stores its raw patch, path, and detected change kind.
_Avoid_: Model-invented location

**Anchor Id**:
A stable identifier assigned by Codex to a **Change Anchor** before model story generation begins. The model may group only provided anchor ids; unknown references are discarded and missing anchors are appended for complete coverage.

**Story Snapshot**:
A persisted version of a **Review Story** associated with one thread and source fingerprint. The current generator writes a completed snapshot after model grouping and validation or fallback construction.
_Avoid_: Transcript output

**Snapshot Status**:
The stored lifecycle result of a snapshot. Current creation produces `ready` for fully validated output or an empty diff, and `partial` when generation falls back or the model omits or invents anchors. `building` and `failed` remain protocol values reserved for future asynchronous generation.

**Step Readiness**:
The per-step readiness value included in the shared API shape. Current story creation emits `ready` steps; `outline`, `enriching`, and `failed` are reserved for future progressive work.

**Snapshot Update**:
A full snapshot notification emitted when the app-server writes a story snapshot. Current generation emits one update for the completed snapshot; a future progressive workflow may emit additional replacements.
_Avoid_: Delta event

**Story Store**:
The structured persistence location for **Story Snapshots**, keyed for lookup by thread and snapshot id and listable by thread. It stores canonical snapshot JSON plus indexed summary columns.

**Story Database**:
The SQLite database managed by the shared state layer that backs the **Story Store**. It is separate from thread metadata storage so story-specific persistence can evolve independently.

**Story Schema**:
The strict structured-output contract used by the model call that creates a **Review Story**. It ensures the server can validate step fields and anchor assignments before persisting a snapshot.

**Partial Story**:
A complete stored snapshot that remains usable but required fallback or coverage correction. In the current implementation, a **Partial Story** is not a streaming or unfinished state.

**Story Surface**:
A product surface that presents a **Story Snapshot** to a reviewer. The reusable app-server API exists so TUI and App UI surfaces can share the same artifact without generating their own story shape.

**Story API**:
The app-server v2 contract used to create and retrieve **Story Snapshots**. It exposes `reviewStory/start`, `reviewStory/read`, `reviewStory/list`, and `reviewStory/snapshot/updated`.
_Avoid_: TUI API, local story service

**reviewStory**:
The app-server v2 API namespace for **Story API** methods. It is separate from the findings-oriented `review` namespace because its artifact explains rather than judges a change.

**Story Overlay**:
The TUI presentation of a completed **Story Snapshot**. It provides an ordered steps rail, step details, changed evidence, and compact-layout contents view without mutating or regenerating the story.

**/story**:
The TUI command that starts story creation for a selected **Concrete Story Source** and opens its resulting **Story Overlay**. The review picker also exposes the same story action.

## Deferred Concepts

These terms describe intended follow-up work, not behavior delivered by the current API/backend implementation.

**Stale Story** and **Snapshot Lineage**:
Future refresh behavior may compare stored source fingerprints and link newly generated snapshots to earlier versions. Current snapshots set `stale` to false and do not link a previous snapshot.

**Evidence Graph**:
A possible future evidence model that adds hunk anchors, commit relationships, or dependency signals. Current evidence is the set of deterministic file-level anchors from a git diff.

**Progressive Story Snapshot** and **Step Enrichment**:
A future asynchronous generation workflow in which an ordered outline is usable before every step has complete explanatory prose. Current generation uses one blocking model grouping call and stores one completed snapshot.

## Example Dialogue

Dev: "Does Review Story find bugs?"

Domain expert: "No. It explains and orders changed evidence for reviewer understanding; findings remain part of the review workflow."

Dev: "Can a model invent file locations for a story step?"

Domain expert: "No. Codex creates file-diff anchors first, and the model may only group those anchor ids."

Dev: "Does story generation stream step updates today?"

Domain expert: "No. It persists a completed ready or partial snapshot. The notification and readiness fields leave room for future progressive generation."

Dev: "Can the TUI and App UI render different story models?"

Domain expert: "No. Both surfaces should consume the shared `reviewStory` snapshot contract."
