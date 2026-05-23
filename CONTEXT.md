(eval):5: parse error near `end'
# Codex Review Experience

Language for features that help people understand, navigate, and evaluate code changes in Codex.

## Language

**Review Story**:
A structured explanation of a change that organizes the diff into a small number of ordered, cohesive steps for reviewer understanding. A **Review Story** is a navigation and explanation artifact, not a finding engine or correctness verdict.
_Avoid_: Review module, PR story, change story

**Story Source**:
The change set that a **Review Story** explains. A **Story Source** may be a branch comparison, uncommitted changes, or a single commit; it is not limited to a hosted pull request.
_Avoid_: Pull request, GitHub PR

**Concrete Story Source**:
A **Story Source** that resolves to a deterministic diff and **Source Fingerprint**, such as a branch comparison, uncommitted changes, or a single commit. V1 **Review Stories** require a **Concrete Story Source**.
_Avoid_: Custom instructions

**Source Fingerprint**:
A deterministic identity for a **Story Source** used to decide whether a **Story Snapshot** still matches the underlying change. The fingerprint is derived from resolved refs, SHAs, and diff content rather than model output.
_Avoid_: Cache key, model context hash

**Stale Story**:
A **Story Snapshot** whose saved **Source Fingerprint** no longer matches the current **Story Source**. A **Stale Story** remains readable, but reviewers should refresh it before relying on its anchors or ordering.
_Avoid_: Invalid story, expired cache

**Story Step**:
One ordered unit inside a **Review Story** that explains a cohesive part of the change. A **Story Step** has a goal, a summary, and references to the specific changed files or ranges that support that explanation.
_Avoid_: Cohort, layer, phase

**Step Goal**:
The reason a **Story Step** exists in the **Review Story**. A **Step Goal** explains the role of the step in the change, while the step summary explains what changed.
_Avoid_: Step title, implementation detail

**Review Focus**:
Non-verdict guidance on what a reviewer should pay attention to while reading a **Story Step**. **Review Focus** can highlight assumptions, dependencies, or areas worth checking, but it is not a finding.
_Avoid_: Finding, issue, warning

**Change Anchor**:
An exact reference from a **Story Step** to evidence in the **Story Source**, such as a changed file, hunk, or line range. A **Change Anchor** must point to an actual changed region, so the reviewer can verify the story against the diff.
_Avoid_: File mention, related file

**Anchor Id**:
A stable identifier assigned by Codex to a **Change Anchor** before model story generation begins. Models choose from **Anchor Ids** instead of inventing file paths or line ranges.
_Avoid_: Model anchor, generated location

**Evidence Graph**:
The system-derived structure of the **Story Source**, including changed files, hunks, commits, renames, and cheap dependency signals. The **Evidence Graph** constrains the model-authored **Review Story** but is not itself the story shown to reviewers.
_Avoid_: Dependency map

**Evidence Signal**:
A cheap, local fact included in the **Evidence Graph**, such as commit order, changed file status, hunk ranges, rename metadata, path relationships, or obvious test/source pairing. **Evidence Signals** are intentionally lighter than full semantic indexing.
_Avoid_: Semantic index, full dependency graph

**Story Review**:
A review-adjacent workflow that generates a **Review Story** for understanding and navigation before findings are produced. A **Story Review** is distinct from the findings-oriented review workflow.
_Avoid_: Review mode, code review

**Story Snapshot**:
A persisted version of a **Review Story** tied to the identity of its **Story Source**. A **Story Snapshot** gives reviewers stable step numbers and lets Codex detect when the story is stale because the underlying change has moved.
_Avoid_: Cached story, generated output

**Snapshot Lineage**:
The relationship between a refreshed **Story Snapshot** and the older snapshot it replaces for the same **Story Source**. **Snapshot Lineage** preserves what reviewers saw before refresh while letting surfaces default to the newest story.
_Avoid_: In-place refresh

**Snapshot Status**:
The lifecycle state of a **Story Snapshot**, such as outline generation, outline ready, enrichment, completion, failure, or staleness. **Snapshot Status** lets **Story Surfaces** show progress without guessing from individual fields.
_Avoid_: Generation state

**Snapshot Update**:
A full replacement notification for the latest version of a **Story Snapshot**. V1 **Story Surfaces** apply **Snapshot Updates** by replacing their local snapshot instead of merging granular patches.
_Avoid_: Step patch, delta event

**Story Store**:
The structured persistence location for **Story Snapshots**, keyed by thread and **Story Source**. The **Story Store** holds the evolving snapshot data while thread history records lifecycle events that point to snapshot ids.
_Avoid_: Thread history, transcript

**Story Database**:
The SQLite database managed by the shared state layer that backs the **Story Store**. The **Story Database** is separate from thread metadata storage so story-specific migrations, cleanup, and larger structured payloads remain isolated.
_Avoid_: Thread database, metadata database

**Story Record**:
The persisted database representation of a **Story Snapshot**. A **Story Record** stores the canonical snapshot as structured JSON plus indexed fields for lookup, status, timestamps, source identity, and step readiness.
_Avoid_: Normalized story schema, raw blob

**Progressive Story Snapshot**:
A **Story Snapshot** that becomes useful before every **Story Step** is fully enriched. A **Progressive Story Snapshot** first exposes a validated outline, then updates individual steps as their descriptions and rationale become ready.
_Avoid_: Streaming story, partial JSON

**Step Enrichment**:
The background work that fills in detailed descriptions, rationale, and summaries for **Story Steps** after the validated outline exists. **Step Enrichment** may run in small batches so reviewers can begin navigating before every step is complete.
_Avoid_: Step generation, eager loading

**Story Schema**:
The strict structured output contract used by model calls that create a **Review Story**. A **Story Schema** lets Codex validate step order, **Anchor Ids**, and enriched fields before updating a **Story Snapshot**.
_Avoid_: Markdown format, prose contract

**Enrichment Context**:
The bounded source context given to **Step Enrichment**, including the selected **Change Anchors**, nearby file context, and cheap dependency neighbors from the **Evidence Graph**. **Enrichment Context** supports explanation but does not replace the anchored evidence.
_Avoid_: Full repo context, diff-only context

**Step Readiness**:
The per-step state that tells a **Story Surface** whether a **Story Step** has only outline data or also has completed **Step Enrichment**. **Step Readiness** lets surfaces render useful pending states without assuming steps finish in story order.
_Avoid_: Loading flag

**Partial Story**:
A **Story Snapshot** whose outline is usable even though one or more **Story Steps** have not completed **Step Enrichment**. A **Partial Story** may include failed steps, but it still preserves navigation through validated anchors.
_Avoid_: Failed story

**Story Surface**:
A product surface that presents a **Story Snapshot** to a reviewer. The TUI and App UI are separate **Story Surfaces** that should read the same underlying **Review Story** data.
_Avoid_: UI implementation, frontend

**Story Overlay**:
The TUI **Story Surface** for navigating a **Story Snapshot**. The **Story Overlay** presents ordered steps, anchored diffs, and step explanation outside the normal transcript flow.
_Avoid_: Markdown story, transcript summary

**Read-Only Story Overlay**:
The v1 **Story Overlay** mode that lets reviewers navigate, inspect, copy, and refresh **Story Snapshots** without leaving comments or submitting hosted reviews.
_Avoid_: Review submission, comment mode

**/story**:
The TUI slash command that starts or opens a **Story Review**. The existing review picker should also expose **Review Story** creation for discoverability.
_Avoid_: /review-story

**Story API**:
The app-server v2 contract used by **Story Surfaces** to create, read, and refresh **Story Snapshots**. The **Story API** is the shared product boundary for the TUI and App UI.
_Avoid_: TUI API, local story service

**reviewStory**:
The app-server v2 API namespace for **Story API** methods. **reviewStory** is separate from the findings-oriented `review` namespace.
_Avoid_: story, review

**Story Turn**:
A thread-scoped model run that generates or refreshes a **Story Snapshot** from a **Story Source**. A **Story Turn** uses Codex's normal execution lifecycle while producing a persisted story artifact.
_Avoid_: Background job, standalone task

## Example Dialogue

Dev: "Can the reviewer find bugs from the Review Story?"

Domain expert: "Findings may later attach to Review Story steps, but the Review Story itself exists to explain the change in a useful review order."

Dev: "Is every Review Story about a GitHub pull request?"

Domain expert: "No. A hosted pull request is one possible Story Source, but local branch comparisons and commits should use the same language."

Dev: "Can arbitrary custom review instructions create a Review Story in v1?"

Domain expert: "No. V1 requires a Concrete Story Source so every Story Step can be anchored to a deterministic diff."

Dev: "How does Codex know a Story Snapshot is stale?"

Domain expert: "Codex compares the current Story Source to the Source Fingerprint saved with the Story Snapshot."

Dev: "Should Codex automatically rewrite a stale story?"

Domain expert: "No. V1 should mark a Stale Story visibly and let the reviewer choose when to refresh it."

Dev: "Should the story have cohorts and layers?"

Domain expert: "Not initially. A Review Story is a single ordered list of Story Steps unless we later introduce a separate concept for independent sub-stories."

Dev: "What is the difference between a Story Step goal and summary?"

Domain expert: "The Step Goal explains why this step belongs in the story; the summary explains what changed in the anchored code."

Dev: "Can a Story Step tell the reviewer what to inspect?"

Domain expert: "Yes, as Review Focus. It should guide attention without claiming the code is wrong."

Dev: "Can a Story Step just mention the files it talks about?"

Domain expert: "No. Each Story Step should carry Change Anchors to the changed ranges that support the explanation."

Dev: "Can the model write paths and line numbers for step anchors?"

Domain expert: "No. Codex assigns Anchor Ids first, and the model may only reference those ids."

Dev: "Does the model decide what changed?"

Domain expert: "The model explains and orders the change, but the Evidence Graph defines the changed evidence it is allowed to reference."

Dev: "Does v1 need a full semantic dependency graph?"

Domain expert: "No. V1 should use cheap local Evidence Signals and leave deeper indexing for later."

Dev: "Should the Review Story be part of the bug-finding review?"

Domain expert: "No. A Story Review helps the reviewer understand the change; a code review looks for issues."

Dev: "Can the story be regenerated whenever the reviewer opens it?"

Domain expert: "No. A Review Story should be saved as a Story Snapshot so reviewers can rely on stable steps and know when the source changed."

Dev: "Does refreshing a stale story overwrite the old snapshot?"

Domain expert: "No. Refresh creates a new Story Snapshot linked to the previous one through Snapshot Lineage."

Dev: "How does a Story Surface know whether the story is ready?"

Domain expert: "It reads the Snapshot Status and Step Readiness rather than inferring readiness from missing prose."

Dev: "Should progressive story updates be sent as small patches?"

Domain expert: "Not in v1. Story Surfaces should receive Snapshot Updates and replace their local snapshot with the newest version."

Dev: "Should the whole Story Snapshot live inside chat history?"

Domain expert: "No. Thread history should record story lifecycle events, while the Story Store holds structured snapshot data."

Dev: "Should Story Snapshots be stored in the thread metadata database?"

Domain expert: "No. Story Snapshots belong in a separate Story Database managed by the state layer."

Dev: "Should the Story Database fully normalize every step and anchor?"

Domain expert: "Not in v1. A Story Record should keep a canonical snapshot JSON while indexing the fields needed for lookup and progress."

Dev: "Does the reviewer have to wait until every step is fully written?"

Domain expert: "No. A Progressive Story Snapshot can expose a validated outline first, then fill in step details while the reviewer navigates."

Dev: "Should Codex enrich every Story Step in a separate model call?"

Domain expert: "Not by default. Step Enrichment should use small batches with bounded parallelism so navigation feels fast without creating excessive model work."

Dev: "Can story generation return Markdown?"

Domain expert: "No. Story generation should return data that matches the Story Schema so Codex can validate and render it across surfaces."

Dev: "Can Step Enrichment inspect context outside the changed ranges?"

Domain expert: "Yes, but only as Enrichment Context. The Story Step still has to explain the anchored changed evidence."

Dev: "Should navigating to a step reprioritize enrichment in v1?"

Domain expert: "Not initially. V1 should enrich the first step, nearby steps, and then the remaining steps in order, while Step Readiness keeps the UI ready for out-of-order completion later."

Dev: "If one Step Enrichment batch fails, is the whole story failed?"

Domain expert: "No. Once the outline exists, Codex should keep a Partial Story usable and mark only the affected steps as failed."

Dev: "Should Review Stories include diagrams in v1?"

Domain expert: "No. V1 should focus on trustworthy step ordering, anchors, progressive readiness, and diff navigation."

Dev: "Can a Review Story include code-review findings?"

Domain expert: "No. Findings belong to the findings-oriented review workflow, though a future surface may attach findings to Story Steps."

Dev: "Will the TUI story and App UI story be different concepts?"

Domain expert: "No. They are different Story Surfaces over the same Review Story data."

Dev: "Should the TUI print the Review Story as a normal assistant message?"

Domain expert: "No. The TUI should use a Story Overlay so reviewers can move between steps and anchored diffs."

Dev: "Can reviewers submit comments from the Story Overlay in v1?"

Domain expert: "No. V1 is a Read-Only Story Overlay; comments can attach to Change Anchors in a later version."

Dev: "How does a TUI user start a Story Review?"

Domain expert: "Use /story directly, or choose the Review Story option from the review picker."

Dev: "Can the TUI define its own story shape first?"

Domain expert: "No. The Story API defines the shared shape, even if the TUI is the first Story Surface to render it."

Dev: "Should story APIs live under review/start?"

Domain expert: "No. Story APIs use the reviewStory namespace because they support review understanding, not findings review."

Dev: "Is story generation a separate background job?"

Domain expert: "No. Story generation should be a Story Turn so progress, cancellation, and replay follow the rest of Codex."
