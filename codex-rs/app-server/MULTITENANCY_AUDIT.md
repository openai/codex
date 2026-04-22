# App-Server Multi-Tenancy Audit

Date: 2026-04-16
Updated: 2026-04-22

Scope: `codex-rs/app-server`, the app-server protocol surface, and the core/login/state/thread-store components that app-server directly depends on.

## Executive Summary

The current app-server is multi-connection, but it is not multi-tenant. A single app-server process currently owns exactly one effective runtime tenant: one base `Config`, one `CODEX_HOME`, one SQLite home, one auth manager, one cloud-requirements loader, one thread manager, one plugin/skills/MCP/model manager graph, one feedback/log pipeline, one set of process-global HTTP client metadata, and one outbound broadcast space.

The safest first design for Codex Cloud is to make each connection or remote-control stream bind to one authenticated tenant and route all requests through a server-derived `TenantContext`. The tenant id used by the runtime registry should be the app-server `IdentityKey`: opaque bytes supplied by the trusted launcher or derived from transport/remote-control auth, compared and hashed by app-server but not interpreted. Do not let clients self-declare an identity key without binding that declaration to transport auth. The lower-risk persistence model is per-tenant root isolation: each tenant gets its own `codex_home`, `sqlite_home`, log directory, auth store, plugin store, memories, sessions, and local config files. A single shared state DB is possible, but would require tenant columns and query filters across thread metadata, spawn edges, jobs, memories, logs, remote-control enrollments, rollout indexes, and legacy filesystem session scanning.

## Current Tenant Boundary

There is no explicit tenant boundary today.

- `run_main_with_transport` constructs the process-wide `EnvironmentManager`, base config, cloud requirements, state DB, log DB, transport router, remote-control connection, and `MessageProcessor` once at process startup (`app-server/src/lib.rs:355-672`).
- `MessageProcessor` owns one `CodexMessageProcessor`, one `ConfigApi`, one `ExternalAgentConfigApi`, one `FsApi`, one `AuthManager`, one `FsWatchManager`, one `Config`, and one config warning list (`app-server/src/message_processor.rs:163-176`).
- `ConnectionSessionState` stores only initialization state, experimental API opt-in, notification opt-outs, and client name/version. It has no principal, tenant id, workspace/account id, or authorization scope (`app-server/src/message_processor.rs:178-189`).
- Once initialized, request dispatch only checks initialized/experimental state, then forwards to handlers. There is no tenant authorization check at dispatch time (`app-server/src/message_processor.rs:721-772`).
- Thread creation notifications auto-attach every initialized connection to every new thread in the process (`app-server/src/lib.rs:844-858`).

This means a shared app-server process would currently let any initialized connection list, load, resume, subscribe to, mutate, archive, or receive notifications for any thread known to the process.

## Required Architecture Changes

### 1. Add Server-Derived IdentityKey Tenant Identity

Use `IdentityKey` as the tenant id for app-server runtime isolation. `IdentityKey` is intentionally opaque bytes; app-server may clone, compare, hash, log a redacted digest, and encode it for storage paths, but must not parse it into account/workspace/user semantics. Authorization and storage-boundary meaning belong to the remote contract implementations and the trusted identity issuer.

Recommended connection model:

- WebSocket upgrade auth validates the caller and returns a principal containing an `IdentityKey`, not just `Ok(())`.
- Remote-control streams derive the `IdentityKey` from the remote-control account/environment/server enrollment, not from the JSON-RPC payload.
- Current single-process startup identity can seed the default tenant runtime for non-multiplexed deployments.
- A connection is single-tenant for its lifetime.
- All JSON-RPC request handling receives a `TenantContext` selected by `connection_id`.

Alternative model:

- A request envelope carries a tenant selector, but the server must verify it maps to the authenticated principal's `IdentityKey` on every request.
- This is more flexible for multiplexed gateway connections, but expands the attack surface and requires more protocol churn.

The existing WebSocket auth only verifies capability tokens or signed bearer JWT validity and discards claims (`transport/auth.rs:273-304`). The listener state stores only `transport_event_tx` and `auth_policy`, then opens a raw connection id (`transport/websocket.rs:74-121`). That needs to become `authorize_upgrade -> AuthorizedPrincipal` and `TransportEvent::ConnectionOpened` should carry the principal.

