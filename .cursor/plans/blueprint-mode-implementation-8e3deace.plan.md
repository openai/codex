<!-- 8e3deace-3fe1-4c6f-9af9-a47d4e7d475a 0d06c4b1-a77d-4ad3-90d7-16e44d5831b0 -->
# Blueprint Mode + Execution Strategies Full Implementation

## Architecture Overview

### Rust Backend (`codex-rs/`)

- **`core/blueprint/`**: State machine, schema, persistence, policy enforcement
- **`core/agent/competition.rs`**: Worktree competition manager + scorer
- **`orchestrator/`**: Extended RPC methods for blueprint operations
- **`deep-research/`**: Blueprint integration adapters
- **`core/integrations/`**: Complete webhook implementations

### TypeScript Frontend (`extensions/vscode-codex/`)

- **`src/blueprint/`**: State management, UI components, commands
- **`src/views/`**: Blueprint panel, status badges, diff preview
- **`src/webhooks/`**: Configuration UI, test runners

---

## Phase 1: Core Blueprint Infrastructure (Rust)

### 1.1 Blueprint Schema & State Machine

**Files**: `codex-rs/core/src/blueprint/` (new directory)

- `schema.rs`: Blueprint block struct with all fields (id, title, goal, assumptions, mode, work_items, risks, eval, budget, etc.)
- `state.rs`: FSM (`Inactive → Drafting → Pending → Approved → Rejected → Superseded`)
- `persist.rs`: Writers for `.md` (docs/blueprints/) and `.json` (logs/blueprint/)
- `policy.rs`: Permission tiers (Safe/Privileged), approval gates
- `budget.rs`: Token/time enforcement, caps, overflow detection
- `mod.rs`: Public exports

**State Transitions**:

```rust
pub enum BlueprintState {
    Inactive,
    Drafting,
    Pending,
    Approved,
    Rejected { reason: String },
    Superseded { new_id: String },
}

pub struct BlueprintBlock {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub assumptions: Vec<String>,
    pub clarifying_questions: Vec<String>,
    pub approach: String,
    pub mode: ExecutionMode,
    pub work_items: Vec<WorkItem>,
    pub risks: Vec<Risk>,
    pub eval: EvalCriteria,
    pub budget: Budget,
    pub rollback: String,
    pub artifacts: Vec<String>,
    pub research: Option<ResearchBlock>,
    pub state: BlueprintState,
    pub need_approval: bool,
}
```

**Side-Effect Lockdown**: All file writes, network calls, package installs BLOCKED until state == `Approved`.

### 1.2 Blueprint Manager

**File**: `codex-rs/core/src/blueprint/manager.rs`

- `create_blueprint()`: Generate from user goal
- `update_blueprint()`: Modify existing (mints new ID if scope changes)
- `approve_blueprint()`: State transition + unlock execution
- `reject_blueprint()`: State transition + logging
- `export_blueprint()`: MD/JSON export
- `get_blueprint()`: Retrieval by ID

### 1.3 Orchestrator RPC Extensions

**File**: `codex-rs/orchestrator/src/rpc/blueprint_methods.rs`

New RPC methods (16 total → 24 total):

```rust
blueprint_create(goal: String, mode: String, budget: Budget) -> BlueprintId
blueprint_get(id: String) -> BlueprintBlock
blueprint_update(id: String, changes: Partial<BlueprintBlock>) -> BlueprintBlock
blueprint_approve(id: String, approver: String) -> Result<()>
blueprint_reject(id: String, reason: String) -> Result<()>
blueprint_export(id: String, format: String, path: Option<String>) -> String
blueprint_set_mode(mode: ExecutionMode) -> Result<()>
blueprint_add_research(id: String, research: ResearchBlock) -> Result<()>
```

Update `codex-rs/orchestrator/src/rpc.rs` with new method routing.

---

## Phase 2: Execution Strategies (Rust)

### 2.1 Orchestrated Control (Enhancement)

**File**: `codex-rs/core/src/orchestration/orchestrated_control.rs` (refactor from `auto_orchestrator.rs`)

Enhancements:

