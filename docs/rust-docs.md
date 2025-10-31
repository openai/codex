# Rust Documentation Guidelines (Codex)

> Codex code should be self-explanatory to future maintainers without hunting Slack threads or PR comments.  
> Document intent, invariants, and system behavior — not line-by-line code.  
> This is a ratcheting standard: improve docs where you touch code.

## Confidentiality & Privacy

- Never include personally identifiable information (PII) in docs or examples.
- Do not disclose OpenAI confidential details (internal systems, keys, roadmap).
- Use scrubbed or fictional data when demonstrating behavior.

## Core Principles

- Prefer **intent + invariants** over restating implementation.
- Write docs that help a contributor **orient quickly**.
- Capture **non-obvious behavior** (I/O, concurrency, retries, lifetime/safety constraints).
- Prefer short, factual docs over verbose or marketing language.
- When in doubt: _what would future you want written here?_
- Improve docs **incrementally**, file-by-file.

**Docs should:**

- Explain what the item does and when to use it.
- Note invariants, assumptions, side effects.
- Link to related components using backticked intra-links, e.g., [`SchedulerClient`].
- Provide minimal but meaningful examples.

**Docs should NOT:**

- Translate code to English.
- Duplicate type signatures.
- Include speculative ideas or stale TODOs — delete instead.
- Add noise or break flow when reading the code.

## When to Update Docs

Update/add docs when code changes involving:

- New public types or functions
- Module purpose shifts
- Error model changes (retry, backoff, timeouts, error variants)
- New configuration or env vars
- Cross-service boundaries or RPC changes
- Unsafe code
- Performance-sensitive, async, or concurrent logic

If a Slack thread or PR explains the reasoning, **summarize the key logic in code**.

## Module-Level Docs (`//!`)

Use modules as **orientation maps**.

Include:

- High-level purpose in the system
- Responsibilities and flows
- Key types with links: [`AppConfig`], [`ArtifactStore`], [`SchedulerClient`]
- Assumptions & invariants
- Behavior around I/O, async, retries
- Any platform or environment differences

When helpful, add **module maps**:

```rust
//! Scheduler coordination layer.
//!
//! Responsibilities:
//! - Submit jobs to system
//! - Track execution state
//! - Stream scheduler events
//!
//! Key types:
//! - [`SchedulerClient`]
//! - [`JobId`]
//! - [`JobSpec`]
//!
//! Flows:
//! - submit → queue → event stream → completion
//! - watch events → reconcile local state
//!
//! Invariants:
//! - Jobs have stable IDs
//! - Event stream is monotonic where possible
```

## Struct-Level Docs

For complex structs, treat docs as a **map to the API surface**.

Include:

- What the type represents
- High-level behavior it provides
- Groups of related methods for orientation (not exhaustive listing)

Example:

```rust
/// Client for interacting with the distributed artifact store.
///
/// Major operations:
/// - Upload blobs (`store_*` fns)
/// - Download & stream content (`fetch_*`)
/// - Verify checksums
///
/// Related APIs:
/// - [`ArtifactId`]
/// - [`UploadConfig`]
///
/// Invariants:
/// - Uploads are atomic when possible
/// - IDs validated before use
pub struct ArtifactStore { /* ... */ }
```

## Function & Method Docs (`///`)

Template structure:

1. One-sentence summary
2. Subtle behaviors / invariants
3. Sections only if needed:

- `# Errors`
- `# Panics`
- `# Safety`
- `# Invariants`
- `# Examples`
- `# Notes`
- `# Performance`

Example:

````rust
/// Resolve configuration sources and produce a validated [`AppConfig`].
///
/// Searches CLI args, env, and default paths, applying merge rules and
/// canonicalizing paths.
///
/// # Errors
/// Returns `ConfigError` if required fields are missing or config is malformed.
///
/// # Examples
/// ```rust
/// # use codex::config::load_config;
/// let cfg = load_config(None)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
````

---

## `unsafe` Code Docs

Include both **doc comments** and `// SAFETY:` inline comments.

```rust
/// Write bytes into a raw buffer without bounds checks.
///
/// # Safety
/// Caller must guarantee:
/// - `ptr` is valid and uniquely owned
/// - `len <= capacity`
/// - Memory remains valid for the duration
pub unsafe fn write_unchecked(ptr: *mut u8, len: usize) { /* ... */ }
```

Inline:

```rust
// SAFETY: caller guarantees buffer validity and non-aliasing.
```

## Examples & Doctests

- Keep examples **minimal and compiling**
- `no_run` for network/FS
- Use `compile_fail` for negative tests demonstrating invariants

```rust,compile_fail
missing_required_field();
```

## Intra-Doc Link Conventions

Always include backticks inside intra-links:

Correct:

```
[`AppConfig`]
[`crate::scheduler::SchedulerClient`]
```

Incorrect:

```
[AppConfig]
```

Use short cross-links when unambiguous.

## Style Rules

- First line: **one sentence, present tense**
- Brevity over completeness
- Bullet lists for behavior > text walls
- ≤ ~100 character line width preferred
- If a doc becomes stale: **update or delete**

Rule of thumb:

> Docs should reduce cognitive load more than they increase it.

## Platform, Performance & Error Notes

Call out when code behavior depends on:

- OS/filesystem differences
- Retry/backoff/network semantics
- Async vs blocking behavior
- Resource constraints (memory, threads, streaming)

## Ratcheting Documentation Enforcement

We improve docs **incrementally**:

- When editing a file, raise its doc standard
- Add `#![warn(missing_docs)]` only after docs are acceptable for that file
- Gradually expand to entire crate

Eventually enable crate-level policies:

```rust
#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
```

But do it **module-by-module**, not all at once.

## Codex Agent Guidance

When Codex modifies Rust:

- Add or update docs for changed public APIs
- Update module/struct “maps” when responsibilities change
- Add/repair backticked intra-doc links
- Summarize complex reasoning in **~3 bullet points**
- Add/refresh minimal doctests
- Skip trivial boilerplate docs

> Codex should leave the codebase more explainable than it found it.

## What Not to Do

- Don’t narrate code
- Don’t restate type signatures
- Don’t leave stale docs
- Don’t add noise or philosophical commentary
- Don’t document trivial functions (unless there are caveats)

Docs should always **help new contributors build correct mental models quickly**.