### 2. Add a Tenant Runtime Registry

Add a process-wide tenant registry owned above `MessageProcessor`. The registry key is `IdentityKey`; there should not be a separate app-server-assigned tenant id unless it is just a local alias for the same opaque bytes.

```rust
struct TenantRegistry {
    tenants: DashMap<IdentityKey, Arc<TenantRuntime>>,
}

struct TenantRuntime {
    identity_key: IdentityKey,
    codex_home: AbsolutePathBuf,
    config_manager: TenantConfigManager,
    auth_manager: Arc<AuthManager>,
    cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
    thread_manager: Arc<ThreadManager>,
    state_db: Option<Arc<StateRuntime>>,
    log_db: Option<LogDbLayer>,
    feedback: CodexFeedback,
    config_api: ConfigApi,
    external_agent_config_api: ExternalAgentConfigApi,
    fs_api: TenantFsApi,
    fs_watch_manager: FsWatchManager,
    command_exec_manager: CommandExecManager,
}
```

Key rule: handlers should not read process-global `self.config`, `self.auth_manager`, `self.thread_manager`, etc. They should receive a `TenantRuntime` resolved from the connection.

Tenant runtime lifecycle needs explicit policies:

- lazy-create on first authenticated connection;
- ref-count or idle unload tenant runtimes;
- bounded per-tenant resources: max loaded threads, command sessions, file watches, background tasks, memory jobs, MCP servers;
- per-tenant shutdown/drain instead of only process-wide graceful drain.

### 3. Pick Persistence Isolation Strategy

Use per-tenant roots first unless there is a strong product need for cross-tenant SQL queries.

Per-tenant root isolation:

- `IdentityKey` maps to a deterministic root such as `$APP_SERVER_STATE/tenants/{encoded_identity_key}/codex_home`.
- Do not place raw identity bytes directly in paths. Use a canonical path-safe encoding such as base64url without padding, or a SHA-256 digest if path names should not reveal even encoded key material.
- Each tenant runtime builds config with `.codex_home(tenant_codex_home)`.
- Each tenant gets separate `state_*.sqlite`, `logs_*.sqlite`, sessions, archived sessions, memory directories, plugin installs, MCP credentials, auth file/keyring key, and history.
- This avoids broad SQL migrations because `StateRuntime::init` already scopes DBs under a provided root (`state/src/runtime.rs:77-150`).

Shared DB isolation:

- Add `tenant_id` to `threads`, `thread_spawn_edges`, `stage1_outputs`, `jobs`, `agent_jobs`, logs tables, remote-control enrollments, and all unique/index definitions. This column should store the canonical serialized `IdentityKey` bytes or path-safe encoding, not a separately invented account/workspace id.
- Convert primary keys from `id` or `(kind, job_key)` to include `tenant_id`.
- Update every query to filter by `tenant_id`. Examples today list and mutate by bare `threads.id` (`state/src/runtime/threads.rs:367-535`).
- Audit legacy rollout filesystem scanning, session index files, and archive/unarchive paths.

The current `threads` schema uses `id TEXT PRIMARY KEY` and has no tenant dimension (`state/migrations/0001_threads.sql:1-24`). Memory reset deletes every memory row in the current DB and all memory jobs (`state/src/runtime/memories.rs:26-55`), so shared DB isolation would be high-risk until every delete/update is tenant-filtered.

## Subsystem Findings And Required Changes

### Protocol And Request Routing

Findings:

- `ClientRequest` contains only `id`, `method`, and `params`; no tenant context exists in the envelope (`app-server-protocol/src/protocol/common.rs:87-101`).
- Thread APIs use bare `thread_id` strings (`v2.rs:2687-2735`, `v2.rs:3218-3245`).
- Filesystem APIs accept absolute host paths directly (`v2.rs:2303-2325`).
- Feedback accepts optional `thread_id` and arbitrary `extra_log_files` (`v2.rs:2283-2293`).

Required changes:

- Add a server-side `ConnectionTenantMap` and require every initialized request to resolve to one tenant before dispatch.
- Keep public protocol unchanged for first pass if connections are single-tenant; document that `thread_id` is tenant-scoped.
- If gateway multiplexing is required, add tenant metadata to a higher-level envelope or `initialize`, and reject mismatches against auth claims.
- Add a thread ownership check before every method that takes `thread_id`.
- Replace arbitrary path params in remote/cloud contexts with tenant workspace handles, thread-scoped roots, or capability-bound file handles.