- Accept `BlueprintBlock` as input
- Generate compact briefs per sub-agent (files, constraints, tests)
- Collect **patch-first diffs** (not full rewrites)
- Run linters/tests before PR
- Emit telemetry events
- Trigger webhooks on completion
```rust
pub async fn execute_orchestrated(
    blueprint: &BlueprintBlock,
    runtime: Arc<AgentRuntime>,
) -> Result<OrchestratedResult> {
    // 1. Build task DAG from blueprint.work_items
    // 2. Spawn sub-agents (Backend/Frontend/DB/Sec/QA)
    // 3. Integrator collects patches
    // 4. Run tests/linters
    // 5. Prepare PR
    // 6. Trigger webhooks
}
```


### 2.2 Worktree Competition (New)

**File**: `codex-rs/core/src/agent/competition.rs` (new)

Components:

- `WorktreeManager`: Create/delete git worktrees
- `CompetitionRunner`: Execute N variants in parallel
- `Scorer`: Compute score = `f(tests, perf, size, simplicity)`
- `ComparisonTable`: Present results to user
- `Merger`: Auto-merge winner, archive losers
```rust
pub struct CompetitionConfig {
    pub num_variants: usize, // 2-3
    pub weights: ScoreWeights,
    pub time_budget_min: u64,
}

pub struct ScoreWeights {
    pub tests: f64,      // 0.5
    pub performance: f64, // 0.3
    pub simplicity: f64,  // 0.2
}

pub async fn run_competition(
    blueprint: &BlueprintBlock,
    config: CompetitionConfig,
) -> Result<CompetitionResult> {
    // 1. Create N worktrees (branches A/B/C)
    // 2. Execute identical task in each
    // 3. Run tests/benchmarks/linters
    // 4. Compute scores
    // 5. Present comparison table
    // 6. Merge winner, archive others
    // 7. Trigger webhooks with comparison data
}
```


**Git Operations**: Use `codex-rs/utils/git/` for worktree management.

### 2.3 Mode Switching

**File**: `codex-rs/core/src/blueprint/execution_mode.rs`

```rust
pub enum ExecutionMode {
    Single,        // No sub-agents
    Orchestrated,  // Central planner + sub-agents
    Competition,   // Worktree variants
}

pub struct ExecutionEngine {
    pub fn set_mode(&mut self, mode: ExecutionMode);
    pub async fn execute(&self, blueprint: &BlueprintBlock) -> Result<ExecutionResult>;
}
```

Runtime API: `runtime.setMode('orchestrated'|'competition'|'single')`.

---

## Phase 3: DeepResearch Integration (Rust)

### 3.1 Research Adapters

**Files**: `codex-rs/deep-research/src/blueprint_adapter.rs`

```rust
pub struct ResearchBlock {
    pub query: String,
    pub depth: u8,
    pub strategy: ResearchStrategy,
    pub sources: Vec<ResearchSource>,
    pub synthesis: String,
    pub confidence: f64,
    pub needs_approval: bool,
}

pub struct ResearchSource {
    pub title: String,
    pub url: String,
    pub date: String,
    pub key_finding: String,
    pub confidence: f64,
}
```

**Approval Dialog**: Must show domains, depth, budget caps, data retention policy.

**Cross-Source Agreement**: Only ingest claims with ≥2 credible sources; flag disagreements.

### 3.2 Integration with Blueprint

**File**: `codex-rs/core/src/blueprint/research_integration.rs`

- Blueprint enters `Researching` sub-state
- Approval dialog triggered
- DeepResearch executed (read-only)
- Results appended to `blueprint.research`
- Sources cited in output

---

## Phase 4: Webhooks (Rust)

### 4.1 Webhook Implementations

**Files**: `codex-rs/core/src/integrations/`

- `github.rs`: Commit status, PR summary, winning branch message
- `slack.rs`: Thread updates for state transitions
- `http.rs`: Generic POST with HMAC signature, retry logic
```rust
pub struct WebhookPayload {
    pub bp_id: String,
    pub state: BlueprintState,
    pub summary: String,
    pub score: Option<CompetitionScore>,
    pub timestamp: String,
}

pub async fn send_webhook(
    service: WebhookService,
    payload: WebhookPayload,
    secret: String,
) -> Result<()> {
    // HMAC-SHA256 signature
    // Exponential backoff retry (3 attempts)
    // Log response
}
```


**Secret Management**: OS keychain integration via `codex-rs/keyring-store/`.

### 4.2 Policy Gating

