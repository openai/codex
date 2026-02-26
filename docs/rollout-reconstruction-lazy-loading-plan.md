# Rollout Reconstruction Lazy Reverse Loading Plan

## Summary

The current rollout reconstruction code has the right intent, but not the right long-term shape for lazy reverse loading.

Today, [`reconstruct_history_from_rollout`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L351-L389) still takes a fully materialized slice of rollout items, and the history path still assumes in-memory prefix rereads through [`HistoryCheckpoint`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L12-L17), [`scan_rollout_tail`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L249-L307), and [`reconstruct_history_from_tail_scan`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L309-L340).

The target design should instead:

- consume rollout items through a lazy reverse source that can load earlier items on demand
- compute [`previous_model`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L8-L9) and [`reference_context_item`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L8-L9) eagerly during initial reconstruction
- return a lazy history object that can later accept additional backtracking and resume loading older rollout items without restarting reconstruction from scratch
- keep the replay state alive in memory for the rest of the process

## Why The Current Shape Falls Short

The current refactor improved the direction of travel, but it still has two in-memory assumptions that work against the future design.

First, [`HistoryCheckpoint`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L12-L17) stores a slice index (`prefix_len`) rather than an opaque position in the rollout source. That is useful for the current implementation, but it ties compaction replay to already-loaded rollout data.

Second, the `replacement_history: None` path in [`reconstruct_history_from_tail_scan`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L321-L333) recursively slices the already-loaded rollout prefix and rebuilds from there. That is logically correct, but operationally it assumes that the full prefix is already resident in memory.

The current code is therefore better than a pure forward replay, but it is still a one-shot reconstruction function rather than a persistent lazy replay object.

## Key Design Principle

Almost all rollout replay logic should operate as a stateful lazy reverse consumer over rollout items.

That means the replay code should:

- read newest-to-oldest
- stop as soon as it has enough information for the current request
- retain enough state to later continue consuming older items when more history is needed

The only part that should remain eagerly resolved is resume metadata:

- [`previous_model`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L8-L9)
- [`reference_context_item`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L8-L9)

Those values should be fully computed during initial construction so that the lazy history object only needs to manage history materialization and future backtracking.

## Proposed API Shape

[`reconstruct_history_from_rollout`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L351-L389) should stop taking `&[RolloutItem]` and instead take a lazy reverse source.

A plain Rust `Iterator` is not the right abstraction here because we need:

- async loading
- reverse pagination
- stop and resume
- opaque cursors for loading older data later

A better shape is a custom async source trait:

```rust
#[async_trait]
trait ReverseRolloutSource {
    type Cursor: Clone + Send + Sync + 'static;

    async fn load_earlier(
        &mut self,
        before: Option<Self::Cursor>,
        limit: usize,
    ) -> CodexResult<ReverseRolloutChunk<Self::Cursor>>;
}

struct ReverseRolloutChunk<C> {
    items_newest_to_oldest: Vec<(C, RolloutItem)>,
    reached_start: bool,
}
```

The exact naming is flexible, but the important properties are:

- items are yielded newest-to-oldest
- the caller can ask for more older items later
- the source owns the pagination details

## Proposed Return Type Shape

[`RolloutReconstruction`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L5-L10) should keep eager metadata, but replace eager `Vec<ResponseItem>` history with a lazy history object.

A target shape would look like:

```rust
pub(super) struct RolloutReconstruction<S: ReverseRolloutSource> {
    pub(super) history: LazyReconstructedHistory<S>,
    pub(super) previous_model: Option<String>,
    pub(super) reference_context_item: Option<TurnContextItem>,
}
```

This keeps metadata simple while moving only history into the lazy segment.

## Lazy History State

The lazy history object should be persistent and own both:

- the reverse rollout source
- the in-memory replay state accumulated so far

A useful shape is:

```rust
struct LazyReconstructedHistory<S: ReverseRolloutSource> {
    source: S,
    earliest_loaded_cursor: Option<S::Cursor>,
    reached_start: bool,
    rollback_debt: usize,
    replay: HistoryReplayState<S>,
}
```

Where `rollback_debt` is the persisted version of the same idea currently used by [`ReverseHistoryCollector`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L19-L85) and [`ReverseMetadataState`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L97-L239): a count of newest user turns that still need to be skipped.

