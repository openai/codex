# Codex Infty: Ultra‑Long Task Orchestration

Design a clean, extensible way to run arbitrarily long tasks (hours–days) with bounded model context, autonomous continuation, and robust correctness review. Works for code and non‑code.

Status: Proposed • Scope: New crates using `codex-core` • Compatibility: Non‑breaking

---

## 1) Motivation
- Context windows are limited → we must compact and retrieve.
- Models pause/ask for permission → we must self‑direct.
- No systematic review → we must verify before returning.

## 2) Approach (High‑Level)
Run three coordinated roles as independent `codex-core` sessions. Reuse existing tools (shell, apply_patch, read_file, list_dir, grep_files) for persistence and retrieval. Add one clean, first-class cross-session facility in core for direction/verification — orchestrator-driven, no model-visible tool. The CLI currently spawns a solver, a director, and three verifiers (`verifier-alpha`, `verifier-beta`, `verifier-gamma`) by default.

- Solver (Model A): executes plan; writes all results to memory/artifacts; never asks humans to continue.
- Director (Model B): answers Solver’s direction questions and re‑plans when needed.
- Verifier (Model C…Cₙ): evaluates completion claims; returns pass/fail with structured feedback.

Inter‑role coordination uses a built‑in CrossSessionHub in core. The orchestrator watches assistant messages and bridges them as user turns to the peer role.

## 3) Architecture
```
┌────────────────────────────┐
│        codex-infty         │
│  Orchestrator + CLI/Lib    │
│  - spawns 3 codex sessions │
│  - supervises long runs    │
│  - configures Run/Role     │
└────────────┬───────────────┘
             │
   ┌─────────▼─────────┐    ┌─────────▼─────────┐
   │   Solver (A)      │    │  Director (B)     │
   │ codex-core session│    │ codex-core session│
   └─────────┬─────────┘    └─────────┬─────────┘
             │                         │
             └──────────┬──────────────┘
                        │
                ┌───────▼────────┐
                │ Verifier(s) (C)│
                │ codex-core sess │
                └───────┬─────────┘
                        │
                CrossSessionHub (core, orchestrator‑driven)
                JSONL rollouts, auto‑compaction
```

### Components
- codex-infty (new crate)
  - Spawns/owns three `codex-core` sessions (A/B/C) with role‑specific base instructions.
  - Supervises progress over very long runs.
  - Defines a simple on‑disk Run Store that the models write to using existing tools.
  - Configures sessions with Run/Role metadata (for cross‑session routing).
- codex-core (existing, with one addition)
  - Reuse streaming, tool routing, JSONL rollouts with resume, auto‑compaction, and existing tools:
    - `apply_patch`, `shell`/`exec_command`/`write_stdin`
    - `grep_files`, `read_file`, `list_dir` (enable via model family/experimental tools)
  - New: built‑in `CrossSessionHub` for intra‑process routing (§5). No new model tool is exposed.

## 4) Data Model (Durable) and Filesystem Layout
Persist everything in a Run Store directory; models read/write using existing tools.

- Run Store layout (example under `~/.codex/infty/<run-id>/`):
  - `artifacts/` – blobs and text outputs (models can create via `apply_patch` for text; `shell` for binary moves/copies).
  - `memory/` – JSON/Markdown notes: facts, hypotheses, plans, decisions, claims, evidence, evaluations.
  - `index/` – optional search/index artifacts (built out‑of‑band by orchestrator jobs; models can still use `grep_files`).

Data is append‑only by convention; items link to each other via ids/paths stored in JSON.

## 5) New Core API: CrossSessionHub (no model tool)
Add a core facility that lets the orchestrator bridge assistant messages between sessions by posting user turns.

### 5.1 Hub API
- Registry that maps `{ run_id, role } -> session handle` and `{ session_id } -> session handle`.
- Sessions register on spawn with `run_id` and `role`; unregister on drop.
- Expose async methods for the orchestrator:
  - `post_user_turn(to: RoleOrId, text: String) -> TurnHandle` – inject a `UserTurn` as if typed by a user.
  - `await_first_assistant(turn: &TurnHandle, timeout: Duration) -> AssistantMessage` – wait until the first assistant message for that turn.
  - `stream_events(session_id) -> impl Stream<Item = Event>` – optional subscription for higher‑level orchestration.

### 5.2 Orchestrator Bridge Logic
- Direction: when the Solver emits an assistant message asking for permission/direction, the orchestrator forwards that assistant text verbatim as a user turn to the Director and waits for the Director’s first assistant reply; it then posts that reply as a user turn to Solver.
- Verification: when Solver requests verification, orchestrator forwards request to Verifier(s); structured verdicts (pass/fail/reasons/suggestions) flow back.
- Persistence: Each session persists its own events to rollout; the orchestrator just routes.

## 6) Run Store Facilities
- Memory notes follow JSON schemas per role (plans, claims, evidence).
- Artifacts include code patches, logs, compiled binaries, docs. Use naming convention `<timestamp>-<summary>.<ext>`.
- Orchestrator can create `index/` entries (e.g., embeddings) offline; models still access via standard tools.

