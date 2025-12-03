## Multi‑Agent Collaboration Design for `codex-core`

This document sketches how to layer a multi‑agent collaboration system on top of the existing `codex-core` agent loop (sessions, turns, tools, context manager), using the proposed `collaboration.*` tools.

The focus is on:

- Keeping the current single‑agent semantics intact.
- Treating “agents” as *logical participants* within a single Codex session.
- Reusing existing primitives (`Session`, `TurnContext`, `ContextManager`, `ToolsConfig`, tasks) wherever possible.
- Providing clear limits (max agents, max depth, per‑agent “juice”).

---

## 1. Mental Model

- A **Codex session** remains the top‑level unit, with one running `SessionTask` at a time.
- Within a session we introduce a **Collaboration Graph**: a set of logical agents that can exchange messages and call tools, orchestrated entirely within the same session.
- The **main agent** corresponds to the existing single agent Codex exposes today. It gets the user’s prompt and controls collaboration via the new `collaboration.*` tools.
- **Child agents** are lightweight logical agents:
  - Each has its own conversation history (`ContextManager`), view of tools, and model configuration.
  - Each runs its own *logical agentic loop* in a separate Tokio task, continuously consuming its model stream so that server‑side buffers do not depend on the parent calling `wait`.
  - All agents share the same session environment (cwd, sandbox manager, MCP connections, rollout recorder), but their prompts and tool calls are built from their own per‑agent `ContextManager`.
  - `collaboration.wait` becomes a **coordination primitive**: the main agent uses it to block until child agents have made enough progress (tokens/messages) rather than to drive the child loops directly.

Agents, from Codex’s point of view, are essentially:

```rust
struct AgentId(pub u32); // 0-based index

struct AgentState {
    id: AgentId,
    parent: Option<AgentId>,
    depth: u32,

    /// Configuration used to build per-agent TurnContexts.
    config: SessionConfiguration,

    /// Agent-local conversation history (what this agent “remembers”).
    history: ContextManager,

    /// Human-readable instructions / persona for this agent.
    instructions: Option<String>,

    /// Soft “compute budget” in tokens for this agent.
    max_juice: Option<u32>,
    used_juice: u32,

    /// High-level lifecycle state.
    status: AgentLifecycleState,
}
```

The main agent is `AgentId(0)` and is backed by the existing session‑wide `SessionConfiguration` and `SessionState.history`.

Agent lifecycle is modeled explicitly as:

```rust
enum AgentLifecycleState {
    /// Agent is not currently streaming; the last assistant message
    /// (if any) is captured for easy retrieval.
    Idle { last_agent_message: String },
    /// Agent is in the middle of a turn / streaming response.
    Running,
    /// Agent hit a fatal error; `error` contains a concise summary.
    Error { error: String },
    /// Agent was explicitly closed (via `collaboration.close`); no
    /// further messages or tool calls are allowed.
    Closed,
    /// Agent is blocked waiting on a user approval (exec / patch /
    /// other), summarised as `request`.
    WaitingForApproval { request: String },
}
```

---

## 2. Collaboration State and Limits

We introduce a session‑scoped **CollaborationState** owned by `Session`:

```rust
struct CollaborationLimits {
    max_agents: u32,
    max_depth: u32,
}

struct CollaborationState {
    /// Ordered list of agents; index is the AgentId.
    agents: Vec<AgentState>,
    /// Invariants and safety limits.
    limits: CollaborationLimits,
    /// Adjacency for the agent tree: parent → children.
    children: HashMap<AgentId, Vec<AgentId>>,
}
```

Key properties:

- **Lifetime**: `CollaborationState` lives for the duration of the session and persists across turns.
- **Root agent**:
  - Created implicitly when collaboration is first used.
  - Mirrors the current `SessionConfiguration` and `SessionState.history`.
  - Has `parent = None`, `depth = 0`.
- **Limits**:
  - `max_agents`: total agents in `agents` (including root); enforced on `collaboration.init_agent`.
  - `max_depth`: maximum `AgentState.depth`; child creation fails if `parent.depth + 1 > max_depth`.
- **Juice**:
  - Each agent has `max_juice` (optional) and `used_juice`.
  - `used_juice` is derived from per‑agent `ContextManager` token usage (`get_total_token_usage`) and/or incremental deltas per turn.