That is the right semantic model because it lines up with [`drop_last_n_user_turns`](../codex-rs/core/src/context_manager/history.rs#L201-L230): rollback is fundamentally “skip N newest user turns,” not “cut to a precomputed absolute boundary.”

## Replace `HistoryCheckpoint` With A Deferred Base

The current [`HistoryCheckpoint`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L12-L17) concept is useful, but it should evolve into a lazy base description rather than a slice-based checkpoint.

A target shape is:

```rust
enum HistoryBase<S: ReverseRolloutSource> {
    Unknown,
    StartOfFile,
    Replacement(Vec<ResponseItem>),
    Compacted {
        summary_text: String,
        before: Option<S::Cursor>,
    },
}

struct HistoryReplayState<S: ReverseRolloutSource> {
    base: HistoryBase<S>,
    suffix: ReverseHistoryCollector,
}
```

This is the key change.

Instead of immediately rebuilding history when we hit `Compacted { replacement_history: None }`, we should record:

- the compaction summary text
- where to resume reading earlier rollout data if we later need to materialize that prefix

That lets lazy history retain a deferred base instead of forcing immediate recursive prefix reconstruction.

## Reverse Consumption Rules

The existing reverse metadata consumer in [`ReverseMetadataState`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L97-L239) is broadly the right shape and should mostly survive.

The lazy reverse history consumer should follow these rules:

1. `ResponseItem`
   - feed into a reverse suffix collector, equivalent to the current [`ReverseHistoryCollector`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L19-L85)
   - apply rollback debt the same way as today

2. `ThreadRolledBack`
   - increment rollback debt
   - do not immediately rebuild history

3. `Compacted { replacement_history: Some(...) }`
   - set `base = Replacement(...)`
   - stop scanning once metadata is also resolved for the current operation

4. `Compacted { replacement_history: None }`
   - set `base = Compacted { summary_text, before }`
   - stop scanning once metadata is also resolved for the current operation

5. beginning of file with no compaction base found
   - set `base = StartOfFile`

That means the replay logic only reads as much rollout as the current operation requires.

## Materializing History

The lazy history object should expose a method that materializes a `Vec<ResponseItem>` only when needed.

A target interface could be:

```rust
impl<S: ReverseRolloutSource> LazyReconstructedHistory<S> {
    async fn materialize(
        &mut self,
        initial_context: &[ResponseItem],
        truncation_policy: TruncationPolicy,
    ) -> CodexResult<Vec<ResponseItem>>;
}
```

The materialization rules would be:

- `Replacement(history)` returns that history as the base
- `StartOfFile` uses an empty base
- `Compacted { summary_text, before }` recursively materializes the older prefix before `before`, collects user messages from that prefix, and then calls [`build_compacted_history`](../codex-rs/core/src/compact.rs#L379-L390)
- after the base is ready, append the surviving suffix collected by the reverse history collector

This keeps the current compaction semantics, but moves prefix loading behind a lazy cursor instead of an in-memory slice.

## Supporting Future Backtracking

The lazy history object should also expose an operation for additional rollback requests that go before the current loaded boundary.

A target interface could be:

```rust
impl<S: ReverseRolloutSource> LazyReconstructedHistory<S> {
    async fn apply_backtracking(&mut self, additional_user_turns: u32) -> CodexResult<()>;
}
```

This method should:

- add to the stored rollback debt
- keep reading earlier rollout items until either:
  - the debt is satisfiable from loaded data, or
  - the source reaches the beginning of the rollout
- update the existing replay state rather than reconstructing from scratch

That is the critical affordance missing from the current design.

## Recommended Refactor Sequence

A staged migration should keep the semantic rules stable while improving the structure.

1. Introduce a reverse rollout source abstraction.
   - keep the current in-memory slice source as the first implementation
   - make the replay logic consume that abstraction instead of `&[RolloutItem]`

2. Replace [`HistoryCheckpoint`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L12-L17) with a deferred base enum.
   - remove `prefix_len`
   - store an opaque “before this point” cursor instead

3. Split eager metadata from lazy history.
   - keep [`previous_model`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L8-L9) and [`reference_context_item`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L8-L9) eager
   - make `history` lazy and persistent

4. Replace [`scan_rollout_tail`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L249-L307) with a persistent reverse consumer.
   - consume chunks incrementally
   - stop when current caller has enough data
   - preserve replay state for later backfill

5. Add `apply_backtracking` to the lazy history object.
   - use rollback debt rather than forcing immediate full rematerialization

6. Only after that, add a real reverse file reader.
   - the in-memory behavior and API should already match the desired loading semantics

## What Can Stay From The Current PR

The following pieces are directionally correct and worth keeping:

- [`ReverseMetadataState`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L97-L239)
- [`ReverseHistoryCollector`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L19-L85)
- rollback as user-turn skip debt rather than an eager cut index

The following pieces are temporary and should be removed in the next structural pass:

- [`HistoryCheckpoint`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L12-L17) as currently defined
- [`TailScan`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L241-L247)
- [`scan_rollout_tail`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L249-L307) as a one-shot helper over `&[RolloutItem]`
- recursive prefix slicing in [`reconstruct_history_from_tail_scan`](../codex-rs/core/src/codex/rollout_reconstruction.rs#L321-L333)

## Bottom Line

The main architectural change is this:

- stop treating rollout reconstruction as a one-shot function over a fully loaded slice
- start treating it as a persistent reverse replay state machine with eager metadata and lazy history

That gives us:

- fast initial resume/fork hydration
- a clear stop condition for reverse loading
- a straightforward path to real lazy file reads
- support for future backtracking requests that extend beyond the currently loaded history boundary
