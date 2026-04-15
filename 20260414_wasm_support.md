# WASM Support In Codex

## Purpose

This document summarizes the design and implementation changes needed to support running the Codex agent harness in a `wasm32` browser environment.

The goals are:

1. Identify the points in the Codex harness that need to be refactored to support WASM so we can discuss the right upstream abstractions with Codex maintainers.
2. Record the major categories of changes required by the current prototype so that future rebases can distinguish essential seams from incidental implementation details.

This document is not intended to be a complete walkthrough of every file changed in the prototype branch. It is intended to capture the architectural pressure points.

## Non-Goal

This document does not argue that the browser should support the full native Codex feature set on day one. The prototype relies on degraded mode in several places. The main question is whether the Codex orchestration loop can run in the browser with a reduced environment surface. The prototype shows that it can.

## Key Result From The Prototype

The main Codex turn loop did not require a fundamental redesign to run in the browser.

The browser demo now runs a real turn through the Codex harness by creating a real thread and calling:

```rust
session.thread.submit(Op::UserTurn { ... }).await
```

The necessary refactoring was mostly at the environment boundary:

- networking and streaming
- async runtime semantics
- code execution runtime
- filesystem/config/state assumptions
- tool inventory assembly
- native-only utility crates

The orchestrator was largely reusable. The host-heavy edges were the real blockers.

## High-Level Framing

The useful architectural question is:

> What assumptions does the Codex harness make about the runtime environment, and which of those assumptions must become explicit abstractions or degraded-mode implementations for `wasm32`?

In practice, the current prototype required two kinds of changes:

1. Compile-time separation of native and wasm implementations.
2. Runtime degradation for features that are not required to complete a basic turn in the browser.

## Refactor Points Required For WASM

### 1. Responses API Transport And Streaming

This is one of the core seams.

The Codex harness assumes streamed model events. The browser prototype preserved that shape rather than introducing a separate unary or mock model path.

What needed refactoring:

- the HTTP transport needed a wasm-compatible implementation
- SSE parsing and stream handling needed to work in wasm
- native-only transport details needed to be compile-gated

Relevant files from the prototype:

- `codex-rs/codex-client/src/transport.rs`
- `codex-rs/codex-client/src/sse.rs`
- `codex-rs/codex-api/src/sse/responses.rs`
- `codex-rs/codex-client/src/custom_ca_wasm.rs`

Upstream discussion point:

- There should be a stable transport boundary for Responses HTTP plus SSE.
- Websocket support can remain native-only at first if it is cleanly compiled out on wasm.

### 2. Async Runtime Semantics

WASM support did not require removing async Rust. It did require making native assumptions about `Send` and task spawning explicit.

The key issue is that native Tokio code often assumes futures can safely satisfy `Send`, while browser-backed values may not.

What needed refactoring:

- wasm-specific async surfaces where `?Send` is required
- compatibility wrappers for spawn/timing behavior
- preserving native `Send` guarantees instead of weakening them globally

Relevant files:

- `codex-rs/core/src/async_runtime.rs`
- `codex-rs/async-utils/src/lib.rs`

Prototype lesson:

- This is a real source of regressions when doing a wasm split.
- The correct pattern is usually platform-specific impls or conditional traits, not silently weakening native guarantees.

Upstream discussion point:

- A small async compatibility layer is a better seam than scattering `cfg(target_arch = "wasm32")` checks throughout orchestration code.

### 3. Code Execution Runtime

This is the most important architectural seam for browser execution.

The browser does not need native shell execution to run a useful Codex turn. It does need a model-visible execution surface for code mode.

The prototype solved this by injecting a browser-specific code-mode runtime backed by an iframe sandbox.

What needed refactoring:

- thread/session startup needed a way to accept a non-native code-mode runtime
- code mode needed to remain enabled on wasm
- nested native tools needed to be absent or reduced in wasm mode

Relevant files:

- `codex-rs/wasm-harness/src/browser.rs`
- `codex-rs/core/src/thread_manager.rs`
- `codex-rs/core/src/codex.rs`
- `codex-rs/core/src/tools/code_mode/mod.rs`
- `codex-rs/code-mode/src/description.rs`

Prototype lesson:

- The orchestrator can stay the same if code execution is treated as an injected environment capability.

Upstream discussion point:

- The code-mode runtime boundary should be treated as a first-class extension point.
- Browser execution can then become one implementation, not a fork of the harness loop.

### 4. Filesystem, Config, Memory, And Session State

Codex currently assumes a host filesystem in many places:

- config loading
- project docs and instruction files
- memory artifacts
- rollout persistence
- message history
- parts of the state bridge

For the browser prototype, most of these did not need full browser implementations. They needed degraded-mode behavior so a basic turn could complete.

What needed refactoring:

- local file reads needed wasm-safe or stubbed alternatives
- config loading needed a wasm-aware path
- memory and history handling needed ephemeral or no-op behavior
- state persistence needed to tolerate the absence of the native DB/runtime

Relevant files:

- `codex-rs/core/src/async_fs.rs`
- `codex-rs/core/src/config_loader_wasm.rs`
- `codex-rs/core/src/memories_wasm.rs`
- `codex-rs/core/src/message_history_wasm.rs`
- `codex-rs/core/src/state_db_bridge.rs`
- `codex-rs/core/src/project_doc_wasm.rs`