- **Tree topology**:
  - Agents form a strict **tree**:
    - Each `AgentState` has exactly one `parent` (except the root) and zero or more children.
    - A parent can only interact with its **direct children** (depth‑1 relative to the parent), never grandchildren or siblings.
  - All collaboration tools enforce this:
    - `send`, `wait`, `close` (see §5.5) only accept agents that are direct children of the caller.
    - This keeps reasoning local and prevents tangled cross‑branch coordination.

Storage location:

- Conceptually, `CollaborationState` is part of the session’s mutable state.
- It can be stored alongside `SessionState` (e.g., as an optional field) or in a sibling struct owned by `Session` and protected by a mutex:
  - `SessionState` continues to own the *user‑facing* history.
  - `CollaborationState` owns per‑agent histories, which may be a subset or re‑projection of that history.

---

## 3. Agent Context vs Session Context

### 3.1 Session Configuration and TurnContext

`Session` already owns:

- `SessionConfiguration` (model, cwd, sandbox/approval policies, features, etc.).
- `SessionState.history: ContextManager` (session‑wide conversation transcript).
- `TurnContext` for each user turn (per‑turn model client + environment).

We extend this with **per‑agent configuration**:

- Each `AgentState.config` is a `SessionConfiguration`:
  - Initially cloned from the current session configuration.
  - Overridden by fields provided in `CollaborationInitAgentInput` (model, sandbox policy, approval policy, etc.).
- When Codex wants to run that agent, it builds a per‑agent `TurnContext` by reusing `Session::make_turn_context`:

```rust
fn make_agent_turn_context(
    session: &Session,
    agent: &AgentState,
    sub_id: String,
) -> TurnContext {
    Session::make_turn_context(
        Some(Arc::clone(&session.services.auth_manager)),
        &session.services.otel_event_manager,
        agent.config.provider.clone(),
        &agent.config,
        session.conversation_id,
        sub_id,
    )
}
```

This keeps all the existing plumbing (telemetry, model selection, truncation policy, sandboxing) intact.

### 3.2 Agent Histories vs Session History

We distinguish:

- **Session/global history** (`SessionState.history`): what the user sees in the UI and what the main agent uses as its context. This remains the canonical transcript of the conversation.
- **Agent history** (`AgentState.history`): what each logical agent sees when it reasons:
  - Includes messages *to and from* that agent, as well as tool calls initiated by that agent.
  - Represented as a `ContextManager`, so we retain all invariants and token accounting (call/output pairing, compaction, etc.).

Synchronization strategy:

- The main agent’s history is exactly `SessionState.history`.
- For child agents:
  - On creation, we initialize `AgentState.history` according to `ContextStrategy` (see below).
  - When an agent sends a message or triggers a tool:
    - We append the corresponding `ResponseItem`s into that agent’s `AgentState.history`.
    - We also emit a *summarized* representation into `SessionState.history` (e.g., a compact `Message` describing “Agent 2 replied with: …”) so the user can follow the collaboration.
- This keeps per‑agent context accurate while avoiding flooding the main transcript with every internal detail.

---

## 4. `ContextStrategy` Semantics

The proposed enum:

```rust
enum ContextStrategy {
  #[default]
  New,
  Fork,
  Replace(Vec<ResponseItem>),
}
```

applies when creating a new agent:

- `New`:
  - Agent starts with an *empty* logical history, except for any synthetic system/role messages Codex inserts (see instructions below).
  - Useful for “specialist” agents that should not see the full prior conversation.
- `Fork`:
  - The new agent’s `ContextManager` is initialized by cloning the parent agent’s promptable history:
    - `parent.history.get_history_for_prompt()` is a good starting point.
  - This gives the child agent the same conversational context as the parent at the time of the fork.
- `Replace(Vec<ResponseItem>)`:
  - A more advanced mode, where the caller provides the full history for the agent.
  - In v1 we can limit usage to internal tooling / tests, but keeping it in the API makes training and “rehydration from logs” possible later.

In all cases, we also inject a small system message at the top of the agent’s history describing:

- The agent’s id and role.
- Any initial instructions.
- Its constraints (depth, max_juice, allowed tools).

This system message is created as a `ResponseItem::Message { role: "system", content: … }` and only lives in `AgentState.history`, not in the global transcript.

---

## 5. Tooling: `collaboration.*` Tools

We introduce five tools:

