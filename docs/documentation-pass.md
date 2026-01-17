# Documentation Pass Spec

This doc is the persistent, canonical spec for the project-wide documentation pass. Re-read before
each new pass.

Notes on doc placement

- (Moved below) See “Rustdoc style rules (strict)” for the general, idiomatic Rustdoc conventions to
  follow.

Process (imperative)

- Pick exactly one next module to work on (single file at a time); do not move on until it is fully
  documented.
- Work the file from top to bottom; do not skip ahead while leaving earlier items partially
  documented.
- After any context compaction or reset, re-read `docs/documentation-pass.md` and
  `docs/documentation-status.md` before continuing work.
- Parallel sessions: before starting a module, claim it in `docs/documentation-status.md` with your
  session number and date, then keep that claim updated until the module (and any tui/tui2 twin) is
  complete.
- Start with the module doc (`//!`) and draft the best available top-level narrative:
  responsibilities, non-responsibilities, state ownership, invariants, and how it fits the
  surrounding modules.
- Then document every type, field, function, method, and non-obvious block in that module (public
  and private) with idiomatic Rustdoc.
- As you document lower-level items, promote any cross-cutting constraints or invariants up to the
  module doc (gravity well): update the module doc again after item-level docs to capture newly
  discovered constraints.
- When a struct or enum has multiple fields or helper methods, refresh its doc comment after field-
  and method-level docs to surface any newly discovered invariants or usage patterns.
- A module is “complete” only when: module docs are current, all items are documented, all field
  docs exist where meaningful, and comment-spacing rules are satisfied.
- If the module has a twin in `tui2`, complete the matching file in `tui2` in the same pass before
  moving on.
- After each significant update, run `cargo doc` and capture any rustdoc warnings/lints; add new
  lint patterns to this document and to `docs/documentation-status.md` so the pass is
  self-improving.

Single-module imperative prompt (follow in order)

- Open the file and read it top-to-bottom to understand the flow.
- Draft or improve the module doc (`//!`) with responsibilities, non-responsibilities, ownership,
  and invariants.
- Walk item-by-item in source order; add `///` docs for every type, field, function, method, and
  non-obvious block.
- For long functions or structs, expand docs beyond a single sentence with a short mechanism
  overview and lifecycle/ownership notes.
- Fix comment spacing (blank line before comment blocks, none after; no blank line after item
  headers).
- Refresh the module doc again to pull up any new constraints discovered while documenting items.
- Run `cargo doc` and record any new warning patterns so future passes preempt them.

Restarting the pass

- Always restart from `codex-rs/tui` first. For this reset, focus only on the TUI crate.
- Prefer deep-first: fully document one module (module doc + key items + field invariants + inline
  comments) before moving to the next module. Avoid broad header sweeps.
- After each module, check the current diff for removed docs in that module and restore them if they
  were inadvertently dropped (e.g., due to placement errors).
- Parallel sessions: do not switch modules without updating `docs/documentation-status.md` so other
  sessions can pick the next file without overlap.

---

You are performing a **PROJECT-WIDE DOCUMENTATION PASS** for a multi-crate Rust TUI codebase.

The goal is to produce a local version of this project where a developer can jump to _any_ crate,
module, type, function, or non-obvious block and immediately understand:

- what it does,
- why it exists,
- what invariants and contracts it relies on,
- who owns what state,
- and how it fits into the overall system.

This documentation pass should resemble the quality and depth of the Rust standard library and
well-documented Rust crates such as tokio, axum, and ratatui: narrative first, precise contracts,
idiomatic Rustdoc, minimal but high-signal inline comments.

---

## **Non-negotiable safety rules**

You MUST:

- Not change runtime behavior.
- Not refactor logic.
- Not rename items or move code.
- Not change visibility (pub/private).
- Not alter async behavior, threading, or scheduling.
- Not change formatting except where needed to cleanly add documentation or comments.

You MAY:

- Add or edit Rustdoc comments (`//!` and `///`).
- Add or edit inline comments **only** where they clarify non-obvious logic, invariants, ordering
  constraints, or edge cases.
- Add Rustdoc intra-links and limited external links.
- Add examples **only** when they add real explanatory value.

If code changes are required to make the design clearer, list them as **follow-ups** but DO NOT
implement them.

---

## **Rustdoc style rules (strict)**

Write idiomatic Rust documentation:

- Every documented item starts with a **single-sentence summary**.
- Summary sentences should be grammatical and end with a period.
- Follow with normal narrative paragraphs — no invented section headers like “Overview”,
  “Postconditions”, or “Misuse”.
- Use Rustdoc headings ONLY when conventional and necessary:
  - `# Errors` when error meaning is non-trivial
  - `# Panics` when panics are part of the contract
  - `# Safety` for unsafe code or invariants
  - `# Examples` only for important public APIs