### Transport And Remote Control

Findings:

- WebSocket auth returns only success/failure (`transport/auth.rs:273-304`).
- WebSocket listener opens a connection id without attaching identity (`transport/websocket.rs:100-121`).
- Remote control is started once with one `StateRuntime` and one `AuthManager` (`transport/remote_control/mod.rs:47-74`).
- Remote-control websocket state stores one auth manager, one enrollment, and one client tracker (`transport/remote_control/websocket.rs:116-158`).
- Remote-control clients are tracked by `(ClientId, StreamId)` to `ConnectionId`, with no tenant field (`transport/remote_control/client_tracker.rs:28-60`).
- Remote-control enrollment is keyed by websocket URL, account id, and app-server client name (`transport/remote_control/websocket.rs:724-790`), but it still feeds into the single process tenant today.

Required changes:

- Make WebSocket auth produce `AuthorizedPrincipal { identity_key, account_id, scopes, auth_method }`.
- Add principal to `TransportEvent::ConnectionOpened`.
- Make `ClientTracker` store `IdentityKey`/principal per stream.
- Remote-control enrollment state should belong to a tenant runtime or include the `IdentityKey` in shared storage.
- Decide whether one remote-control connection can multiplex tenants. If yes, every remote-control envelope must include authenticated tenant routing metadata from the backend.

### Outbound Routing And Notifications

Findings:

- Outbound envelopes are either `ToConnection` or global `Broadcast`; there is no tenant-targeted broadcast (`outgoing_message.rs:84-94`).
- Server-request callbacks are keyed only by `RequestId`, not `(connection_id, request_id)` or tenant (`outgoing_message.rs:111-133`).
- Server requests can broadcast to all initialized connections (`outgoing_message.rs:278-338`).
- Server notifications with no explicit connection list broadcast to every initialized connection (`outgoing_message.rs:507-547`).
- The router fans out a broadcast to all initialized connections, subject only to notification opt-outs (`transport/mod.rs:352-390`).

Required changes:

- Replace `Broadcast` with `ToTenant { identity_key, message }` and, only where needed, `ToAllTenants` for administrative process notices.
- Add `IdentityKey` to outbound connection state and filter broadcasts by tenant.
- Key callbacks by `ServerRequestHandle { identity_key, connection_id, request_id }` or allocate globally unique unguessable IDs and verify the responding connection is an intended recipient.
- Make `process_response` and `process_error` take `connection_id` and enforce callback ownership.
- Audit every `send_server_notification(...)` call and make it tenant-scoped or thread-subscriber-scoped.

### Thread Manager And Thread State

Findings:

- `ThreadManagerState` has one `HashMap<ThreadId, Arc<CodexThread>>`, one auth manager, one models manager, one environment manager, one skills manager, one plugins manager, and one MCP manager (`core/src/thread_manager.rs:204-218`).
- `ThreadManager::new` builds managers from one `codex_home` and one restriction product (`core/src/thread_manager.rs:220-272`).
- `finalize_thread_spawn` inserts by bare `ThreadId` (`core/src/thread_manager.rs:956-985`).
- App-server thread subscription state is keyed by bare `ThreadId` and `ConnectionId`, with no tenant ownership check (`app-server/src/thread_state.rs:187-348`).
- `ThreadWatchManager` tracks one global running turn count and loaded statuses (`app-server/src/thread_status.rs:19-24`).

Required changes:

- Prefer one `ThreadManager` per `TenantRuntime`.
- If using a single global manager, all maps must key by `(IdentityKey, ThreadId)` and every manager method must require tenant.
- `ThreadStateManager`, `ThreadWatchManager`, pending unloads, pending server requests, and listener tasks must be per-tenant or tenant-keyed.
- `thread_created` broadcasts must include tenant or stay inside a tenant manager.
- Auto-attach on thread creation must attach only the creating connection and explicit same-tenant subscribers, not all initialized connections.
- Thread IDs can remain UUIDs but must be treated as tenant-scoped. Do not rely on UUID unguessability as authorization.

### Config, Auth, And Cloud Requirements

Findings:

- `Config` contains tenant-sensitive roots and settings: `cwd`, auth store mode, MCP servers and OAuth settings, agent roles, memories, `codex_home`, `sqlite_home`, log dir, and more (`core/src/config/mod.rs:360-430`).
- `ConfigApi` stores one `codex_home`, one CLI override set, one runtime feature map, one cloud-requirements loader, and reloads all loaded threads through one `UserConfigReloader` (`app-server/src/config_api.rs:73-155`, `app-server/src/config_api.rs:217-300`).
- `ExternalAgentConfigApi` owns one migration service rooted at one `codex_home` (`app-server/src/external_agent_config_api.rs:18-28`).
- `AuthManager` is explicitly a single source of truth for one `codex_home` auth snapshot (`login/src/auth/manager.rs:1151-1238`).
- File auth is `$CODEX_HOME/auth.json`, and keyring identity is derived from canonical `codex_home` (`login/src/auth/storage.rs:57-145`).
- `USER_AGENT_SUFFIX`, `ORIGINATOR`, and residency headers are process-global statics; comments explicitly say this assumes one MCP server per process (`login/src/auth/default_client.rs:18-98`).

Required changes:

- Move config/auth/cloud requirements into `TenantRuntime`.
- `ConfigApi` writes must mutate only that tenant and reload only that tenant's loaded threads.
- Runtime feature enablement must be per tenant, not process-wide.
- Auth login/logout must operate on tenant auth only and notify tenant connections only.
- External auth refresh must target the tenant's active connection(s), not broadcast globally.
- Replace process-global default client metadata with per-request/per-tenant HTTP client metadata or a request context passed through OpenAI/ChatGPT clients.
- Cloud requirements loader must be per tenant/account/workspace and refreshed on tenant auth changes only.

### Local Thread Store, Rollouts, And State DB

Findings:

- `LocalThreadStore` is built once from one `RolloutConfig` (`thread-store/src/local/mod.rs:18-42`).
- `RolloutConfig` is just `codex_home`, `sqlite_home`, `cwd`, model provider, and memory flag (`rollout/src/config.rs:6-34`).
- Thread listing walks/scans the one rollout root and/or state DB behind that config (`thread-store/src/local/list_threads.rs:15-64`).
- `StateRuntime` opens one state DB and one logs DB under one root (`state/src/runtime.rs:77-150`).
- Thread rows, indexes, and lookups are keyed by bare thread IDs and paths (`state/migrations/0001_threads.sql:1-24`, `state/src/runtime/threads.rs:367-535`).

Required changes:

- For per-tenant roots, create one `LocalThreadStore` per tenant.
- Ensure thread resume/read/archive/unarchive/set-name/metadata-update always resolve paths inside the tenant root unless an explicit tenant-authorized path capability is provided.
- Avoid accepting raw rollout `path` from clients in multi-tenant cloud mode, or validate it is inside the tenant's allowed storage/workspace.
- For shared DB, migrate every schema/query and legacy backfill path to include tenant.

### Filesystem, Watches, Fuzzy Search, And Command Exec

Findings:

- `FsApi::default()` uses the default host environment filesystem and all operations pass `sandbox None` (`app-server/src/fs_api.rs:29-90`).
- FS params use absolute paths and no thread/workspace/tenant handle (`app-server-protocol/src/protocol/v2.rs:2303-2325`).
- `FsWatchManager` is process-global and keyed by `(connection_id, watch_id)` (`app-server/src/fs_watch.rs:71-130`).
- `CommandExecManager` is process-global and sessions are keyed by `(connection_id, process_id)` (`app-server/src/command_exec.rs:47-125`).
- Fuzzy file search maps cancellation tokens and session IDs globally, while params provide arbitrary roots.

Required changes:

- In cloud mode, do not expose absolute host path operations. Require a tenant workspace id, thread id, or server-issued filesystem capability.
- Tenant FS operations should use that tenant's environment manager and sandbox policy.
- Watch IDs, fuzzy-search session IDs, and cancellation tokens must be tenant+connection scoped.
- Command sessions should be tenant+connection scoped and enforce tenant resource limits.
- Command execution must use tenant workspace/sandbox/network policy, not only client-provided params.

### Login And Account APIs

Findings:

- `active_login` is one mutex for the entire process (`codex_message_processor.rs:686-721`).
- API-key and ChatGPT login write to `self.config.codex_home` and reload one `AuthManager` (`codex_message_processor.rs:1181-1730`).
- Account update and login-completed notifications broadcast globally.