**File**: `codex-rs/core/src/blueprint/policy.rs`

```rust
pub struct WebhookPolicy {
    pub enabled: bool,
    pub require_approval: bool,
    pub allowed_domains: Vec<String>,
}
```

All webhooks **opt-in** and gated by policy.

---

## Phase 5: Telemetry (Rust)

### 5.1 Event Collection

**File**: `codex-rs/core/src/blueprint/telemetry.rs`

Events:

- `bp.start`, `bp.generate`, `bp.approve`, `bp.reject`, `bp.export`
- `exec.start`, `exec.result`
- `research.start`, `research.complete`

Metrics:

- `time_to_pending`, `time_to_approval`, `num_revisions`
- `research_calls`, `webhook_calls`

**Privacy**: User IDs hashed, URLs domain-only (optional).

### 5.2 Storage

JSON logs: `logs/telemetry/*.jsonl`

Optional: Export to OpenTelemetry compatible backends.

---

## Phase 6: TypeScript UI (VS Code Extension)

### 6.1 Blueprint UI Components

**Directory**: `extensions/vscode-codex/src/blueprint/`

**Files**:

- `state.ts`: Local state management (sync with Rust backend)
- `panel.tsx`: Main Blueprint editor panel (WebView)
- `statusBadge.ts`: Status indicator (pending/approved/rejected/superseded)
- `diffPreview.ts`: Locked diff preview until approved
- `commands.ts`: Slash command handlers

**Panel Sections** (WebView React components):

- Title/Goal input
- Assumptions list
- Clarifying Questions list
- Approach textarea
- Mode selector (single/orchestrated/competition)
- Work Items table (name, files, tests)
- Risks table (item, mitigation)
- Eval criteria (tests, metrics)
- Budget fields (tokens, time)
- Rollback plan
- Research block (query, sources, synthesis)

### 6.2 Slash Commands

**File**: `extensions/vscode-codex/src/blueprint/commands.ts`

Implement:

```typescript
/blueprint on|off
/blueprint "title or goal..." --mode=... --budget.tokens=... --budget.time=...
/approve <bp-id>
/reject <bp-id> --reason=...
/blueprint export <bp-id> --format=md|json --path=...
/mode single|orchestrated|competition
/deepresearch "query" --depth=1..3 --policy=...
```

**Compatibility Alias**: `/plan` → `/blueprint` (config flag `compat.planAlias`).

### 6.3 GUI Controls

**File**: `extensions/vscode-codex/src/blueprint/toolbar.ts`

Header buttons:

- **Enter Blueprint** (toggle)
- **Approve**
- **Reject**
- **Export**
- **Mode** selector (dropdown)

Status badge colors:

- Pending: Amber
- Approved: Green
- Rejected: Red
- Superseded: Gray

### 6.4 Approval Dialog

**File**: `extensions/vscode-codex/src/blueprint/approvalDialog.ts`

For DeepResearch and privileged operations:

- Show domains, depth, budget caps
- Data retention policy
- Approve/Reject buttons

### 6.5 Diff Preview

**File**: `extensions/vscode-codex/src/blueprint/diffPreview.ts`

- Locked until `state == Approved`
- Show "Approve blueprint to preview changes" message
- After approval: side-by-side diff view

---

## Phase 7: Configuration & Settings

### 7.1 VS Code Settings

**File**: `extensions/vscode-codex/package.json`

```json
{
  "codex.blueprint.enabled": { "type": "boolean", "default": true },
  "codex.blueprint.mode": { "enum": ["single", "orchestrated", "competition"], "default": "orchestrated" },
  "codex.blueprint.autoApprove": { "type": "boolean", "default": false },
  "codex.blueprint.exportPath": { "type": "string", "default": "docs/blueprints" },
  "codex.competition.numVariants": { "type": "number", "default": 2 },
  "codex.competition.weights": { "type": "object" },
  "codex.research.requireApproval": { "type": "boolean", "default": true },
  "codex.webhooks.enabled": { "type": "boolean", "default": false },
  "codex.webhooks.github.enabled": { "type": "boolean", "default": false },
  "codex.webhooks.slack.enabled": { "type": "boolean", "default": false },
  "codex.telemetry.enabled": { "type": "boolean", "default": true },
  "codex.compat.planAlias": { "type": "boolean", "default": true }
}
```

