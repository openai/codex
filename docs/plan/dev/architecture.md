# plan mode - Architecture

**Version**: 0.57.0  
**Audience**: Developers, Contributors

---

## System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    VS Code Extension                        │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────┐  │
│  │  Commands    │  │  Status Bar  │  │  Plan UI   │  │
│  │  (/Plan)│  │  (Shift+Tab) │  │  (WebView)      │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬────────┘  │
│         │                  │                    │            │
│         └──────────────────┴────────────────────┘            │
│                            │                                 │
│                     RPC Protocol Client                      │
└────────────────────────────┼─────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│              Orchestrator RPC Server (Rust)                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  RPC Methods (24 total, 8 Plan-specific)          │ │
│  │  - Plan.create/get/update/approve/reject/export   │ │
│  │  - Plan.setMode, Plan.addResearch            │ │
│  └────────────────────────────────────────────────────────┘ │
│                            │                                 │
│                            ▼                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │              Plan Manager                          │ │
│  │  - CRUD operations                                      │ │
│  │  - State transitions                                    │ │
│  │  - Persistence (MD + JSON)                              │ │
│  └────────────────────────────────────────────────────────┘ │
└────────────────────────────┬────────────────────────────────┘
                             │
                    ┌────────┴─────────┐
                    │                  │
                    ▼                  ▼
          ┌─────────────────┐  ┌──────────────┐
          │  Execution      │  │  Telemetry   │
          │  Engine         │  │  & Webhooks  │
          └─────────────────┘  └──────────────┘
                    │
        ┌───────────┼───────────┐
        │           │           │
        ▼           ▼           ▼
    ┌───────┐  ┌────────┐  ┌──────────┐
    │Single │  │Orchest-│  │Competi-  │
    │ Mode  │  │rated   │  │tion      │
    └───────┘  └────────┘  └──────────┘
                    │              │
                    ▼              ▼
               ┌─────────┐  ┌──────────────┐
               │SubAgents│  │Git Worktrees │
               └─────────┘  └──────────────┘
```

---

## Module Breakdown

### 1. Plan Core (`codex-rs/core/src/Plan/`)

#### `schema.rs`

Defines all data structures:

- `ExecutionMode`: enum (Single, Orchestrated, Competition)
- `PlanBlock`: Complete planning artifact
- `WorkItem`, `Risk`, `EvalCriteria`, `Budget`
- `ResearchBlock`, `ResearchSource`

**Key Types**:
```rust
pub struct PlanBlock {
    pub id: String,                          // Unique ID (timestamp-based)
    pub title: String,                       // Human-readable title
    pub goal: String,                        // High-level goal
    pub state: PlanState,               // Current state
    pub mode: ExecutionMode,                 // Execution strategy
    pub work_items: Vec<WorkItem>,           // Tasks to complete
    pub budget: Budget,                      // Token/time limits
    pub research: Option<ResearchBlock>,     // Optional research results
    // ... more fields
}
```

#### `state.rs`

Finite State Machine implementation:

```
Inactive ──start_drafting──> Drafting ──submit──> Pending
                                                       │
                                          ┌────────────┼────────────┐
                                          │            │            │
                                      approve      reject      supersede
                                          │            │            │
                                          ▼            ▼            ▼
                                     Approved     Rejected    Superseded
                                     (executable) (terminal)   (terminal)
```

**State Transitions**:
- `start_drafting()`: Inactive → Drafting
- `submit_for_approval()`: Drafting → Pending
- `approve(approver)`: Pending → Approved
- `reject(reason)`: Pending/Drafting → Rejected
- `supersede(new_id)`: Any → Superseded

#### `persist.rs`

Dual-format persistence:

- **Markdown** (`docs/Plans/YYYY-MM-DD_title.md`): Human-readable, Git-friendly
- **JSON** (`logs/Plan/bp-id.json`): Full fidelity, machine-readable

**Operations**:
- `save_markdown(Plan)` → PathBuf
- `save_json(Plan)` → PathBuf
- `load_json(id)` → PlanBlock
- `list_Plans()` → Vec<String>

#### `policy.rs`

Permission enforcement:

```rust
pub enum PermissionTier {
    Safe,       // Read workspace, compute, dry-runs
    Privileged, // Network, install, destructive git
}