Required changes:

- `active_login` must be per tenant.
- Browser/device-code login callback state must include tenant and must not cancel another tenant's login.
- `account/read`, `account/login`, `account/logout`, rate limits, and external token refresh must use tenant auth.
- Account notifications must go only to the tenant.

### Thread APIs And Authorization

Findings:

- `load_thread` parses a bare thread id and calls `self.thread_manager.get_thread(thread_id)` (`codex_message_processor.rs:663-682`).
- `thread/start` derives config from one codex home, can persist project trust into that home, starts through one thread manager, auto-attaches the creating connection, then broadcasts `thread/started` globally (`codex_message_processor.rs:2249-2717`).
- `thread/list` reads the one local store and global statuses (`codex_message_processor.rs:3631-3713`).
- `thread/loaded/list` returns every loaded thread id from the one thread manager (`codex_message_processor.rs:3715-3772`).
- `thread/read`, `thread/resume`, `thread/fork`, `thread/setName`, `thread/metadata/update`, archive/unarchive use the one store/config and often locate rollouts under one `codex_home`.
- Turn APIs (`turn/start`, `turn/steer`, `turn/interrupt`, MCP resource/tool APIs) authorize only by ability to load the thread.

Required changes:

- Every thread method must resolve `TenantRuntime` first, then resolve `ThreadId` inside that tenant only.
- Add `ensure_thread_access(tenant, connection, thread_id, operation)` before read/write/turn/MCP/feedback operations.
- For `thread/start`, write trust/config only into tenant config.
- For `thread/list` and `thread/loaded/list`, return only tenant threads.
- For `thread/read includeTurns`, do not read arbitrary rollout paths outside tenant storage.
- For `thread/resume`/`thread/fork`, reject raw `path` unless it is inside tenant-owned storage or comes from a server-issued capability.
- For turn operations, subscription and outbound lifecycle events must be scoped to subscribed connections for that tenant/thread.

### MCP, Plugins, Skills, Apps

Findings:

- MCP OAuth login loads latest config from the one config root and uses one thread manager MCP manager (`codex_message_processor.rs:5305-5399`).
- MCP status reads process/tenant-wide config and auth (`codex_message_processor.rs:5415-5528`).
- Apps list uses one config, one auth manager, and global cached connector helpers (`codex_message_processor.rs:5838-6016`).
- Skills and plugin operations use one `codex_home`, one plugin manager, one skills manager, and one auth manager (`codex_message_processor.rs:6018-6740`).
- Marketplace add writes directly into `self.config.codex_home` (`codex_message_processor.rs:6363-6394`).
- Skills config write uses `ConfigEditsBuilder::new(&self.config.codex_home)` (`codex_message_processor.rs:6464-6518`).

Required changes:

- Plugins manager, skills manager, MCP manager, app connector caches, marketplace installs, and skill config writes must be per tenant.
- MCP OAuth credentials and callback state must include tenant. Fixed callback ports can conflict across concurrent tenants; prefer ephemeral callbacks or a broker keyed by tenant+state.
- Plugin install/uninstall/list should enforce tenant policy and use tenant auth for remote sync.
- App connector caches should include tenant/account/config in cache keys.

### Feedback, Logging, Analytics, And OTEL

Findings:

- `run_main_with_transport` creates one `CodexFeedback`, one tracing subscriber, one `StateRuntime`, and one log DB layer (`app-server/src/lib.rs:479-532`).
- Feedback upload may gather subtree thread IDs, SQLite logs, rollout attachments, and client-provided extra log files from the process tenant (`codex_message_processor.rs:8082-8265`).
- Analytics clients are built from one auth manager and one base URL (`message_processor.rs:263-270`).

Required changes:

- Logs and feedback snapshots must be tenant-tagged or tenant-isolated.
- Feedback upload with `include_logs` must only include tenant-owned threads and logs.
- `extra_log_files` should be removed in cloud mode or replaced by tenant-scoped attachment handles.
- Analytics events must include tenant/account/workspace where appropriate and avoid leaking tenant-specific request IDs across tenants.
- Process-level tracing subscriber can stay global, but per-tenant log DB sinks need either tenant-specific layers or a shared log schema with tenant columns.

## High-Risk Single-Tenant Assumptions To Remove First