### 7.2 Keybindings

**File**: `extensions/vscode-codex/package.json`

```json
{
  "key": "shift+tab",
  "command": "codex.blueprint.toggle",
  "when": "editorTextFocus"
}
```

Optional: `ui.keymap.blueprintToggle` setting.

---

## Phase 8: Testing

### 8.1 Rust Unit Tests

**Files**: `codex-rs/core/src/blueprint/tests/`

- `schema_tests.rs`: Blueprint block serialization/deserialization
- `state_machine_tests.rs`: State transitions
- `policy_tests.rs`: Permission gates, approval enforcement
- `competition_tests.rs`: Worktree operations, scoring
- `research_integration_tests.rs`: Approval dialogs, cross-source agreement

### 8.2 Rust Integration Tests

**Files**: `codex-rs/core/tests/blueprint_integration_tests.rs`

- End-to-end blueprint lifecycle (create → approve → execute)
- Mode switching (orchestrated ↔ competition)
- Webhook delivery (mock endpoints)
- Telemetry collection

### 8.3 TypeScript Tests

**Files**: `extensions/vscode-codex/src/blueprint/__tests__/`

- `commands.test.ts`: Slash command parsing
- `state.test.ts`: State sync with backend
- `panel.test.ts`: UI component rendering
- `approval.test.ts`: Approval dialog flow

### 8.4 E2E Tests

**File**: `extensions/vscode-codex/src/test/e2e/blueprint.test.ts`

- GUI/CLI parity: Same blueprint via both interfaces
- Approval flow: Click approve → state changes → execution unlocked
- Export: Generate MD/JSON files
- Mode switch: Orchestrated → Competition

---

## Phase 9: Documentation

### 9.1 User Docs

**Files**: `docs/blueprint/`

- `README.md`: Overview, quick start, examples
- `slash-commands.md`: Complete command reference
- `execution-modes.md`: Orchestrated vs Competition
- `research-integration.md`: DeepResearch usage
- `webhooks.md`: GitHub/Slack/HTTP setup

### 9.2 Developer Docs

**Files**: `docs/blueprint/dev/`

- `architecture.md`: Component diagram, data flow
- `state-machine.md`: FSM transitions, invariants
- `competition-scoring.md`: Scoring algorithm, weights tuning
- `telemetry.md`: Event schema, privacy policy

### 9.3 Samples

**Files**: `docs/blueprints/samples/`

- `simple-feature.md`: Single-file change
- `orchestrated-refactor.md`: Multi-agent coordination
- `competition-optimization.md`: Performance competition

---

## Phase 10: Rollout & Migration

### 10.1 Feature Flag

**File**: `codex-rs/core/src/config/types.rs`

```rust
pub struct FeaturesConfig {
    pub blueprint_mode: bool,  // default: false → true
}
```

### 10.2 Migration Script

**File**: `scripts/migrate_plans_to_blueprints.py`

- Move `docs/plans/*.md` → `docs/blueprints/*.md`
- Convert legacy format to Blueprint Block schema
- Update references in logs

### 10.3 Release Notes

**File**: `CHANGELOG.md`

```markdown
## [0.57.0] - 2025-11-XX

### Added
- **Blueprint Mode**: Read-only planning phase with approval gates
- **Worktree Competition**: Auto-score variants, merge winner
- **DeepResearch Integration**: Approval dialogs, source citations
- **Webhooks**: GitHub, Slack, HTTP with HMAC signatures
- **Telemetry**: Privacy-respecting event collection

### Changed
- Orchestrated Control: Now operates on approved Blueprints
- `/plan` command aliased to `/blueprint` (compatibility window)

### Deprecated
- `/plan` command (use `/blueprint` instead)
```

### 10.4 Rollout Phases

**Phase 1 (Dogfood)**: `features.blueprintMode=true` for devs; alias `/plan` enabled.

**Phase 2 (Beta)**: GUI enabled; collect telemetry; tune scoring weights.

**Phase 3 (GA)**: Alias off by default; migration script provided.

---

## Acceptance Criteria