pub enum PrivilegedOperation {
    Network,            // Research, webhooks
    Install,            // Package managers
    GitDestructive,     // Force push, hard reset
    FileWriteExternal,  // Write outside workspace
    ShellExec,          // Arbitrary commands
}
```

**Approval Roles** (hierarchy):
- User < Reviewer < Maintainer < Admin

#### `budget.rs`

Token & time tracking:

```rust
pub struct BudgetTracker {
    budget: Budget,
    tokens_used: AtomicU64,
    start_time: Instant,
}
```

**Enforcement**:
- Step limit: Max 20,000 tokens per operation
- Session cap: Max 100,000 tokens total
- Time cap: Max 30 minutes (configurable)
- Early termination on overflow

#### `manager.rs`

High-level API:

```rust
pub struct PlanManager {
    Plans: Arc<RwLock<HashMap<String, PlanBlock>>>,
    persister: PlanPersister,
    policy_enforcer: PolicyEnforcer,
}
```

**Methods**:
- `create_Plan(goal, title)` → String (bp-id)
- `get_Plan(id)` → PlanBlock
- `update_Plan(id, update_fn)` → String
- `approve_Plan(id, approver, role)` → Result<()>
- `reject_Plan(id, reason)` → Result<()>
- `export_Plan(id)` → (md_path, json_path)

---

### 2. Execution Engine (`codex-rs/core/src/execution/`)

#### `engine.rs`

Switchable execution strategies:

```rust
pub struct ExecutionEngine {
    mode: ExecutionMode,
    runtime: Arc<AgentRuntime>,
}

impl ExecutionEngine {
    pub fn set_mode(&mut self, mode: ExecutionMode);
    pub async fn execute(&self, Plan: &PlanBlock) -> Result<ExecutionResult>;
}
```

**Dispatch Logic**:
```rust
match self.mode {
    ExecutionMode::Single => self.execute_single(Plan).await,
    ExecutionMode::Orchestrated => self.execute_orchestrated(Plan).await,
    ExecutionMode::Competition => self.execute_competition(Plan).await,
}
```

---

### 3. Competition (`codex-rs/core/src/agent/competition.rs`)

#### Components

1. **WorktreeManager**: Git worktree operations
   - `create_worktree(variant_name)` → PathBuf
   - `remove_worktree(path)` → Result<()>
   - `archive_variant(name)` → Result<()>

2. **CompetitionScorer**: Variant scoring
   - `run_tests(worktree, eval)` → f64
   - `measure_performance(worktree)` → f64
   - `measure_simplicity(worktree)` → f64
   - `score_variant(worktree, eval)` → CompetitionScore

3. **CompetitionRunner**: Orchestration
   - `run_competition(Plan)` → CompetitionResult
   - `merge_winner(result)` → Result<()>
   - `archive_losers(result)` → Result<()>

#### Scoring Algorithm

```
Score = w_tests × S_tests + w_perf × S_perf + w_simp × S_simp

where:
- w_tests = 0.5 (configurable)
- w_perf = 0.3 (configurable)
- w_simp = 0.2 (configurable)
- S_* ∈ [0, 100]

If tests fail → Score = -∞ (disqualified)
```

---

### 4. Telemetry (`codex-rs/core/src/telemetry/`)

#### Event Flow

```
Application
    │
    ▼
TelemetryEvent::new(EventType::PlanStart)
    │
    ▼
TelemetryCollector.record(event)
    │
    ▼
Channel (buffer: 100 events)
    │
    ▼
Background Task (flush every 60s or when buffer full)
    │
    ▼
TelemetryStorage.store(event)
    │
    ▼