- Do NOT restate signatures, field names, or obvious control flow in prose.
- For long or multi-step functions, extend the summary with a brief mechanism overview (the 2–4 key
  steps or phases) so readers without full context can follow the flow without reading the body.
- When a long function performs multiple distinct responsibilities, prefer a short overview in the
  doc comment and add sparse inline comments to mark phase boundaries or non-obvious transitions.
- If a one-line summary would be too terse for a reader without module context, add 1–3 more
  sentences that explain the core mechanism and what the function returns or mutates.
- Apply the same “extra context” rule to long or high-impact structs, enums, and field groups:
  expand their doc comments with 2–4 sentences that explain lifecycle, ownership, and why the fields
  exist together.
- For fields that are part of a larger state machine or lifecycle, add a short rationale line that
  ties them to the owning component’s responsibilities (who sets them, when they change, what they
  gate).
- If a field represents a multi-step or multi-owner flow, add more than one sentence so a reader can
  understand where it is set, consumed, and how it affects behavior.
- Prefer explaining **mechanisms and invariants**, not symptoms or hypothetical bugs.
  - Write “Correctness relies on…” rather than “If this looks wrong…”.

### General idiomatic Rustdoc conventions

- Module docs (`//!`) belong at the top of the module file as the first non-empty line.
- Item docs (`///`) must appear immediately before the item (before any `#[...]` attributes).
- When items have attributes (for example `#[inline]`, `#[cfg(...)]`), place the doc comment above
  the attributes, not between the attribute and the item.
- Prefer Rust line doc comments (`//!`/`///`) over block doc comments (`/** ... */`).
- Keep doc comments adjacent to the item they document; avoid trailing or floating docs.
- For field and item documentation, prefer `///` unless the comment is a local, non-contract
  explanation scoped to a specific control-flow branch.
- Use `//` only for local, non-contract clarifications inside function bodies; if it describes
  ownership, invariants, or lifecycle, it belongs in `///` or `//!`.
- Avoid bare `//` section headers; prefer doc comments or remove if they add no information.
- Leave a blank line before each comment block (doc comments and `//` blocks), except immediately
  after an item header (e.g., `struct`, `enum`, `impl`) where no extra blank line should be
  inserted.
- Do not leave a blank line after a comment block; the comment should be immediately followed by the
  item or code it documents.
- Keep doc comments in active voice, avoid restating signatures, and prefer short paragraphs with
  explicit invariants and ownership notes.
- Headings and fenced code blocks must follow Markdown conventions with a blank line before and
  after the heading/code block so rustdoc renders them predictably.
- Use fenced code blocks for examples; omit the language tag for Rust, and add a tag only for plain
  text or other languages.
- Examples should compile; when they cannot, mark them with `no_run` or `ignore` and briefly explain
  why in the surrounding prose.
- If an intra-doc link is ambiguous, add a reference link at the end of the doc block:
  - Example:
    - [`Text`] describes ...
    - [`Text`]: foo::bar::Text
- Prefer intra-doc links to types and modules over prose re-explanations; use `crate::`, `super::`,
  and `Self::` where appropriate.
- Use backticks for code identifiers, flags, and literals in prose.
- Use plural/singular consistently and avoid synonyms for the same concept within a module.
- Prefer Markdown over HTML in doc comments; use HTML only when Markdown cannot express the layout.
- If you add reference links, put them at the end of the doc block and separate them from the prose
  with a blank line.
- Avoid raw URLs in prose; prefer named links or reference links when possible.
- Avoid bare URLs: use angle-bracket auto links (`<https://...>`) or reference links so rustdoc does
  not emit `bare_urls` warnings.
- Avoid literal angle-bracket placeholders like `<key>` or `<label>` in prose; rustdoc treats them
  as HTML tags. Use backticks (`` `key` ``) or escape as `&lt;key&gt;`.
- Avoid bracketed status tags like `[UNSTABLE]` or `[x]` in plain text; rustdoc interprets them as
  links. Use backticks or escape `[`/`]` as `\\[`/`\\]`.
- Ensure intra-doc links resolve: use fully-qualified paths when the type is not in scope, and
  prefer backticks if the target is private or intentionally unlinked.

### Intra-doc links

- Prefer Rustdoc links like [`Type`], [`module::item`], [`Self::method`].
- Use external links only for genuinely load-bearing specs (terminal behavior, protocols,
  standards).

---

## **Comment placement rules (gravity well rule)**

- Anything about **architecture, layering, ownership, lifecycle, or invariants** belongs in:
  - crate-level docs (`//!` in lib.rs / main.rs),
  - module-level docs (`//!`),
  - or item docs (`///`).

