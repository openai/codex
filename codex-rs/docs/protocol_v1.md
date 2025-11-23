Overview of Protocol Defined in [protocol.rs](../protocol/src/protocol.rs) and the Codex engine in [codex.rs](../core/src/codex.rs).

The goal of this document is to define terminology used in the system and explain the expected behavior of the system.

NOTE: This document summarizes the protocol at a high level. The Rust types and enums in [protocol.rs](../protocol/src/protocol.rs) are the source of truth and may occasionally include additional fields or variants beyond what is covered here.

## Entities

These are entities that exist on the Codex backend. The intent of this section is to establish vocabulary and construct a shared mental model for the `Codex` core system.

0. `Model`
   - In our case, this is the Responses REST API
1. `Codex`
   - The core engine of codex
   - Runs locally, either in a background thread or separate process
   - Communicated to via a queue pair – SQ (Submission Queue) / EQ (Event Queue)
   - Takes user input, makes requests to the `Model`, executes commands and applies patches.
2. `Session`
   - The `Codex`'s current configuration and state
   - `Codex` starts with no `Session`, and it is initialized by `Op::ConfigureSession`, which should be the first message sent by the UI.
   - The current `Session` can be reconfigured with additional `Op::ConfigureSession` calls.
   - Any running execution is aborted when the session is reconfigured.
3. `Task`
   - A `Task` is `Codex` executing work in response to user input.
   - `Session` has at most one `Task` running at a time.
   - Receiving `Op::UserInput` starts a `Task`
   - Consists of a series of `Turn`s
   - The `Task` executes to until:
     - The `Model` completes the task and there is no output to feed into an additional `Turn`
     - Additional `Op::UserInput` aborts the current task and starts a new one
     - UI interrupts with `Op::Interrupt`
     - Fatal errors are encountered, eg. `Model` connection exceeding retry limits
     - Blocked by user approval (executing a command or patch)
4. `Turn`
   - One cycle of iteration in a `Task`, consists of:
     - A request to the `Model` - (initially) prompt + (optional) `last_response_id`, or (in loop) previous turn output
     - The `Model` streams responses back in an SSE, which are collected until "completed" message and the SSE terminates
     - `Codex` then executes command(s), applies patch(es), and outputs message(s) returned by the `Model`
     - Pauses to request approval when necessary
   - The output of one `Turn` is the input to the next `Turn`
   - A `Turn` yielding no output terminates the `Task`

The term "UI" is used to refer to the application driving `Codex`. This may be the CLI / TUI chat-like interface that users operate, or it may be a GUI interface like a VSCode extension. The UI is external to `Codex`, as `Codex` is intended to be operated by arbitrary UI implementations.

### Agent identifiers

Every participant in a session (the root UI thread plus each spawned/forked child) is assigned a monotonically increasing numeric `AgentId`. Agent `0` is always the root thread. Subagents inherit their parent's `AgentId` as `parent_agent_id` so UIs can correlate trees even when conversations are forked or exported. These IDs are surfaced in `SubagentSummary` payloads and in a dedicated inbox event described below.

When a `Turn` completes, the `response_id` from the `Model`'s final `response.completed` message is stored in the `Session` state to resume the thread given the next `Op::UserInput`. The `response_id` is also returned in the `EventMsg::TurnComplete` to the UI, which can be used to fork the thread from an earlier point by providing it in the `Op::UserInput`.

Each `Session` still runs at most one `Task` at a time. For parallel work, you can either run multiple Codex sessions or use subagents (via the `subagent_*` tools) to orchestrate multiple child sessions within a single daemon.

Subagent sessions run in parallel with the root thread, so you scale overlapping conversations without launching new daemons.
Enable the `subagent_tools` feature flag (see `../../docs/config.md#feature-flags`) and tune how many child sessions stay active with `max_active_subagents` (`../../docs/config.md#max_active_subagents`).

## Interface

- `Codex`
  - Communicates with UI via a `SQ` (Submission Queue) and `EQ` (Event Queue).
- `Submission`
  - These are messages sent on the `SQ` (UI -> `Codex`)
  - Has an string ID provided by the UI, referred to as `sub_id`
  - `Op` refers to the enum of all possible `Submission` payloads
    - This enum is `non_exhaustive`; variants can be added at future dates
- `Event`
  - These are messages sent on the `EQ` (`Codex` -> UI)
  - Each `Event` has a non-unique ID, matching the `sub_id` from the `Op::UserInput` that started the current task.
  - `EventMsg` refers to the enum of all possible `Event` payloads
    - This enum is `non_exhaustive`; variants can be added at future dates
    - It should be expected that new `EventMsg` variants will be added over time to expose more detailed information about the model's actions.

For complete documentation of the `Op` and `EventMsg` variants, refer to [protocol.rs](../protocol/src/protocol.rs). Some example payload types:

- `Op`
  - `Op::UserInput` – Any input from the user to kick off a `Task`
  - `Op::Interrupt` – Interrupts a running task
  - `Op::ExecApproval` – Approve or deny code execution