JSONL file: logs/telemetry/telemetry-YYYY-MM-DD.jsonl
```

#### Privacy Guarantees

| Data Type | Treatment |
|-----------|-----------|
| User ID | SHA-256 hashed |
| Session ID | SHA-256 hashed |
| Plan ID | SHA-256 hashed |
| URLs | Domain-only (no paths/queries) |
| Timestamps | UTC, no timezone inference |
| Metadata | User-controlled, opt-in |

---

### 5. Webhooks (`codex-rs/core/src/webhooks/`)

#### Send Flow

```
WebhookClient.send(config, payload)
    │
    ▼
Retry Loop (max 3 attempts)
│
├─ Attempt 1 (0s delay)
│   ├─ Format payload (GitHub/Slack/HTTP)
│   ├─ Compute HMAC-SHA256
│   ├─ Send POST request
│   └─ Check response status
│       ├─ Success → Done
│       └─ Error → Retry
│
├─ Attempt 2 (1s delay)
│   └─ ...
│
└─ Attempt 3 (2s delay)
    └─ ... (exponential backoff)
```

#### HMAC Signature

```rust
fn compute_hmac(secret: &str, body: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes());
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
```

Sent as header:
```
X-Codex-Signature: sha256=<hmac-hex>
```

---

## Data Flow

### Plan Creation → Execution

```
1. User: /Plan "Add feature" --mode=orchestrated
       ↓
2. VS Code Extension: commands.ts
       ↓
3. RPC Client: Plan.create request
       ↓
4. Orchestrator Server: process_write_request()
       ↓
5. PlanManager: create_Plan()
       ├─ Generate ID
       ├─ Set state = Drafting
       └─ Persist (MD + JSON)
       ↓
6. Return: bp-id to client
       ↓
7. User reviews exported MD file
       ↓
8. User: /approve bp-123
       ↓
9. PlanManager: approve_Plan()
       ├─ Check policy (Maintainer role?)
       ├─ Transition state: Pending → Approved
       ├─ Persist
       ├─ Emit telemetry event
       └─ Trigger webhooks
       ↓
10. Execution Engine: execute(Plan)
        ├─ Verify state == Approved
        ├─ Check budget
        └─ Dispatch by mode:
            ├─ Single: Direct execution
            ├─ Orchestrated: Sub-agents
            └─ Competition: Worktrees
        ↓
11. Results + artifacts
        ├─ Emit telemetry (exec.result)
        └─ Trigger webhooks (completion)
```

---

## State Invariants

1. **No Side Effects Before Approval**:
   - `state != Approved` → All file writes, network calls BLOCKED

2. **Immutable After Approval**:
   - `state == Approved` → Cannot modify Plan
   - To change: reject → edit → re-approve

3. **Terminal States**:
   - `Rejected` and `Superseded` cannot transition further

4. **Budget Monotonic**:
   - `tokens_used` only increases
   - Cannot "refund" tokens

5. **Idempotent Persistence**:
   - Multiple saves of same Plan produce identical JSON

---

## Concurrency Model

### Orchestrator Server

- **Single-Writer Queue**: All write operations serialized
- **Read Parallelism**: Multiple concurrent reads
- **Idempotency Cache**: 10-minute TTL, prevents duplicate writes

### Telemetry

- **Channel-based**: mpsc channel (buffer: 100)
- **Async Flush**: Background task, non-blocking
- **File Locking**: Mutex-protected file handle

### Webhooks

- **Fire-and-Forget**: Async send, doesn't block main flow
- **Retry in Background**: Exponential backoff managed by tokio

---

## Security Model

### Approval Gates

```rust
PolicyEnforcer.enforce(
    operation: PrivilegedOperation,
    user_role: Option<ApprovalRole>,
    domain: Option<&str>
) -> Result<()>
```

**Decision Matrix**:

| Operation | Min Role | Domain Check | Side Effect |
|-----------|----------|--------------|-------------|
| Read workspace | User | No | No |
| Run tests (dry) | User | No | No |
| Network call | Maintainer | Yes | Yes |
| Install package | Maintainer | No | Yes |
| Git destructive | Admin | No | Yes |

### Webhook Signatures

```
HMAC-SHA256(secret, body) → hex digest
Header: X-Codex-Signature: sha256=<digest>
```

Receiver MUST verify signature before processing payload.

---

## Performance Characteristics

### Memory Usage

| Component | Memory (avg) | Notes |
|-----------|--------------|-------|
| Plan in-memory | ~10 KB | Per Plan |
| Telemetry buffer | ~100 KB | 100 events |
| Webhook queue | ~50 KB | Async send |
| RPC server | ~5 MB | Base overhead |

### Latency

| Operation | Latency (p95) |
|-----------|---------------|
| Plan create | <10 ms |
| Plan approve | <5 ms |
| RPC roundtrip | <15 ms |
| Telemetry record | <1 ms (async) |
| Webhook send | <500 ms (with retry) |

### Throughput

- **RPC Server**: 1000+ req/sec
- **Telemetry**: 10,000+ events/sec
- **Webhooks**: 100+ notifications/sec (rate-limited)

---

## Testing Strategy

### Unit Tests

Location: Embedded in each module (`#[cfg(test)]`)