- `collaboration.init_agent`
- `collaboration.send`
- `collaboration.wait`
- `collaboration.get_state`
- `collaboration.close`

They are exposed as standard function tools via `ToolSpec::Function` and implemented as `ToolHandler`s that operate on `Session` + `TurnContext` and `CollaborationState`.

All tools share:

- A common metadata bag:

  ```rust
  pub struct ExtraMetadata(pub HashMap<String, serde_json::Value>);
  ```

- A shared convention:
  - `message_tool_call_success`: true iff the tool handled the request successfully (even if no agent ran).
  - `message_tool_call_error_should_penalize_model`: true when the error is clearly model‑caused (e.g., invalid agent index), false for environment/limit issues.
  - `extra`: structured fields (agent ids, state snapshots, counters) that can evolve without changing the top‑level schema.

### 5.1 `collaboration.init_agent`

**Purpose**

Create a new logical agent as a child of the calling agent.

**Input**

```rust
pub struct CollaborationInitAgentInput {
  /// Index proposed by the model; ignored by the server.
  agent_idx: u32,

  /// New fields (all default to current agent values).
  context_strategy: Option<ContextStrategy>,
  instructions: Option<String>,
  sandbox_policy: Option<SandboxPolicy>,

  /// Future: per-agent model choice.
  model: String,
}
```

Design choices:

- **Agent index ownership**:
  - The server, not the model, owns id assignment.
  - The `agent_idx` input is treated as a hint and ignored; the actual id is returned in metadata:

    ```rust
    extra["agent_idx"] = json!(assigned_id);
    extra["parent_agent_idx"] = json!(caller_agent_id);
    ```

- **Defaults**:
  - `context_strategy`: defaults to `New`.
  - `instructions`: defaults to the parent agent’s instructions (or session’s `developer_instructions` / `user_instructions`).
  - `sandbox_policy`, `model`: default to the session’s current config.
  - The child’s requested `sandbox_policy` is intended to be at least as strict as the parent/session; relaxing policies should result in an error.
    - TODO(jif): define and implement a formal “strictness” ordering for this policy and enforce it in `collaboration.init_agent`.

**Output**

```rust
pub struct CollaborationInitAgentMetadata {
  pub message_tool_call_success: bool,
  pub message_tool_call_error_should_penalize_model: bool,
  pub extra: ExtraMetadata,
}

pub struct CollaborationInitAgentOutput {
  /// Status text from rollout_state.init_agent(...), e.g. success or error reason.
  pub content: String,
  pub metadata: CollaborationInitAgentMetadata,
}
```

Semantics:

- On success:
  - New `AgentState` is added to `CollaborationState.agents`.
  - `extra` includes at least:
    - `agent_idx`: assigned id.
    - `parent_agent_idx`: id of the caller.
    - `depth`: depth of the new agent.
    - `max_juice`: the agent’s juice cap.
  - `message_tool_call_success = true`.
- On failure:
  - `content` is a concise error message (e.g., “max agent count reached”).
  - `message_tool_call_success = false`.
  - `message_tool_call_error_should_penalize_model`:
    - `true` for model mistakes (invalid indices, negative juice, unsupported combination).
    - `false` for environment limits (max agents / depth reached).

### 5.2 `collaboration.send`

**Purpose**

Send a textual message from the calling agent to one or more recipient agents.

**Input**

```rust
pub struct CollaborationSendInput {
  /// Indices of the agents to send the message to.
  /// Must be in [0, num_agents - 1] at runtime.
  pub recipients: Vec<u32>,
  /// The message to send.
  pub message: String,
}
```

Sender identification:

- The sender is the agent currently executing the tool call.
- We track this via `extra` on tool calls:
  - When a tool call originates from a particular agent, we tag it with `extra["caller_agent_idx"]`.
  - Within the handler, we resolve that id from the turn’s collaboration context (see §6).

**Behavior**

For each `recipient`:

- Validate `recipient` against `CollaborationState.agents.len()`.
- Append a `ResponseItem::Message { role: "user", content: … }` to the recipient’s `AgentState.history`, with content enriched to include the sender:

  - E.g. `"From agent 0: <message>"`.

- Optionally, add a summarized `ResponseItem` into the session history so the UI can show “agent‑to‑agent” chatter.

**Output**