- `EventMsg`
  - `EventMsg::AgentMessage` – Messages from the `Model`
  - `EventMsg::ExecApprovalRequest` – Request approval from user to execute a command
  - `EventMsg::TaskComplete` – A task completed successfully
  - `EventMsg::Error` – A task stopped with an error
  - `EventMsg::Warning` – A non-fatal warning that the client should surface to the user
  - `EventMsg::TurnComplete` – Contains a `response_id` bookmark for last `response_id` executed by the task. This can be used to continue the task at a later point in time, perhaps with additional user input.
- `EventMsg::SubagentLifecycle` – Emits `SubagentSummary` payloads that describe each child session, including its `agent_id`, `parent_agent_id`, and current pending inbox counts.
  These lifecycle events are emitted whenever the daemon’s view of a subagent changes (creation, status/reasoning-header updates, or removal). They also persist in rollout files so `codex resume` can rebuild prior subagent state—including attachments on spawn/fork and detach on cancel/prune—before replaying model turns.
- `EventMsg::AgentInbox` – Notifies the UI when a subagent’s inbox depth changes, for example after the parent sends an interrupt or a watchdog ping arrives. The payload includes the target `agent_id`, `session_id`, and the counts of pending regular vs interrupt messages so UIs can render badges without polling.
  For example, if the root interrupts child agent `3`, the UI may receive an `AgentInbox` event for `agent_id = 3` showing one pending interrupt message and zero regular messages.

#### Subagent tool reminders

- `subagent_await` accepts an optional `timeout_s` capped at 1,800 s (30 minutes). Omit it or pass `0` to use the 30-minute default. Each `timeout_s` must be at least 300 s (5 minutes); prefer 5–30 minute timeouts and use backoff (for example, 300s → 600s → 1,200s) so you can check on children, log progress, or deliver interrupts instead of parking for the full cap.
- `subagent_logs` is read-only and does not change a child’s state; prefer it when you only need to inspect recent activity without advancing the subagent.

The `response_id` returned from each task matches the OpenAI `response_id` stored in the API's `/responses` endpoint. It can be stored and used in future `Sessions` to resume threads of work.

## Transport

Can operate over any transport that supports bi-directional streaming. - cross-thread channels - IPC channels - stdin/stdout - TCP - HTTP2 - gRPC

Non-framed transports, such as stdin/stdout and TCP, should use newline-delimited JSON in sending messages.

## Example Flows

Sequence diagram examples of common interactions. In each diagram, some unimportant events may be eliminated for simplicity.

### Basic UI Flow

A single user input, followed by a 2-turn task

```mermaid
sequenceDiagram
    box UI
    participant user as User
    end
    box Daemon
    participant codex as Codex
    participant session as Session
    participant task as Task
    end
    box Rest API
    participant agent as Model
    end
    user->>codex: Op::ConfigureSession
    codex-->>session: create session
    codex->>user: Event::SessionConfigured
    user->>session: Op::UserInput
    session-->>+task: start task
    task->>user: Event::TaskStarted
    task->>agent: prompt
    agent->>task: response (exec)
    task->>-user: Event::ExecApprovalRequest
    user->>+task: Op::ExecApproval::Allow
    task->>user: Event::ExecStart
    task->>task: exec
    task->>user: Event::ExecStop
    task->>user: Event::TurnComplete
    task->>agent: stdout
    agent->>task: response (patch)
    task->>task: apply patch (auto-approved)
    task->>agent: success
    agent->>task: response<br/>(msg + completed)
    task->>user: Event::AgentMessage
    task->>user: Event::TurnComplete
    task->>-user: Event::TaskComplete
```

### Task Interrupt

Interrupting a task and continuing with additional user input.

```mermaid
sequenceDiagram
    box UI
    participant user as User
    end
    box Daemon
    participant session as Session
    participant task1 as Task1
    participant task2 as Task2
    end
    box Rest API
    participant agent as Model
    end
    user->>session: Op::UserInput
    session-->>+task1: start task
    task1->>user: Event::TaskStarted
    task1->>agent: prompt
    agent->>task1: response (exec)
    task1->>task1: exec (auto-approved)
    task1->>user: Event::TurnComplete
    task1->>agent: stdout
    task1->>agent: response (exec)
    task1->>task1: exec (auto-approved)
    user->>task1: Op::Interrupt
    task1->>-user: Event::Error("interrupted")
    user->>session: Op::UserInput w/ last_response_id
    session-->>+task2: start task
    task2->>user: Event::TaskStarted
    task2->>agent: prompt + Task1 last_response_id
    agent->>task2: response (exec)
    task2->>task2: exec (auto-approve)
    task2->>user: Event::TurnCompleted
    task2->>agent: stdout
    agent->>task2: msg + completed
    task2->>user: Event::AgentMessage
    task2->>user: Event::TurnCompleted
    task2->>-user: Event::TaskCompleted
```