1. Global outbound broadcasts.
2. Global thread-created auto-attach to every initialized connection.
3. One `AuthManager` and auth store for all connections.
4. One `ConfigApi` and runtime feature map.
5. Process-global default HTTP client headers (`ORIGINATOR`, `USER_AGENT_SUFFIX`, residency).
6. Thread/store APIs that resolve bare `thread_id` without tenant.
7. Absolute-path FS APIs and raw rollout path resume/fork.
8. One `active_login`.
9. One `ThreadManager` with shared plugin/skills/MCP/model managers.
10. Shared feedback/log snapshots without tenant labels.

## Suggested Implementation Plan

### Phase 0: Safety Switches

- Add a cloud/multi-tenant mode feature flag.
- In that mode, disable or restrict `fs/*`, raw `thread/resume.path`, raw `thread/fork.path`, `feedback.extraLogFiles`, and unauthenticated WebSocket listeners.
- Add logging fields for connection id, principal, redacted identity key digest, thread id, and request id.

### Phase 1: Tenant Context Plumbing

- Use `IdentityKey` as the tenant id and add `TenantPrincipal`, `TenantRuntime`, and a `TenantRegistry` keyed by `IdentityKey`.
- Seed the initial single-tenant runtime from the process startup `IdentityKey` where remote contracts are not yet multiplexed.
- Make transport auth produce principals that include an `IdentityKey`.
- Store principal/`IdentityKey` in `ConnectionSessionState` or adjacent connection metadata.
- Route every initialized request through a tenant runtime.
- Convert outbound broadcasts to tenant broadcasts.
- Update `process_response`/`process_error` to include connection id and validate callback ownership.

### Phase 2: Per-Tenant Runtime Isolation

- Move config/auth/cloud requirements/thread manager/config API/external config API/feedback/log handles into `TenantRuntime`.
- Create per-tenant `codex_home` and `sqlite_home`.
- Make `ThreadStateManager`, `ThreadWatchManager`, `CommandExecManager`, `FsWatchManager`, fuzzy search state, pending unloads, and active login per tenant or tenant-keyed.
- Ensure `thread_created` auto-attach only considers same-tenant connections.

### Phase 3: Surface Hardening

- Add thread ownership checks for every `thread_id` method.
- Replace remote filesystem APIs with tenant workspace APIs.
- Remove raw client paths for rollout/history operations in cloud mode.
- Tenant-scope plugin/MCP/app caches and OAuth callback state.
- Add per-tenant resource quotas.

### Phase 4: Optional Shared Persistence

Only do this if operational requirements demand one shared SQL store. Otherwise keep root isolation.

- Add `tenant_id` columns and composite keys to state/log schemas, with values derived from the canonical serialized `IdentityKey`.
- Migrate all query helpers and indexes.
- Backfill tenant ids for existing isolated roots.
- Update rollout/session index and archive paths.

## Test Matrix

Minimum multi-tenant regression tests:

- Two tenants connect to one app-server process; tenant A starts a thread; tenant B never receives `thread/started`, status, approvals, raw events, or token usage notifications.
- Tenant B cannot `thread/read`, `thread/resume`, `turn/start`, `turn/interrupt`, `mcp/*`, `feedback/upload`, archive, or rename tenant A's thread by guessed UUID.
- Tenant A and tenant B can use the same client request id without callback collisions.
- Tenant-specific config write reloads only tenant A threads.
- Tenant-specific login/logout sends account notifications only to tenant A.
- Tenant-specific `memory/reset` clears only tenant A memory data.
- FS/read/write/watch/fuzzy search cannot escape tenant workspace.
- Command exec process IDs collide safely across tenants/connections.
- Plugin install/uninstall and MCP OAuth in tenant A do not change tenant B config, caches, credentials, or server status.
- Remote-control streams for two tenants do not share client trackers, enrollments, cursors, outbound buffers, or auth recovery state.

## Bottom Line

A single deployable app-server process is feasible, but not as a narrow patch. The smallest defensible architecture is per-tenant runtime isolation inside the process, selected by authenticated `IdentityKey`, with tenant-scoped outbound routing and per-tenant `CODEX_HOME` roots. That avoids large SQL migrations while removing the dangerous cross-connection assumptions. After that is working, shared persistence can be considered as an explicit storage project rather than as a prerequisite for multi-tenancy.