```rust
pub struct CollaborationSendMetadata {
  pub message_tool_call_success: bool,
  pub message_tool_call_error_should_penalize_model: bool,
  pub is_send_success_msg: Option<bool>,
  pub message_content_str: Option<String>,
  pub extra: ExtraMetadata,
}

pub struct CollaborationSendOutput {
  pub content: String, // e.g. "Message sent successfully."
  pub metadata: CollaborationSendMetadata,
}
```

Conventions:

- On full success:
  - `is_send_success_msg = Some(true)`.
  - `message_content_str = Some(message.clone())`.
  - `extra["recipients"] = json!(recipients)`.
- On partial success (some valid, some invalid ids):
  - `message_tool_call_success = false`.
  - `message_tool_call_error_should_penalize_model = true`.
  - Content explains which ids failed.

### 5.3 `collaboration.wait`

**Purpose**

Allow the main agent to yield control and synchronize with child agents that are already running in their own loops. This is where we *observe and bound* other agents’ progress, not where we start their streams.

**Input**

```rust
pub struct CollaborationWaitInput {
  /// Maximum duration to wait, measured in tokens. Must be >= 0.
  pub max_duration: u32,
  /// Optional list of child agents to wait for.
  /// Must be direct children of the caller.
  pub recipients: Option<Vec<u32>>,
}
```

Interpretation:

- `max_duration` is a soft token budget *for all agents during this wait call*.
- Codex approximates this in terms of:
  - Incremental `TokenUsageInfo` per agent turn, using their per‑agent `ContextManager`.
  - And/or a fixed per‑turn granularity (e.g., “at most N turns across agents”).

Execution model:

- Collaboration runs as a dedicated `SessionTask` (a “supervisor”) which in turn spawns **one Tokio task per agent**:
  - Each agent task owns its own `run_task` loop, built from a per‑agent `TurnContext` and `ContextManager`.
  - These agent tasks continuously consume their model streams and write results into per‑agent histories and internal queues, so upstream servers never block on the parent.
- `collaboration.wait` is implemented inside the supervisor task as:
  - A blocking point for the **main agent**: it observes per‑agent queues / histories and the total token deltas since the last wait.
  - If `recipients` is `Some`, it only considers those direct children; otherwise it considers *all* direct children of the caller.
  - It blocks until one of the following holds:
    - Enough new tokens have been consumed across the targeted agents (reaching `max_duration`).
    - All targeted agents are idle/completed, out of juice, or closed.
    - Cancellation is requested.
- When `wait` returns, the main agent can inspect child results via `collaboration.get_state` without having had to manually drive each child’s `run_turn`.

**Output**

```rust
pub struct CollaborationWaitMetadata {
  pub message_tool_call_success: bool,
  pub message_tool_call_error_should_penalize_model: bool,
  pub is_wait_success_msg: Option<bool>,
  pub extra: ExtraMetadata,
}

pub struct CollaborationWaitOutput {
  /// Text like "Started waiting successfully." or "Finished waiting."
  pub content: String,
  pub metadata: CollaborationWaitMetadata,
}
```

`extra` can include:

- A snapshot of which agents ran and for how many tokens:

  - `extra["agents_ran"] = json!([{ "agent_idx": 1, "delta_tokens": 128, "status": "running" }, …])`.

- Whether token/juice limits were hit:

  - `extra["token_budget_exhausted"] = json!(true)` when `max_duration` was the stopping reason.

### 5.4 `collaboration.get_state`

**Purpose**

Give the main agent a high‑level view of the collaboration graph (agents, statuses, juice, depth).

**Input/Output**

```rust
pub struct CollaborationGetStateInput {
  // No fields; the call takes an empty object.
}

pub struct CollaborationGetStateMetadata {
  /// Always true on successful get_state.
  pub message_tool_call_success: Option<bool>,
  pub message_tool_call_error_should_penalize_model: Option<bool>,
  pub extra: ExtraMetadata,
}

pub struct CollaborationGetStateOutput {
  /// Human-readable description of all agents' states.
  pub content: String,
  pub metadata: CollaborationGetStateMetadata,
}
```

Implementation:

- Walk `CollaborationState.agents` and construct:
  - A human‑readable table/list in `content` (suitable for the model to parse informally).
  - A structured mirror in `extra["agents"]`, e.g.:

    ```json
    [
      {
        "agent_idx": 0,
        "parent_agent_idx": null,
        "depth": 0,
        "status": "running",
        "used_juice": 512,
        "max_juice": 2000
      },
      ...
    ]
    ```

