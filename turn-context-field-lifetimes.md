# `TurnContext` field lifetimes

`TurnContext` is not actually turn-scoped as a whole. It contains four different kinds of state:

- snapshots of mutable thread settings;
- session-wide live handles;
- values resolved specifically for one turn;
- genuinely mutable turn state.

Physically, every field lives as long as the longest `Arc<TurnContext>`. Normally that is turn completion, but unified-exec watchers retain it until the process exits, and the reusable Guardian trunk can retain the originating context for effectively the rest of the session.

Relevant ownership points:

- [`RunningTask` retains the context](codex-rs/core/src/state/turn.rs#L72).
- [The spawned task captures another `Arc`](codex-rs/core/src/tasks/mod.rs#L374).
- [Unified-exec exit watchers retain the context](codex-rs/core/src/unified_exec/async_watcher.rs#L107).
- [Guardian subagent forwarding captures the parent context](codex-rs/core/src/codex_delegate.rs#L142).

## Terminology

- **Durable**: copied into `TurnContextItem`, persisted to the rollout, and retained as the in-memory reference context for later turns.
- **Next turn**: copied into `PreviousTurnSettings` for decisions made during a subsequent turn.
- **MCP**: copied into the session-held MCP manager or its runtime context.
- **Child/review**: copied into a child-agent configuration, review context, or alternate-model context.
- **Live use**: read later through a retained `Arc<TurnContext>`, without a separate longer-lived projection.

## Field inventory

| Field | Real semantic lifetime and origin | Captured and used later |
|---|---|---|
| `sub_id` | Genuine turn/task identifier | Durable; copied into metadata, MCP manager, events, approvals, and child/review machinery |
| `trace_id` | Snapshot of the tracing span when the context is created | Copied into regular, compact, remote-compact, and user-shell task metadata; not durable |
| `realtime_active` | Runtime snapshot taken when the context is built; not live conversation state | Durable and Next turn; copied to review context |
| `config` | Newly allocated per-turn `Config`, mostly copied from thread configuration but rewritten with selected cwd, workspace roots, permission state, reasoning, service tier, and related values | Parts are Durable; full config is cloned into child agents, review, and alternate-model compaction |
| `auth_manager` | Live session-wide shared service handle, not a snapshot | Used later by `apps_enabled()` and copied into review contexts; otherwise redundant with session/provider access |
| `model_info` | Per-turn lookup of model catalog metadata for the selected thread model | Model slug is Durable and Next turn; full object drives sampling/tools and is copied or replaced in child/review contexts |
| `session_telemetry` | Session telemetry handle cloned and rebound to this turn's model and slug | Used through turn finish and copied/rebound for review and alternate-model contexts |
| `provider` | Recreated per turn from thread provider configuration plus live auth | Used for sampling and compaction; provider info is copied into child config and review |
| `reasoning_effort` | Snapshot of thread collaboration settings, with model compatibility adjustment in `with_model` | Durable; copied into model requests, child config, review, and compaction |
| `reasoning_summary` | Thread setting or model-default-derived turn value | Used for model, compact, and child requests; notably not persisted from this field—the rollout compatibility field is hardcoded to `Auto` |
| `session_source` | Immutable thread identity copied into every turn | Copied into metadata and review; child threads derive a new `SessionSource::SubAgent` |
| `parent_thread_id` | Immutable thread relationship copied into every turn | Copied into request metadata, review, and agent-tree handling |
| `environments` | Immutable turn snapshot of thread environment selections | Used throughout tools; selections and cwd escape into MCP and child sessions; review copies it. Underlying `Environment` Arcs and shell-snapshot futures are shared across turns |
| `cwd` | Turn-derived legacy projection: primary selected-environment cwd, otherwise thread fallback cwd | Durable, MCP, metadata, child, and review capture. This is explicitly deprecated and is the largest compatibility duplicate |
| `current_date` | Genuine creation-time turn snapshot | Durable; used in model environment context and copied to review |
| `timezone` | Genuine creation-time turn snapshot | Durable; used in model environment context and copied to review |
| `app_server_client_name` | Thread/session setting snapshot | Used later for hook payloads and plugin-install/tool exposure behavior; copied to review |
| `developer_instructions` | Thread configuration snapshot | Materialized into model history; copied into child config and `with_model`; deliberately cleared for review |
| `user_instructions` | Rendered snapshot of thread-loaded `AGENTS.md` instructions | Materialized into model history; preserved by `with_model`, cleared for review. Delegated children reload from the parent session rather than this field |
| `collaboration_mode` | Mutable thread setting frozen for this turn | Durable; used throughout the task and copied to review and alternate-model contexts |
| `multi_agent_version` | Session-latched value selected once and then copied into turns | Durable; used for tools and lifecycle. Review forces it to `Disabled` |
| `personality` | Mutable thread setting frozen for this turn | Durable; copied into prompts, compaction, and review |
| `approval_policy` | Mutable thread setting frozen for this turn | Durable; also copied into MCP, child config, review, and approval paths |
| `permission_profile` | Materialized per-turn permission snapshot derived from thread state and workspace roots | Durable; copied into MCP, metadata sandbox tag, child config, review, and execution paths |
| `network` | Turn-selected handle to the live session proxy. The choice of `Some` or `None` is turn-specific, but internals remain shared and live | Used by shell and unified exec and copied to review. Only `is_some()` enters metadata. `TurnContextItem.network` comes from config requirements, not this field |
| `windows_sandbox_level` | Mutable thread setting frozen for this turn | Used by execution and metadata sandbox tagging; copied to review. Child config inherits it indirectly through cloned `config` |
| `available_models` | Snapshot of the models-manager cache at turn creation | Used later only to build multi-agent/model-selection tool specs; review obtains a new list |
| `unified_exec_shell_mode` | Derived from session features, user shell, and runtime paths | Used later by unified-exec tool specs and runtime; recalculated for review |
| `final_output_json_schema` | Genuine request/turn-specific value | Copied into each model `Prompt`; not durable; review clears it |
| `dynamic_tools` | Thread/session configuration copied into every turn | Used by tool-router construction; copied into review |
| `turn_metadata_state` | Genuine mutable turn state | Shared by `with_model`; read for every model and MCP request. Its Git enrichment task clones this state independently and can briefly outlive `TurnContext` |
| `extension_data` | Genuine mutable turn extension store | Explicitly cloned into `RunningTask` and `SessionTaskContext`; shared by alternate-model contexts |
| `turn_skills` | Per-turn resolved skills outcome plus a genuinely turn-local implicit-invocation dedupe set | Outcome is also inserted into extension data. `with_model` shares both; review shares the outcome but creates a fresh dedupe set |
| `turn_timing_state` | Genuine mutable turn/task timing state | Sampling and tool guards retain it independently; read through completion or abort; shared by `with_model` |
| `server_model_warning_emitted` | Per-context response-stream dedupe flag | Used during streaming. `with_model` makes a distinct atomic initialized from the old value; review resets it |
| `model_verification_emitted` | Per-context response-stream dedupe flag | Streaming-only; copied by value into `with_model` and reset for review |

## Independent longer-lived projections

- [`TurnContextItem`](codex-rs/core/src/session/turn_context.rs#L409) is the durable rollout and reference-context projection.
- [`PreviousTurnSettings`](codex-rs/core/src/session/turn.rs#L175) retains model, compatibility hash, and realtime state for the next turn.
- [MCP refresh](codex-rs/core/src/session/mcp.rs#L320) captures selected cwd/environment, approval policy, turn ID, and permission profile into a session-held manager.
- [`TurnMetadataState`](codex-rs/core/src/turn_metadata.rs#L85) captures identity, cwd, policy, and metadata into independently mutable turn state.
- [Child-agent config construction](codex-rs/core/src/tools/handlers/multi_agents_common.rs#L220) copies model, provider, reasoning, instructions, approval, shell, cwd, permissions, and sandbox paths.
- [Review context construction](codex-rs/core/src/session/review.rs#L109) copies most of the parent context into another `TurnContext`.
- [`with_model`](codex-rs/core/src/session/turn_context.rs#L290) preserves nearly the entire context while replacing model-dependent fields.

## Architectural conclusions

- The genuinely turn-owned core is small: IDs and tracing, metadata, timing, extension state, skill dedupe, stream dedupe, final schema, and turn-resolved model/environment state.
- Most of the struct is a flattened thread-settings snapshot.
- `auth_manager` and probably `provider` are session services masquerading as turn state.
- Feature flags, shell policy, Linux sandbox executable, compact prompt, compatibility hash, truncation policy, and tool mode now come directly from `config` or `model_info` instead of duplicate `TurnContext` fields.
- `cwd` is duplicated legacy state and should disappear once consumers consistently use the selected turn environment.
- Mutable turn state such as metadata, timing, extension data, and stream dedupe would be clearer in a separate runtime-state object rather than mixed into an otherwise immutable snapshot.
- Background consumers should capture explicit narrow payloads rather than `Arc<TurnContext>`.

The most concerning lifetime leak is Guardian: the reusable trunk can retain and later consult the parent context from the turn that originally created it. That is not only memory retention; it creates a stale-authority boundary around cwd, approvals, permissions, and related settings.