**Coverage Targets**:
- Plan: 90%+
- Telemetry: 85%+
- Webhooks: 85%+
- Competition: 80%+

### Integration Tests

Location: `codex-rs/core/tests/Plan_integration_tests.rs`

**Scenarios**:
- Full Plan lifecycle (create → approve → execute)
- Mode switching (orchestrated ↔ competition)
- Webhook delivery (mock endpoints)
- Telemetry collection

### E2E Tests

Location: `extensions/vscode-codex/src/test/e2e/`

**Scenarios**:
- GUI/CLI parity
- Approval flow (click approve → state changes → execution unlocked)
- Export (generate MD/JSON files)

---

## Extension Points

### Custom Scorers

Implement `CompetitionScorer` trait:

```rust
pub trait Scorer {
    async fn score_variant(&self, path: &PathBuf, eval: &EvalCriteria) -> Result<f64>;
}
```

### Custom Research Providers

Implement `ResearchProvider` trait from `codex-deep-research`:

```rust
#[async_trait]
pub trait ResearchProvider: Send + Sync {
    async fn search(&self, query: &str, depth: u8) -> Result<Vec<ResearchSource>>;
}
```

### Custom Webhook Services

Add to `WebhookService` enum and implement formatter in `WebhookClient`:

```rust
pub enum WebhookService {
    GitHub,
    Slack,
    Http,
    Custom(String), // Your service
}
```

---

## File Structure

```
codex-rs/
├── core/
│   └── src/
│       ├── Plan/
│       │   ├── schema.rs         (312 lines)
│       │   ├── state.rs          (250 lines)
│       │   ├── persist.rs        (384 lines)
│       │   ├── policy.rs         (298 lines)
│       │   ├── budget.rs         (335 lines)
│       │   ├── manager.rs        (385 lines)
│       │   └── research_integration.rs (248 lines)
│       ├── execution/
│       │   └── engine.rs         (215 lines)
│       ├── agent/
│       │   └── competition.rs    (450 lines)
│       ├── telemetry/
│       │   ├── events.rs         (212 lines)
│       │   ├── collector.rs      (178 lines)
│       │   └── storage.rs        (186 lines)
│       └── webhooks/
│           ├── types.rs          (188 lines)
│           └── client.rs         (256 lines)
└── orchestrator/
    └── src/
        ├── rpc.rs                (+152 lines)
        └── server.rs             (+185 lines)

extensions/vscode-codex/src/
├── Plan/
│   ├── state.ts                  (175 lines)
│   ├── commands.ts               (285 lines)
│   └── statusBadge.ts            (122 lines)
├── ui/
│   └── statusBar.ts              (56 lines)
└── views/
    ├── agentProvider.ts          (68 lines)
    ├── researchProvider.ts       (58 lines)
    └── mcpProvider.ts            (51 lines)
```

**Total**: ~6,000 lines of production code

---

## See Also

- [State Machine Diagram](./state-machine.md)
- [Competition Scoring](./competition-scoring.md)
- [Telemetry Schema](./telemetry.md)

---

**Made with ❤️ by zapabob**