- `message_tool_call_success = Some(true)` on success; errors are unlikely here.

`get_state` is the **primary** way the main agent inspects child progress and results. It is expected to grow richer over time (e.g., enums for lifecycle state, last result metadata) even if the top‑level schema remains stable.

### 5.5 `collaboration.close`

**Purpose**

Allow a parent agent to explicitly close one or more of its child agents. Closing an agent:

- Stops its Tokio task and prevents further model calls.
- Recursively closes all of its descendants in the agent tree.
- Marks their lifecycle state as `Closed` while keeping enough metadata for inspection via `get_state`.

**Input**

```rust
pub struct CollaborationCloseInput {
  /// Indices of the agents to close.
  /// Each must be a direct child of the caller.
  pub recipients: Vec<u32>,
}
```

**Output**

```rust
pub struct CollaborationCloseMetadata {
  pub message_tool_call_success: bool,
  pub message_tool_call_error_should_penalize_model: bool,
  pub extra: ExtraMetadata,
}

pub struct CollaborationCloseOutput {
  /// Human-readable status, e.g. "Closed 2 agents (and their descendants)."
  pub content: String,
  pub metadata: CollaborationCloseMetadata,
}
```

Semantics:

- On success:
  - For each `recipient` that is a direct child of the caller:
    - Cancel its Tokio task and mark it `Closed`.
    - Recursively do the same for all its descendants using the `children` adjacency.
  - `extra` may include:
    - `closed_agent_indices`: all agents that were closed.
  - `message_tool_call_success = true`.
- On failure:
  - If any `recipient` is not a direct child of the caller (or refers to the root), reject the call:
    - `message_tool_call_success = false`.
    - `message_tool_call_error_should_penalize_model = true`.
    - `content` explains the violation.

---

## 6. Mapping Tool Calls to Agents

Because all agents share the same tool ecosystem, we need a way to know *which agent* is currently issuing a tool call.

Approach:

- Associate each `TurnContext` / `sub_id` with an `AgentId` inside `CollaborationState`:
  - When the supervisor spawns an agent’s Tokio task, it registers a mapping from that agent’s `sub_id` (or a dedicated identifier in the `TurnContext`) to `AgentId`.
  - All model streams and tool calls originating from that agent run under that `TurnContext`.
- When building tool calls in `ToolRouter::build_tool_call`, we extend `ToolPayload` or `ToolInvocation` with:

  - `caller_agent_idx: AgentId` (looked up from the current `TurnContext`’s identifier).

- The collaboration tool handlers read `caller_agent_idx` to:
  - Determine parent/child relationships for `init_agent`.
  - Record messages in the correct `AgentState.history` for `send`.
  - Attribute juice consumption to the right agent for `wait` and enforce that parents only target direct children.

This separation keeps the rest of the tool ecosystem unchanged: non‑collaboration tools are unaware of agents and continue to operate exactly as before, but their events are recorded in the appropriate agent’s history when run on behalf of that agent.

---

## 7. Limits, Safety, and Error Handling

### 7.1 Max Agents and Depth

Enforced in `CollaborationState`:

- On `init_agent`:
  - If `agents.len() >= max_agents` → reject with a clear error string and `message_tool_call_success = false`.
  - If `parent.depth + 1 > max_depth` → reject similarly.
- The main agent (id 0) always exists once collaboration is used and does not count towards depth.

### 7.2 Juice and Token Accounting

- Per agent:
  - `used_juice` is derived from that agent’s `ContextManager::get_total_token_usage()`.
  - `max_juice` comes from the `juice` field on `CollaborationInitAgentInput`.
  - After each agent turn, we compute the delta and update `used_juice`.
  - Once `used_juice >= max_juice`, the agent’s status is set to `AgentLifecycleState::Exhausted` and `wait` will no longer schedule it.
- Per `wait` call:
  - Track a separate counter for the duration of that call based on token deltas across all agents that ran.

### 7.3 Error Surfaces

Examples:

- Invalid agent indices:
  - `init_agent`: if the parent agent id is invalid, this is a model bug; penalize.
  - `send`: out‑of‑bounds recipients; penalize.
  - `wait`/`get_state`: essentially infallible; errors here are environment errors.
- Limit violations:
  - Max agents / depth / juice: treat as environment constraints; do **not** penalize the model but describe limits clearly in the error content.