1. ✅ `/blueprint on` and **Enter Blueprint** button trigger identical state transitions
2. ✅ `pending` state accepts **Approve/Reject** from CLI and GUI
3. ✅ No side effects (file writes, network, installs) while not `approved`
4. ✅ Export produces **MD/JSON** in configured directories
5. ✅ Mode switching affects execution engine (orchestrated/competition/single)
6. ✅ DeepResearch prompts approval dialog, logs sources + synthesis
7. ✅ Worktree Competition auto-scores, merges winner, archives losers
8. ✅ Orchestrated Control integrates deterministic diffs, runs tests
9. ✅ Webhooks deliver to GitHub/Slack/HTTP with retries
10. ✅ Telemetry collects events without PII
11. ✅ GUI/CLI parity: Same operations via both interfaces
12. ✅ Upstream compatibility: Public APIs unchanged; new features additive

---

## Risks & Mitigations

| Risk | Mitigation |

|------|-----------|

| Scope creep in Blueprint | Strict schema + size caps; enforce clarifying question limits |

| Noisy research | Depth ≤3, domain allow-list, cross-source agreement rule |

| Branch thrash in competition | Limit to 2-3 variants; auto-archive losers; time budget |

| Webhook secrets leakage | Keychain integration; redact logs; signature verification |

| State drift (Rust ↔ TypeScript) | Single source of truth (Rust); TypeScript polls RPC |

| Performance overhead | Lazy-load WebView; cache RPC responses; debounce UI updates |

---

## Implementation Order

1. **Rust Core** (Phases 1-5): Blueprint schema, state machine, execution strategies, research, webhooks, telemetry
2. **TypeScript UI** (Phase 6): VS Code extension, commands, panels, approval dialogs
3. **Configuration** (Phase 7): Settings, keybindings
4. **Testing** (Phase 8): Unit, integration, e2e tests
5. **Documentation** (Phase 9): User docs, developer docs, samples
6. **Rollout** (Phase 10): Feature flag, migration script, release notes

---

## Estimated Effort

- **Rust Backend**: 120-150 hours
- **TypeScript Frontend**: 60-80 hours
- **Testing**: 40-50 hours
- **Documentation**: 20-30 hours
- **Total**: 240-310 hours (6-8 weeks, 1-2 engineers)

---

## Success Metrics

- Blueprint approval rate: >80%
- Competition winner correctness: >90%
- Webhook delivery success: >99%
- User satisfaction (survey): >4.5/5
- Telemetry opt-out rate: <10%

### To-dos

- [ ] Implement Blueprint schema, state machine, persistence in codex-rs/core/src/blueprint/
- [ ] Create Blueprint manager (create/update/approve/reject/export) in Rust
- [ ] Add 8 new RPC methods for blueprint operations to orchestrator
- [ ] Enhance AutoOrchestrator to accept BlueprintBlock, emit telemetry, trigger webhooks
- [ ] Implement Worktree Competition (manager, scorer, merger) in codex-rs/core/src/agent/competition.rs
- [ ] Create ExecutionMode enum and ExecutionEngine with mode switching
- [ ] Integrate DeepResearch with Blueprint (approval dialog, ResearchBlock)
- [ ] Implement GitHub/Slack/HTTP webhooks with HMAC, retry, keychain integration
- [ ] Implement telemetry event collection (privacy-respecting) in Rust
- [ ] Create TypeScript Blueprint UI components (panel, statusBadge, diffPreview)
- [ ] Implement slash commands (/blueprint, /approve, /reject, /mode, /deepresearch)
- [ ] Create GUI toolbar with Enter Blueprint, Approve, Reject, Export, Mode buttons
- [ ] Implement approval dialog for DeepResearch and privileged operations
- [ ] Add VS Code settings and keybindings to package.json
- [ ] Write Rust unit tests (schema, state machine, policy, competition, research)
- [ ] Write Rust integration tests (lifecycle, mode switching, webhooks, telemetry)
- [ ] Write TypeScript tests (commands, state, panel, approval)
- [ ] Write e2e tests (GUI/CLI parity, approval flow, export, mode switch)
- [ ] Write user docs (README, commands, modes, research, webhooks)
- [ ] Write developer docs (architecture, state machine, scoring, telemetry)
- [ ] Create sample blueprints (simple, orchestrated, competition)
- [ ] Add feature flag for blueprint mode in config
- [ ] Create migration script to move docs/plans/ to docs/blueprints/
- [ ] Update CHANGELOG.md with v0.57.0 release notes