- Inline comments inside functions are ONLY for:
  - tricky local logic,
  - ordering constraints,
  - platform quirks,
  - edge cases,
  - or intentionally surprising behavior.

Litmus test:

- If a comment would still be true if the function body were rewritten, it must be a doc comment,
  not an inline comment.

---

## **Coverage target**

You are documenting the _entire project_, across all crates.

### 1) **Workspace / crate-graph documentation (important)**

At the workspace root or top-level crate docs, document:

- Each crate’s responsibility and non-responsibility
- How crates relate architecturally (layering, allowed dependency direction)
- Which crates define core concepts vs UI vs protocol vs integration
- Any invariants that span multiple crates

Treat crates as architectural layers, not just Cargo packages.

---

### 2) **Crate-level documentation**

For each crate:

- Purpose and scope
- Key concepts and terminology
- Main flows (happy path)
- Major non-goals
- How it interacts with other crates
- Where correctness invariants are enforced
- Where debugging typically starts (only stable, meaningful entry points)

---

### 3) **Module-level documentation**

For every non-trivial module, add or refine a `//!` header explaining:

- What problem the module solves
- Key concepts and terms
- Responsibilities vs non-responsibilities
- How it fits into surrounding modules
- Any state machine or lifecycle it manages
- Which components are responsible for enforcing invariants

Avoid API listings; write narrative explanations.

---

### 4) **Public API documentation**

For every public struct, enum, trait, function, and method:

- One-sentence summary
- Conceptual role in the system
- Invariants and contracts
- Ownership, mutability, and threading assumptions
- For `Result`: what errors mean and what callers should do
- Examples only when genuinely useful

---

### 5) **Important internal items**

For private but central components (state machines, caches, event routers, schedulers):

- Document like public APIs, but lighter
- Focus on ownership, invariants, and coupling points
- Explain why the abstraction exists

---

### 6) **Field-level documentation (TUI-specific emphasis)**

Document struct fields when:

- The meaning is not obvious from name/type
- The field participates in an invariant
- The field is a cache key, revision counter, tick, generation ID, or sentinel

For such fields, explain:

- What change it represents
- Who is responsible for updating it
- What breaks if it is not updated

These fields are part of the correctness contract even in private structs.

---

### 7) **Event flow & state ownership (critical for TUIs)**

For modules handling:

- input events,
- draw events,
- async streams,
- protocol events,

Document explicitly:

- Who owns the state
- Who may mutate it
- When mutation is allowed
- How synchronization with rendering is achieved

Avoid implicit ownership.

---

### 8) **Terminal and platform assumptions**

When behavior depends on:

- terminal capabilities,
- platform quirks,
- input modifiers,
- cursor or rendering behavior,

Document:

- The assumption being made
- Where it is handled or abstracted
- Whether it is a hard requirement or best-effort behavior

Do not describe symptoms; describe mechanisms and constraints.

---

### 9) **Async rationale**

For async code paths:

- Explain why async is required here
- Whether async is part of the public contract or an implementation detail
- How async tasks interact with UI or shared state

Prevent accidental blocking or cross-thread mutation assumptions.

---

### 10) **Modes, overlays, and lifecycle**

When code implements modes, overlays, or lifecycle transitions:

- Define what a “mode” means in this system
- Describe transition points
- State what data is preserved vs reset
- Document invariants that must hold across transitions

---

### 11) **Inline comments**

Add inline comments only for:

- non-obvious edge cases,
- ordering dependencies,
- intentional performance tradeoffs,
- platform-specific behavior,
- surprising but correct logic.

Avoid “what the code does” comments.

---

## **Depth calibration**

Document to the level where a new contributor can:

- find the right abstraction quickly,
- understand the system’s shape,
- and reason about correctness without reverse-engineering call graphs.

Avoid:

- documenting trivial helpers,
- repeating code in prose,
- tutorial-style exposition in internal modules.

Bias toward documenting invariants, ownership, and lifecycle.

---

## **Work plan**

1. Workspace / crate-graph docs
2. Crate-level docs
3. Module-level docs
4. Public API docs
5. Key internal components
6. Field-level invariant docs
7. Selective inline comments
8. Terminology consistency pass (prefer links over re-explanation)

---

## **Final output**

Produce a patch that:

- adds crate, module, item, and field documentation,
- adds only high-signal inline comments,
- uses idiomatic Rustdoc and intra-links,
- avoids non-idiomatic labeled sections.

At the end, provide a short **Follow-ups** list:

- assumptions you had to guess,
- places where naming or structure blocks good docs,
- suggested future refactors (DO NOT implement).

You are not here to be polite — you are here to make the system legible.