---

## 8. Integration with Existing Components

### 8.1 `Session` and `SessionState`

Add collaboration‑related helpers on `Session`:

- `fn collaboration(&self) -> &CollaborationStateHandle` to access the shared state (behind a mutex).
- Helper methods to:
  - Initialize the root agent from the current session config.
  - Get the `AgentState` for the “main” agent (id 0).
  - Map `sub_id` or tool call metadata to `AgentId`.

`SessionState` can remain focused on global history and token/rate limit tracking; collaboration uses `ContextManager` in parallel for per‑agent history.

### 8.2 `run_task` / `run_turn`

The single‑agent path (today’s behavior) remains:

- `run_task` builds `prompt` from *session* history and calls `run_turn`.

For collaboration, we introduce a **CollaborationTask**:

- A specialized `SessionTask` that:
  - Runs the main agent’s `run_task` loop (the one that can call `collaboration.*` tools).
  - Spawns and supervises one Tokio task per child agent, each running its own `run_task` / `run_turn` loop with a per‑agent `TurnContext` and `ContextManager`.
  - Maintains per‑agent queues and histories so the main agent can retrieve child outputs cheaply.
- `collaboration.wait` is implemented within this CollaborationTask as a synchronization point with those per‑agent tasks; the agents themselves do not depend on `wait` to consume their model streams.

We still **reuse** the existing `run_turn` and streaming/tool machinery: collaboration only changes how many `run_task` loops we run in parallel and how we coordinate them, not the semantics of a single agent’s interaction with the model.

### 8.3 Tooling and Registry

We add collaboration handlers under `core/src/tools/handlers/collaboration.rs`:

- Define `ToolSpec`s for the four tools, matching the Rust structs defined above (via `JsonSchema`).
- Implement `ToolHandler` for each, reading/writing `CollaborationState`, `Session`, and `TurnContext`, and returning structured outputs.

The `ToolRouter` remains unchanged; collaboration is just another tool namespace (`"collaboration.init_agent"`, etc.).

---

## 9. Rollout and Telemetry

Because all agent interactions still happen within a single `Session`:

- **Rollout recorder**:
  - We continue to record `RolloutItem::ResponseItem` for:
    - Messages from the main agent.
    - Summaries of child agents’ activity (collaboration events).
  - Per‑agent histories live alongside this and can be serialized separately if needed for debugging.
- **Telemetry (OTel)**:
  - For child agents, we re‑use the existing `OtelEventManager`, potentially tagging:
    - `agent_idx`, `parent_agent_idx`, `depth` as attributes on tool and model spans.

This gives us visibility into:

- How many agents are spawned per session.
- How much juice they consume.
- How collaboration changes overall token usage and user outcomes.

---

## 10. Implementation Outline

1. **Core types and state**
   - Introduce `AgentId`, `AgentState`, `CollaborationLimits`, `CollaborationState`.
   - Add a collaboration handle to `Session` (with appropriate locking).
   - Implement methods for creating the root agent and looking up agents.
2. **Per‑agent configuration**
   - Extend `SessionConfiguration` cloning logic to produce per‑agent configs.
   - Implement `make_agent_turn_context` helpers.
3. **Tool specs and handlers**
   - Add `collaboration.rs` handler module.
   - Define `ToolSpec`s for `collaboration.init_agent`, `.send`, `.wait`, `.get_state`, `.close`.
   - Implement tool handlers using the structs in the prompt and `ExtraMetadata`.
4. **Agent scheduling**
   - Implement a `CollaborationTask` supervisor that spawns one Tokio task per agent, tracks per‑agent token deltas/juice, and implements `collaboration.wait` as a synchronization barrier over those tasks rather than directly calling `run_turn`.
5. **History integration**
   - Decide on a minimal, stable projection of agent activity into `SessionState.history` (e.g., short assistant messages summarizing agent actions).
   - Ensure per‑agent histories remain valid `ContextManager`s (respecting call/output invariants).
6. **Limits and validation**
   - Enforce `max_agents`, `max_depth`, and per‑agent `max_juice`.
   - Wire `message_tool_call_error_should_penalize_model` for model vs environment errors.

This design keeps the existing Codex single‑agent loop intact while introducing a structured, tool‑driven multi‑agent collaboration model with clear state boundaries and safety limits.
