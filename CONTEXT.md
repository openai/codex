# Codex Worktrees

This context describes how Codex names and reasons about Git-backed session destinations. The product exposes the familiar Git term while distinguishing those destinations from the broader notion of a session workspace.

## Language

**Workspace**:
The filesystem location where a Codex session runs.

**Managed worktree**:
A Codex-owned workspace backed by a Git worktree.
_Avoid_: workspace when the Git-backed ownership matters

**External worktree**:
A Git worktree visible to Codex but not owned or mutated by Codex.

**Worktree origin**:
The current creator lineage of a managed worktree, such as CLI or App, retained for compatibility while clients converge on one app-server-backed implementation.

**Worktree**:
The user-facing term for a managed worktree, matching the Codex App and developers' existing Git vocabulary.
_Avoid_: workspace when referring specifically to the product feature

**Worktree management**:
Standalone creation, inspection, and removal of managed worktrees outside a running Codex session.

**Worktree launch**:
Starting a Codex session in a named managed worktree, creating it when needed.

**Worktree switching**:
Moving an active Codex session into a managed worktree from inside the TUI.

## Relationships

- A **managed worktree** is one kind of **workspace**
- An **external worktree** is a visible **workspace** that Codex does not own
- A **worktree** is the user-facing name for a **managed worktree**
- A **worktree origin** describes current provenance, not a permanent product subtype
- A **workspace** may exist without being a **managed worktree**
- **Worktree management** operates on **managed worktrees** whether or not a session is currently running in them
- **Worktree launch** may create or reuse a **managed worktree** before the session begins
- **Worktree switching** may create or reuse a **managed worktree** after the session has begun
- **Worktree management**, **worktree launch**, and **worktree switching** share the app-server-backed managed-worktree model

## Example dialogue

> **Dev:** "When a user chooses a **worktree**, are they choosing any **workspace**?"
> **Domain expert:** "No — they are choosing a Codex-managed Git-backed **workspace**."
>
> **Dev:** "Does a **worktree** only exist once a session starts there?"
> **Domain expert:** "No — **worktree management** lets users create and inspect it before a session enters it."
>
> **Dev:** "Why can `--worktree` create one instead of only selecting an existing one?"
> **Domain expert:** "Because **worktree launch** should be a one-step path into the named worktree."
>
> **Dev:** "Can `--force` remove an **external worktree**?"
> **Domain expert:** "No — force bypasses cleanliness checks, not ownership."
>
> **Dev:** "Are CLI and App worktrees different products?"
> **Domain expert:** "No — their **worktree origins** differ today while the App is still migrating toward the shared app-server implementation."
>
> **Dev:** "Can remote CLI commands manage worktrees outside a session?"
> **Domain expert:** "Yes — remote and local management both go through app-server."

## Flagged ambiguities

- "workspace" and "worktree" were being used interchangeably — resolved: **workspace** is broader; **worktree** names the Git-backed product feature.
- "force" could imply bypassing ownership — resolved: it only bypasses dirty-state protection for **managed worktrees**.
- "CLI worktree" and "App worktree" could sound like permanent subtypes — resolved: they are current **worktree origins**, not the long-term domain model.