## 7) Orchestrator Flow
1. Initialize Run Store + metadata (objective, roles, options).
2. Spawn Solver, Director, Verifier sessions via `CrossSessionHub`.
3. Seed objective as Solver user turn; monitor outputs.
4. Relay direction/verification messages automatically between roles.
5. Trigger periodic checkpoints (copy artifacts/memory to dated snapshots).
6. On completion, ensure Verifier returns pass, then emit final deliverable path.
7. Support resume: reload Run Store, respawn sessions with `InitialHistory::Resumed`.

## 8) Context Management
- Conversational context: rely on `codex-core` auto‑compaction.
- Long‑term memory: persist facts/results as files; retrieve with `grep_files`/`read_file`/`list_dir`.
- Run Store snapshots allow cold resume even after orchestrator restart.

## 9) Verification Strategies
- Code: tests, linters, type checks via `shell` under sandbox.
- Text: grader rubrics, citation/contradiction checks.
- Math/research: multi‑verifier consensus, self‑consistency, proof‑sketch validation.

## 10) Security & Policy
- All execution stays under `codex-core` sandbox/approval.
- Memory/Artifact tools are pure data I/O (no code execution).
- Inter‑role calls run in isolated sessions.

## 11) MVP (Phased)
1. codex-core
   - Add `CrossSessionHub` with registration and post/await APIs.
   - Add `run_id` and `role` registration on session spawn (optional fields).
   - Tests: two sessions in a run; orchestrator posts user text to Director and bridges reply to Solver.
2. codex-infty
   - Orchestrator lib + CLI: create Run Store directories, spawn A/B/C sessions with `run_id`/`role`, run loop; ship role prompts. Enable `grep_files`/`read_file`/`list_dir`.
3. Verification
   - Use `shell` to run checks/tests when applicable; use Verifier sessions for rubric‑based judgments.

## 12) Finalization & Extensibility
- Finalization workflow (after `verdict == pass`): the orchestrator issues a final `UserTurn` to the Solver instructing:
  - Create a clean `deliverable/` folder under the Run Store.
  - Copy/transform only the necessary end results; remove scratch artifacts.
  - Write a `deliverable/README.md` including: overview, contents manifest with paths and sizes, verification steps (how to run tests), and any limitations.
  - Summarize the work in the final assistant message and return the path to `deliverable/`.

- Extensibility:
  - Pluggable `IndexStrategy` (keyword/embeddings/hybrid) built by the orchestrator (models still query via `grep_files`).
  - Multiple Verifiers with majority/weighted consensus.
  - Future: broadcast/multicast cross‑session calls (e.g., ask three verifiers and aggregate).

## 13) Why This Solves The Three Problems
- Context: conversational compaction + durable memory with retrieval.
- Pauses: assistant questions are bridged to a Director; the orchestrator backstops.
- Review: Solver’s verification request is bridged to Verifier(s) with structured verdicts and remediation.

This keeps `codex-core` focused and leverages its strengths (streaming, tools, compaction, rollouts) while adding a small, clean cross‑session primitive to enable arbitrarily long, autonomous runs across domains.

---

## 14) End‑to‑End Example (Minimal)

Assume a run folder at `~/.codex/infty/run_123/`.

1) User objective → Solver (UserTurn)
- User: "Write a tiny CLI that prints Fibonacci numbers and provide usage docs."

2) Solver starts
- Tool: `update_plan` → steps: parse request; scaffold CLI; implement logic; write docs; verify; finalize deliverable.
- Tool: `grep_files` → searches `artifacts/` and repo for prior art.

3) Solver seeks direction
- Solver’s assistant message: “Confirm plan: binary in ./fib, args: N, output first N Fibonacci numbers; docs in memory/docs.md?”
- Orchestrator posts a UserTurn to Director with that question and sets `final_output_json_schema` to the Director schema (strict).
- Director’s first assistant message:
```json
{ "directive": "Proceed. Add tests under memory/tests.md; prefer iterative impl; expose --limit flag.", "rationale": "Keeps stack small; eases verification." }
```
- Orchestrator posts that reply as a UserTurn to Solver; Solver continues.

4) Solver implements
- Tool: `apply_patch` → creates `artifacts/fib.rs` and a small Cargo bin, or shell scaffolding.
- Tool: `shell` → `cargo run -- 10` to sanity check (under sandbox).
- Writes `memory/docs.md` and `memory/tests.md`.

5) Solver claims completion
- Writes `memory/claims/cli.json` (per template) referencing artifacts and docs.
- Solver’s assistant message: “Please verify claim in memory/claims/cli.json with artifacts/fib.rs; run cargo test if present.”
- Orchestrator posts a UserTurn to Verifier with `final_output_json_schema` set to the Verifier schema (strict). Verifier runs checks (via `shell`), returns:
```json
{ "verdict": "fail", "reasons": ["No tests"], "suggestions": ["Add tests covering N=1,2,10"] }
```
- Orchestrator posts that reply as a UserTurn to Solver; Solver iterates (adds tests, fixes issues).

6) Pass and finalize
- Verifier returns `{ "verdict": "pass", … }`.
- Orchestrator issues finalization UserTurn to Solver:
  - "Create deliverable/: include compiled bin or script, usage docs, and tests; write deliverable/README.md with run instructions; remove scratch files."
- Solver:
  - Tool: `shell`/`apply_patch` → builds `deliverable/` with README and artifacts.
  - Assistant message: "Deliverable ready at ~/.codex/infty/run_123/deliverable/."

7) Orchestrator returns the final path to the user.