Prototype lesson:

- For a browser v0, the important distinction is between in-turn required state and optional persisted state.
- Many of these systems can run in degraded mode initially.

Upstream discussion point:

- The harness should make instruction/config/state sources injectable where practical.
- Optional persistence should be cleanly separable from turn execution.

### 5. Tool Inventory Assembly

The browser does not need every built-in Codex tool to complete a basic turn.

The important requirement is that the harness can assemble a reduced, coherent tool surface from available capabilities.

What needed refactoring:

- built-in tool inventory logic had to tolerate absent native capabilities
- code-mode-only browser execution had to work with fewer nested tools
- native-only tools had to be omitted rather than assumed

Relevant files:

- `codex-rs/core/src/tools/spec.rs`
- `codex-rs/core/src/tools/handlers/mod.rs`
- `codex-rs/core/src/tools/code_mode/mod.rs`

Prototype lesson:

- Browser support does not require porting every native tool.
- It requires that tools be derived from capabilities rather than hard-wired native assumptions.

Upstream discussion point:

- Tool assembly should be capability-driven.
- That would align well with the broader orchestrator/environment split.

### 6. Native-Only Utility Crates

Several crates assumed native behavior directly at crate root, which prevented wasm compilation even when the browser did not need their full behavior.

The main fix was to split these crates into native/wasm implementations and re-export through a common surface.

Examples:

- `codex-rs/apply-patch/src/lib.rs`
- `codex-rs/apply-patch/src/native.rs`
- `codex-rs/apply-patch/src/wasm.rs`
- `codex-rs/secrets/src/lib.rs`
- `codex-rs/secrets/src/native.rs`
- `codex-rs/secrets/src/wasm.rs`
- `codex-rs/login/src/wasm.rs`
- `codex-rs/utils/pty/src/process_wasm.rs`

Prototype lesson:

- Some of these crates are not conceptually part of the wasm browser runtime.
- They still need compile-time separation so the orchestrator can build.

Upstream discussion point:

- Native/wasm crate structure should minimize leakage of host assumptions into shared orchestration code.

### 7. Native Execution, Sandboxing, And Process APIs

Native Codex includes shell execution, PTY handling, sandbox policies, JS REPL internals, and related runtime services that do not map directly to browser execution.

For the browser prototype, the correct move was not to emulate the full native stack. The correct move was to provide wasm-specific modules that let the core crate compile and let the browser run with a reduced execution surface.

Relevant files:

- `codex-rs/core/src/exec_wasm.rs`
- `codex-rs/core/src/exec_policy_wasm.rs`
- `codex-rs/core/src/landlock_wasm.rs`
- `codex-rs/core/src/tools/js_repl_wasm.rs`
- `codex-rs/core/src/mcp_connection_manager_wasm.rs`

Upstream discussion point:

- Executor and sandboxing concerns should remain environment-owned.
- The orchestrator should depend on explicit capability surfaces, not directly on native process behavior.

## What Did Not Need Fundamental Refactoring

The following was the most important finding from the prototype:

- the main turn loop did not need to be replaced
- the browser did not need a custom harness loop
- the browser did not need a fake event model

Once the host-heavy assumptions were refactored or degraded, the prototype could run a real turn through the existing Codex orchestration path.

This is important for upstream discussion because it suggests the right investment is not a second browser-specific harness. The right investment is making the existing harness more environment-aware.

## Rebase Guidance

Codex upstream is evolving quickly, including active work around orchestrator/environment boundaries. Future rebases are likely to encounter churn in exactly the areas touched by this prototype.

To make rebases tractable, it is useful to classify changes into two buckets.

### Essential Changes To Preserve

These reflect real architectural seams needed for wasm:

- Responses transport/SSE support
- async runtime compatibility
- injectable code execution runtime
- degraded-mode config/state/memory handling
- capability-driven tool assembly
- native/wasm crate separation for host-bound utilities

### Changes That May Need Rework During Rebase

These are more implementation-specific and may not survive upstream changes unchanged:

- exact wasm stub file layout
- exact browser sandbox message shape
- exact prompt tuning in code mode
- exact conditional compilation points
- exact degraded-mode behavior for optional systems

The important rule for rebasing is:

> preserve the seam, not necessarily the exact patch.

## Questions To Discuss With Upstream Maintainers

1. What is the intended stable boundary for environment-specific execution capabilities?
2. Should Responses networking be abstracted more explicitly, or is compile-gated transport injection sufficient?
3. What is the intended abstraction boundary for code mode runtime injection?
4. Which persistence systems are truly required for a turn, and which can be optional?
5. Should tool inventory assembly become explicitly capability-driven?
6. Which native-only crates should expose formal wasm stubs versus being moved behind higher-level interfaces?

## Practical Takeaway

The prototype suggests that supporting wasm is feasible without replacing the Codex agent harness.

The main work is to make runtime assumptions explicit:

- what needs network transport
- what needs async tasking semantics
- what needs code execution
- what needs filesystem or persistence
- what tools exist because of capabilities versus because of current native defaults

That is the set of seams worth discussing upstream.
