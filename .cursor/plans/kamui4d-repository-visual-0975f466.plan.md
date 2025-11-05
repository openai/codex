<!-- 0975f466-b611-418a-a4c9-db2ffdd2f7b4 96615661-d482-44d6-a029-f658df89ebec -->
# AI-Native Prism Complete Migration Plan

## 新リポジトリ構成

```
prism/
├── apps/
│   ├── web/                    # Next.js 15 App Router
│   │   ├── app/
│   │   │   ├── (dashboard)/    # メインUI
│   │   │   ├── orchestrate/    # AI並列実行
│   │   │   ├── blueprint/      # Blueprint Editor
│   │   │   ├── visualize/      # 3D Git可視化
│   │   │   ├── api/            # API Routes
│   │   │   │   ├── orchestrate/
│   │   │   │   ├── blueprint/
│   │   │   │   ├── webhook/
│   │   │   │   ├── git/
│   │   │   │   └── mcp/
│   │   ├── components/
│   │   ├── lib/
│   │   └── package.json
│   │
│   ├── desktop/                # Electron App (既存)
│   └── cli/                    # Prism CLI (新規)
│
├── packages/
│   ├── git-core/               # Git Worktree + Orchestrated Edit
│   ├── supervisor/             # AI Orchestration Engine
│   ├── consensus/              # Voting & Scoring
│   ├── mcp-clients/            # MCP Client集約
│   ├── blueprint/              # Blueprint処理
│   ├── webhook/                # Webhook処理
│   └── types/                  # 共有型定義
│
├── mcp-servers/                # 全MCPサーバー（Rust + TypeScript）
│   ├── codex-mcp/              # Codex Official MCP
│   ├── gemini-cli-mcp/         # Gemini CLI MCP
│   ├── orchestrator-rpc/       # Orchestrator RPC Server
│   ├── filesystem/             # File System MCP
│   ├── github/                 # GitHub MCP
│   ├── playwright/             # Playwright MCP
│   ├── chrome-devtools/        # Chrome DevTools MCP
│   ├── youtube/                # YouTube MCP
│   └── sequential-thinking/    # Sequential Thinking MCP
│
├── kernel-extensions/          # AI-Native OS (既存完全移植)
│   ├── linux/                  # Linux Kernel Modules
│   ├── windows/                # Windows Kernel Driver
│   ├── rust/                   # Rust FFI
│   └── tools/
│
├── extensions/
│   └── vscode/                 # VSCode Extension (新規)
│
├── docs/
│   ├── architecture/
│   ├── api/
│   ├── guides/
│   └── SNS_POST_v1.0.0.md
│
├── .github/
│   ├── workflows/              # CI/CD
│   └── CODEOWNERS
│
├── turbo.json                  # Turborepo設定
├── package.json                # Monorepo root
└── README.md
```

## Phase 1: 新リポジトリ初期化

### 1.1 GitHub Repository作成

```bash
# GitHub CLI使用
gh repo create zapabob/prism --public \
  --description "AI-Native Code Intelligence Platform with Multi-LLM Orchestration" \
  --gitignore Node \
  --license MIT

# Clone
git clone https://github.com/zapabob/prism.git
cd prism
```

### 1.2 Turborepo初期化

```bash
npx create-turbo@latest --example with-tailwind prism
cd prism
```

**turbo.json**:

```json
{
  "$schema": "https://turbo.build/schema.json",
  "globalDependencies": ["**/.env.*local"],
  "pipeline": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": [".next/**", "dist/**", "build/**"]
    },
    "lint": {
      "outputs": []
    },
    "dev": {
      "cache": false,
      "persistent": true
    },
    "type-check": {
      "dependsOn": ["^build"]
    }
  }
}
```

## Phase 2: 型定義パッケージ (最優先)

### 2.1 `packages/types/`

**packages/types/package.json**:

```json
{
  "name": "@prism/types",
  "version": "1.0.0",
  "main": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "scripts": {
    "build": "tsc",
    "type-check": "tsc --noEmit"
  }
}
```

**packages/types/src/index.ts** (500行):

```typescript
// Git関連型
export interface Commit3D {
  sha: string
  message: string
  author: string
  authorEmail: string
  timestamp: Date
  branch: string
  parents: string[]
  x: number
  y: number
  z: number
  color: string
}

export interface FileStats {
  path: string
  additions: number
  deletions: number
  commits: number
  lastModified: Date
  heatmapIntensity: number
}

export interface BranchNode {
  name: string
  commit: string
  x: number
  y: number
  z: number
  children: string[]
}

// Worktree関連型
export interface WorktreeConfig {
  repoRoot: string
  worktreePrefix: string
  instanceId: string
}

export interface Worktree {
  path: string
  branch: string
  baseCommit: string
}

export interface WorktreeInfo {
  path: string
  commit: string
  branch: string | null
  locked: boolean
}

export type MergeResult =
  | { type: 'success'; commit: string }
  | { type: 'conflict'; conflicts: string[] }

// Orchestrated Edit型
export interface OrchestratedEditRequest {
  repoRoot: string
  filePath: string
  content: string
  preimageSHA?: string
}

export interface OrchestratedEditResult {
  success: boolean
  newSHA: string
  error: string | null
}

// Supervisor型
export enum CoordinationStrategy {
  Sequential = 'sequential',
  Parallel = 'parallel',
  Hybrid = 'hybrid'
}

export enum MergeStrategy {
  Concatenate = 'concatenate',
  Voting = 'voting',
  HighestScore = 'highest_score'
}

export enum ManagementStyle {
  Centralized = 'centralized',
  Competition = 'competition',
  Collaborative = 'collaborative'
}

export interface SupervisorConfig {
  strategy: CoordinationStrategy
  maxParallelAgents: number
  mergeStrategy: MergeStrategy
  managementStyle: ManagementStyle
}

export interface Assignment {
  stepId: string
  agentType: string
  task: string
  dependencies: string[]
  domain: string
  priority: number
}

export interface TaskResult {
  stepId: string
  agentType: string
  success: boolean
  output: string
  metrics: ScoringMetrics
  error?: string
  duration: number
}

// Consensus型
export interface ScoringMetrics {
  testSuccessRate: number
  lintPassRate: number
  performanceGain: number
  changeRisk: number
  readabilityScore: number
  securityScore: number
}

export interface ScoringWeights {
  tests: number
  linting: number
  performance: number
  risk: number
  readability: number
  security: number
}

export interface AgentVote {
  agentName: string
  preferredSolution: string
  confidence: number
  reasoning: string
}

export interface ConsensusResult {
  selectedSolution: string
  selectionStrategy: string
  finalScore: number
  votes: AgentVote[]
  decisionLog: DecisionLog
}

export interface DecisionLog {
  timestamp: Date
  participants: string[]
  votingRounds: number
  finalDecision: string
}

// Multi-AI型
export enum OrchestrationMode {
  Centralized = 'centralized',
  Competition = 'competition',
  Collaborative = 'collaborative'
}

export interface AIInstance {
  id: string
  name: string
  provider: 'codex' | 'gemini' | 'claude' | 'openai' | 'anthropic'
  model: string
  worktree?: Worktree
  status: AIStatus
}

export enum AIStatus {
  Idle = 'idle',
  Running = 'running',
  Completed = 'completed',
  Failed = 'failed',
  Voting = 'voting'
}

export interface MultiAIConfig {
  mode: OrchestrationMode
  ais: AIInstance[]
  task: string
  repoPath: string
  consensusStrategy: ConsensusStrategy
  timeout?: number
}

export interface LaunchResult {
  results: TaskResult[]
  consensus?: ConsensusResult
  winner?: AIInstance
  duration: number
}

// Blueprint型
export interface Blueprint {
  id: string
  name: string
  version: string
  description: string
  author: string
  createdAt: Date
  updatedAt: Date
  steps: BlueprintStep[]
  dependencies: string[]
  metadata: Record<string, unknown>
}

export interface BlueprintStep {
  id: string
  type: 'code' | 'test' | 'review' | 'research' | 'custom'
  description: string
  agent?: string
  inputs: Record<string, unknown>
  outputs: string[]
  successCriteria: SuccessCriteria[]
}

export interface SuccessCriteria {
  metric: string
  operator: 'gt' | 'lt' | 'eq' | 'gte' | 'lte'
  value: number | string
}

// Webhook型
export interface WebhookConfig {
  id: string
  name: string
  url: string
  events: WebhookEvent[]
  secret?: string
  active: boolean
}

export enum WebhookEvent {
  OrchestrationStart = 'orchestration.start',
  OrchestrationComplete = 'orchestration.complete',
  TaskComplete = 'task.complete',
  ConsensusReached = 'consensus.reached',
  GitCommit = 'git.commit',
  GitMerge = 'git.merge'
}

export interface WebhookPayload {
  event: WebhookEvent
  timestamp: Date
  data: Record<string, unknown>
  signature?: string
}

// MCP型
export interface MCPMessage {
  jsonrpc: '2.0'
  id?: number | string
  method?: string
  params?: unknown
  result?: unknown
  error?: MCPError
}

export interface MCPError {
  code: number
  message: string
  data?: unknown
}

export interface MCPServerConfig {
  name: string
  command: string
  args: string[]
  env?: Record<string, string>
}

// Kernel Extension型（既存移植）
export interface KernelStats {
  aiTasksDetected: number
  priorityBoosts: number
  gpuAllocations: number
  averageLatency: number
}

// すべてstrict型定義、any完全禁止
```

**packages/types/tsconfig.json**:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "commonjs",
    "declaration": true,
    "outDir": "./dist",
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "strictFunctionTypes": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noImplicitReturns": true,
    "noFallthroughCasesInSwitch": true,
    "esModuleInterop": true,
    "skipLibCheck": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
```

## Phase 3: Git Core Package

### 3.1 `packages/git-core/`

**packages/git-core/src/worktree-manager.ts** (400行):

```typescript
import { exec } from 'child_process'
import { promisify } from 'util'
import * as path from 'path'
import * as fs from 'fs/promises'
import type { Worktree, WorktreeConfig, WorktreeInfo, MergeResult } from '@prism/types'

const execAsync = promisify(exec)

export class WorktreeManager {
  constructor(private repoRoot: string) {}
  
  async create(config: WorktreeConfig): Promise<Worktree> {
    const baseCommit = await this.getCurrentCommit()
    const worktreePath = path.join(
      this.repoRoot,
      '.prism',
      'worktrees',
      `${config.worktreePrefix}-${config.instanceId}`
    )
    const branch = `${config.worktreePrefix}/${config.instanceId}/${Date.now()}`
    
    // 既存削除
    if (await this.exists(worktreePath)) {
      await this.removeInternal(worktreePath)
    }
    
    // Worktree作成
    await execAsync(
      `git worktree add -b "${branch}" "${worktreePath}" "${baseCommit}"`,
      { cwd: this.repoRoot }
    )
    
    return { path: worktreePath, branch, baseCommit }
  }
  
  async remove(worktree: Worktree): Promise<void> {
    await this.removeInternal(worktree.path)
  }
  
  private async removeInternal(worktreePath: string): Promise<void> {
    try {
      await execAsync(`git worktree remove "${worktreePath}" --force`, {
        cwd: this.repoRoot
      })
    } catch (error) {
      // Worktreeが既に削除されている場合は無視
      if (!(error instanceof Error) || !error.message.includes('not a working tree')) {
        throw error
      }
    }
  }
  
  async commit(worktree: Worktree, message: string): Promise<string> {
    await execAsync('git add -A', { cwd: worktree.path })
    await execAsync(`git commit -m "${this.escapeShell(message)}"`, {
      cwd: worktree.path
    })
    return await this.getCurrentCommit(worktree.path)
  }
  
  async merge(worktree: Worktree): Promise<MergeResult> {
    // mainブランチに戻る
    await execAsync('git checkout main', { cwd: this.repoRoot })
    
    try {
      // マージ実行
      await execAsync(
        `git merge "${worktree.branch}" --no-ff --no-edit -m "Merge AI worktree ${worktree.branch}"`,
        { cwd: this.repoRoot }
      )
      
      const commit = await this.getCurrentCommit()
      return { type: 'success', commit }
    } catch (error) {
      // コンフリクトチェック
      const conflicts = await this.getConflicts()
      if (conflicts.length > 0) {
        return { type: 'conflict', conflicts }
      }
      throw error
    }
  }
  
  async listAll(): Promise<WorktreeInfo[]> {
    const { stdout } = await execAsync('git worktree list --porcelain', {
      cwd: this.repoRoot
    })
    
    return this.parseWorktreeList(stdout)
  }
  
  private parseWorktreeList(output: string): WorktreeInfo[] {
    const worktrees: WorktreeInfo[] = []
    const lines = output.trim().split('\n')
    
    let current: Partial<WorktreeInfo> = {}
    
    for (const line of lines) {
      if (line.startsWith('worktree ')) {
        current.path = line.substring(9)
      } else if (line.startsWith('HEAD ')) {
        current.commit = line.substring(5)
      } else if (line.startsWith('branch ')) {
        current.branch = line.substring(7)
      } else if (line.startsWith('locked')) {
        current.locked = true
      } else if (line === '') {
        if (current.path && current.commit) {
          worktrees.push({
            path: current.path,
            commit: current.commit,
            branch: current.branch || null,
            locked: current.locked || false
          })
        }
        current = {}
      }
    }
    
    return worktrees
  }
  
  private async getConflicts(): Promise<string[]> {
    try {
      const { stdout } = await execAsync('git diff --name-only --diff-filter=U', {
        cwd: this.repoRoot
      })
      return stdout.trim().split('\n').filter(Boolean)
    } catch {
      return []
    }
  }
  
  private async getCurrentCommit(cwd: string = this.repoRoot): Promise<string> {
    const { stdout } = await execAsync('git rev-parse HEAD', { cwd })
    return stdout.trim()
  }
  
  private async exists(worktreePath: string): Promise<boolean> {
    try {
      await fs.access(worktreePath)
      return true
    } catch {
      return false
    }
  }
  
  private escapeShell(str: string): string {
    return str.replace(/"/g, '\\"')
  }
}
```

**packages/git-core/src/orchestrated-edit.ts** (250行):

```typescript
import * as crypto from 'crypto'
import * as fs from 'fs/promises'
import * as path from 'path'
import type { OrchestratedEditRequest, OrchestratedEditResult } from '@prism/types'

export class OrchestratedEditor {
  async safeWrite(request: OrchestratedEditRequest): Promise<OrchestratedEditResult> {
    const filePath = path.join(request.repoRoot, request.filePath)
    
    // Preimage SHA検証
    if (request.preimageSHA) {
      const validationResult = await this.validatePreimage(filePath, request.preimageSHA)
      if (!validationResult.valid) {
        return {
          success: false,
          newSHA: validationResult.actualSHA,
          error: validationResult.error
        }
      }
    }
    
    // ディレクトリ作成
    const dir = path.dirname(filePath)
    await fs.mkdir(dir, { recursive: true })
    
    // 書き込み
    await fs.writeFile(filePath, request.content, 'utf-8')
    
    const newSHA = await this.computeSHA256(filePath)
    
    return {
      success: true,
      newSHA,
      error: null
    }
  }
  
  private async validatePreimage(
    filePath: string,
    expectedSHA: string
  ): Promise<{ valid: boolean; actualSHA: string; error: string }> {
    const exists = await this.fileExists(filePath)
    
    if (exists) {
      const actualSHA = await this.computeSHA256(filePath)
      if (actualSHA !== expectedSHA) {
        return {
          valid: false,
          actualSHA,
          error: `Edit conflict: expected SHA ${expectedSHA} but found ${actualSHA}`
        }
      }
      return { valid: true, actualSHA, error: '' }
    } else {
      const emptySHA = this.computeSHA256String('')
      if (expectedSHA !== emptySHA) {
        return {
          valid: false,
          actualSHA: emptySHA,
          error: 'Edit conflict: file does not exist but preimage SHA provided'
        }
      }
      return { valid: true, actualSHA: emptySHA, error: '' }
    }
  }
  
  private async computeSHA256(filePath: string): Promise<string> {
    const content = await fs.readFile(filePath, 'utf-8')
    return this.computeSHA256String(content)
  }
  
  private computeSHA256String(content: string): string {
    return crypto.createHash('sha256').update(content).digest('hex')
  }
  
  private async fileExists(filePath: string): Promise<boolean> {
    try {
      await fs.access(filePath)
      return true
    } catch {
      return false
    }
  }
}
```

## Phase 4: Supervisor Package

### 4.1 `packages/supervisor/src/executor.ts` (500行)

Rust `codex-rs/supervisor/src/executor.rs`を完全移植:

```typescript
import type {
  Assignment,
  TaskResult,
  SupervisorConfig,
  CoordinationStrategy
} from '@prism/types'

interface DependencyGraph {
  nodes: Map<string, Assignment>
  edges: Map<string, Set<string>>
  completed: Set<string>
}

export async function executePlan(
  assignments: Assignment[],
  config: SupervisorConfig
): Promise<TaskResult[]> {
  const concurrencyLimit = getConcurrencyLimit(config)
  return await executeWithDependencies(assignments, concurrencyLimit)
}

function getConcurrencyLimit(config: SupervisorConfig): number {
  switch (config.strategy) {
    case CoordinationStrategy.Sequential:
      return 1
    case CoordinationStrategy.Parallel:
      return config.maxParallelAgents
    case CoordinationStrategy.Hybrid:
      return Math.max(2, Math.floor(config.maxParallelAgents / 2))
  }
}

async function executeWithDependencies(
  assignments: Assignment[],
  concurrencyLimit: number
): Promise<TaskResult[]> {
  const graph = buildDependencyGraph(assignments)
  const results: TaskResult[] = []
  const active = new Set<string>()
  const errors: Array<{ stepId: string; error: string }> = []
  
  while (hasRemaining(graph)) {
    const ready = getReadyTasks(graph, active, errors)
    
    if (ready.length === 0 && active.size === 0) {
      // デッドロック検出
      throw new Error('Dependency deadlock detected')
    }
    
    const batch = ready.slice(0, concurrencyLimit - active.size)
    
    if (batch.length === 0) {
      // アクティブタスク完了待ち
      await new Promise(resolve => setTimeout(resolve, 100))
      continue
    }
    
    await Promise.all(
      batch.map(async (assignment) => {
        active.add(assignment.stepId)
        
        try {
          const result = await executeSingleTask(assignment)
          results.push(result)
          
          if (!result.success) {
            errors.push({ stepId: assignment.stepId, error: result.error || 'Unknown error' })
          }
        } catch (error) {
          errors.push({
            stepId: assignment.stepId,
            error: error instanceof Error ? error.message : String(error)
          })
        } finally {
          active.delete(assignment.stepId)
          markComplete(graph, assignment.stepId)
        }
      })
    )
  }
  
  return results
}

function buildDependencyGraph(assignments: Assignment[]): DependencyGraph {
  const graph: DependencyGraph = {
    nodes: new Map(),
    edges: new Map(),
    completed: new Set()
  }
  
  for (const assignment of assignments) {
    graph.nodes.set(assignment.stepId, assignment)
    graph.edges.set(assignment.stepId, new Set(assignment.dependencies))
  }
  
  return graph
}

function hasRemaining(graph: DependencyGraph): boolean {
  return graph.nodes.size > graph.completed.size
}

function getReadyTasks(
  graph: DependencyGraph,
  active: Set<string>,
  errors: Array<{ stepId: string; error: string }>
): Assignment[] {
  const ready: Assignment[] = []
  const erroredSteps = new Set(errors.map(e => e.stepId))
  
  for (const [stepId, assignment] of graph.nodes) {
    if (graph.completed.has(stepId) || active.has(stepId)) {
      continue
    }
    
    // 依存タスクがエラーの場合はスキップ
    const hasErroredDependency = assignment.dependencies.some(dep => erroredSteps.has(dep))
    if (hasErroredDependency) {
      continue
    }
    
    // すべての依存が完了済み
    const allDepsComplete = assignment.dependencies.every(dep => graph.completed.has(dep))
    if (allDepsComplete) {
      ready.push(assignment)
    }
  }
  
  // 優先度順にソート
  return ready.sort((a, b) => b.priority - a.priority)
}

function markComplete(graph: DependencyGraph, stepId: string): void {
  graph.completed.add(stepId)
}

async function executeSingleTask(assignment: Assignment): Promise<TaskResult> {
  const startTime = Date.now()
  
  try {
    // ここでMCPクライアント経由で実際のAI実行
    // TODO: MCPクライアント統合
    
    const output = `Executed ${assignment.task} with ${assignment.agentType}`
    
    return {
      stepId: assignment.stepId,
      agentType: assignment.agentType,
      success: true,
      output,
      metrics: {
        testSuccessRate: 1.0,
        lintPassRate: 1.0,
        performanceGain: 0.0,
        changeRisk: 0.1,
        readabilityScore: 0.8,
        securityScore: 0.9
      },
      duration: Date.now() - startTime
    }
  } catch (error) {
    return {
      stepId: assignment.stepId,
      agentType: assignment.agentType,
      success: false,
      output: '',
      metrics: {
        testSuccessRate: 0,
        lintPassRate: 0,
        performanceGain: 0,
        changeRisk: 1.0,
        readabilityScore: 0,
        securityScore: 0
      },
      error: error instanceof Error ? error.message : String(error),
      duration: Date.now() - startTime
    }
  }
}
```

## Phase 5: Blueprint & Webhook Packages

### 5.1 `packages/blueprint/` (新規、300行)

**packages/blueprint/src/executor.ts**:

```typescript
import type { Blueprint, BlueprintStep, TaskResult } from '@prism/types'

export class BlueprintExecutor {
  async execute(blueprint: Blueprint): Promise<TaskResult[]> {
    const results: TaskResult[] = []
    
    for (const step of blueprint.steps) {
      const result = await this.executeStep(step, blueprint)
      results.push(result)
      
      if (!result.success) {
        throw new Error(`Blueprint step ${step.id} failed: ${result.error}`)
      }
      
      // Success Criteria検証
      const criteriaPass = await this.validateCriteria(step, result)
      if (!criteriaPass) {
        throw new Error(`Step ${step.id} did not meet success criteria`)
      }
    }
    
    return results
  }
  
  private async executeStep(
    step: BlueprintStep,
    blueprint: Blueprint
  ): Promise<TaskResult> {
    const startTime = Date.now()
    
    // Agent選択・実行
    const agent = step.agent || this.selectAgent(step.type)
    
    // TODO: MCP経由でAgent実行
    
    return {
      stepId: step.id,
      agentType: agent,
      success: true,
      output: `Executed ${step.description}`,
      metrics: {
        testSuccessRate: 1.0,
        lintPassRate: 1.0,
        performanceGain: 0,
        changeRisk: 0.1,
        readabilityScore: 0.8,
        securityScore: 0.9
      },
      duration: Date.now() - startTime
    }
  }
  
  private async validateCriteria(
    step: BlueprintStep,
    result: TaskResult
  ): Promise<boolean> {
    for (const criteria of step.successCriteria) {
      const metricValue = result.metrics[criteria.metric as keyof typeof result.metrics]
      
      if (typeof metricValue !== 'number') continue
      
      const targetValue = typeof criteria.value === 'number' ? criteria.value : parseFloat(criteria.value)
      
      switch (criteria.operator) {
        case 'gt':
          if (!(metricValue > targetValue)) return false
          break
        case 'gte':
          if (!(metricValue >= targetValue)) return false
          break
        case 'lt':
          if (!(metricValue < targetValue)) return false
          break
        case 'lte':
          if (!(metricValue <= targetValue)) return false
          break
        case 'eq':
          if (metricValue !== targetValue) return false
          break
      }
    }
    
    return true
  }
  
  private selectAgent(type: string): string {
    const agentMap: Record<string, string> = {
      code: 'CodeExpert',
      test: 'TestingExpert',
      review: 'CodeReviewer',
      research: 'DeepResearcher',
      custom: 'General'
    }
    return agentMap[type] || 'General'
  }
}
```

### 5.2 `packages/webhook/` (新規、250行)

**packages/webhook/src/manager.ts**:

```typescript
import * as crypto from 'crypto'
import type { WebhookConfig, WebhookPayload, WebhookEvent } from '@prism/types'

export class WebhookManager {
  private webhooks: Map<string, WebhookConfig> = new Map()
  
  register(config: WebhookConfig): void {
    this.webhooks.set(config.id, config)
  }
  
  unregister(id: string): void {
    this.webhooks.delete(id)
  }
  
  async trigger(event: WebhookEvent, data: Record<string, unknown>): Promise<void> {
    const payload: WebhookPayload = {
      event,
      timestamp: new Date(),
      data
    }
    
    const webhooksToTrigger = Array.from(this.webhooks.values()).filter(
      wh => wh.active && wh.events.includes(event)
    )
    
    await Promise.all(
      webhooksToTrigger.map(webhook => this.sendWebhook(webhook, payload))
    )
  }
  
  private async sendWebhook(
    webhook: WebhookConfig,
    payload: WebhookPayload
  ): Promise<void> {
    const payloadWithSignature = {
      ...payload,
      signature: webhook.secret ? this.generateSignature(payload, webhook.secret) : undefined
    }
    
    try {
      const response = await fetch(webhook.url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Prism-Event': payload.event,
          'X-Prism-Signature': payloadWithSignature.signature || ''
        },
        body: JSON.stringify(payloadWithSignature)
      })
      
      if (!response.ok) {
        console.error(`Webhook ${webhook.id} failed: ${response.statusText}`)
      }
    } catch (error) {
      console.error(`Webhook ${webhook.id} error:`, error)
    }
  }
  
  private generateSignature(payload: WebhookPayload, secret: string): string {
    const payloadString = JSON.stringify(payload)
    return crypto
      .createHmac('sha256', secret)
      .update(payloadString)
      .digest('hex')
  }
}
```

## Phase 6: MCP Servers完全移植

### 6.1 全MCPサーバーをコピー

```bash
# Codexから全MCPサーバーをコピー
cp -r ../codex/codex-rs/gemini-cli-mcp-server mcp-servers/gemini-cli-mcp/
cp -r ../codex/codex-rs/orchestrator-rpc-server mcp-servers/orchestrator-rpc/

# 既存MCPサーバー（mcp-serversディレクトリ）もコピー
# filesystem, github, playwright, chrome-devtools, youtube, sequential-thinking
```

### 6.2 MCP Clients Package

**packages/mcp-clients/src/index.ts** (800行):

```typescript
import type { MCPMessage, MCPServerConfig } from '@prism/types'

export { CodexMCPClient } from './codex-client'
export { GeminiMCPClient } from './gemini-client'
export { ClaudeMCPClient } from './claude-client'
export { OrchestratorRPCClient } from './orchestrator-rpc-client'
export { FileSystemMCPClient } from './filesystem-client'
export { GitHubMCPClient } from './github-client'
export { PlaywrightMCPClient } from './playwright-client'
export { ChromeDevToolsMCPClient } from './chrome-devtools-client'
export { YouTubeMCPClient } from './youtube-client'
export { SequentialThinkingMCPClient } from './sequential-thinking-client'

// Base Client
export abstract class BaseMCPClient {
  protected process?: ChildProcess
  
  constructor(protected config: MCPServerConfig) {}
  
  async start(): Promise<void> {
    this.process = spawn(this.config.command, this.config.args, {
      env: { ...process.env, ...this.config.env },
      stdio: ['pipe', 'pipe', 'pipe']
    })
    
    // JSON-RPC通信初期化
  }
  
  async stop(): Promise<void> {
    if (this.process) {
      this.process.kill()
      this.process = undefined
    }
  }
  
  async sendRequest(method: string, params?: unknown): Promise<unknown> {
    const message: MCPMessage = {
      jsonrpc: '2.0',
      id: Date.now(),
      method,
      params
    }
    
    // JSON-RPC送信・受信
    return await this.sendMessage(message)
  }
  
  protected abstract sendMessage(message: MCPMessage): Promise<unknown>
}
```

## Phase 7: Web App完全実装

### 7.1 API Routes (10ファイル)

**apps/web/app/api/orchestrate/launch/route.ts**:

```typescript
import { NextResponse } from 'next/server'
import { WorktreeManager } from '@prism/git-core'
import { executePlan } from '@prism/supervisor'
import type { MultiAIConfig } from '@prism/types'

export async function POST(request: Request) {
  const config: MultiAIConfig = await request.json()
  
  // TODO: Multi-AI Launch実装
  
  return NextResponse.json({
    success: true,
    launchId: 'launch-123',
    status: 'running'
  })
}
```

**apps/web/app/api/blueprint/execute/route.ts**:

```typescript
import { NextResponse } from 'next/server'
import { BlueprintExecutor } from '@prism/blueprint'
import type { Blueprint } from '@prism/types'

export async function POST(request: Request) {
  const blueprint: Blueprint = await request.json()
  
  const executor = new BlueprintExecutor()
  const results = await executor.execute(blueprint)
  
  return NextResponse.json({ results })
}
```

**apps/web/app/api/webhook/register/route.ts**:

```typescript
import { NextResponse } from 'next/server'
import { WebhookManager } from '@prism/webhook'
import type { WebhookConfig } from '@prism/types'

const webhookManager = new WebhookManager()

export async function POST(request: Request) {
  const config: WebhookConfig = await request.json()
  webhookManager.register(config)
  
  return NextResponse.json({ success: true })
}
```

### 7.2 UI Pages (5ファイル)

**apps/web/app/orchestrate/page.tsx** (600行):

```typescript
'use client'

import { useState } from 'react'
import { Canvas } from '@react-three/fiber'
import { CommitGraph3DOptimized } from '@/components/visualizations/CommitGraph3DOptimized'
import type { AIInstance, OrchestrationMode } from '@prism/types'

export default function OrchestratePage() {
  const [mode, setMode] = useState<OrchestrationMode>('centralized')
  const [ais, setAIs] = useState<AIInstance[]>([])
  const [task, setTask] = useState('')
  
  const handleLaunch = async () => {
    const response = await fetch('/api/orchestrate/launch', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ mode, ais, task, repoPath: '.' })
    })
    
    const result = await response.json()
    console.log(result)
  }
  
  return (
    <div className="grid grid-cols-12 h-screen bg-gradient-to-br from-slate-950 via-purple-950 to-slate-950">
      {/* Left Panel */}
      <div className="col-span-3 border-r border-purple-500/30 p-6">
        <h2 className="text-2xl font-bold text-purple-300 mb-6">Orchestration</h2>
        
        {/* Mode Selector */}
        <div className="mb-6">
          <label className="text-purple-200 text-sm mb-2 block">Mode</label>
          <select
            value={mode}
            onChange={(e) => setMode(e.target.value as OrchestrationMode)}
            className="w-full bg-slate-900/50 border border-purple-500/50 rounded p-2 text-purple-100"
          >
            <option value="centralized">Centralized</option>
            <option value="competition">Competition</option>
            <option value="collaborative">Collaborative</option>
          </select>
        </div>
        
        {/* AI Selector */}
        <div className="mb-6">
          <label className="text-purple-200 text-sm mb-2 block">AI Instances</label>
          {/* TODO: AI選択UI */}
        </div>
        
        {/* Task Input */}
        <div className="mb-6">
          <label className="text-purple-200 text-sm mb-2 block">Task</label>
          <textarea
            value={task}
            onChange={(e) => setTask(e.target.value)}
            className="w-full bg-slate-900/50 border border-purple-500/50 rounded p-2 text-purple-100 h-32"
            placeholder="Describe the task..."
          />
        </div>
        
        <button
          onClick={handleLaunch}
          className="w-full bg-gradient-to-r from-purple-600 to-pink-600 text-white py-3 rounded font-semibold hover:from-purple-500 hover:to-pink-500 transition"
        >
          Launch
        </button>
      </div>
      
      {/* Center: 3D Visualization */}
      <div className="col-span-6 relative">
        <Canvas camera={{ position: [0, 0, 50], fov: 75 }}>
          <ambientLight intensity={0.5} />
          <pointLight position={[10, 10, 10]} />
          <CommitGraph3DOptimized commits={[]} />
        </Canvas>
      </div>
      
      {/* Right Panel */}
      <div className="col-span-3 border-l border-purple-500/30 p-6">
        <h2 className="text-2xl font-bold text-purple-300 mb-6">Status</h2>
        {/* TODO: AI Status表示 */}
      </div>
    </div>
  )
}
```

**apps/web/app/blueprint/page.tsx** (400行):

```typescript
'use client'

import { useState } from 'react'
import type { Blueprint } from '@prism/types'

export default function BlueprintPage() {
  const [blueprint, setBlueprint] = useState<Blueprint | null>(null)
  
  const handleExecute = async () => {
    if (!blueprint) return
    
    const response = await fetch('/api/blueprint/execute', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(blueprint)
    })
    
    const result = await response.json()
    console.log(result)
  }
  
  return (
    <div className="min-h-screen bg-gradient-to-br from-slate-950 via-blue-950 to-slate-950 p-8">
      <h1 className="text-4xl font-bold text-blue-300 mb-8">Blueprint Editor</h1>
      
      {/* Blueprint Editor UI */}
      <div className="grid grid-cols-2 gap-8">
        {/* Left: Visual Editor */}
        <div className="bg-slate-900/50 border border-blue-500/30 rounded-lg p-6">
          <h2 className="text-2xl font-semibold text-blue-200 mb-4">Visual Editor</h2>
          {/* TODO: ノードベースエディタ */}
        </div>
        
        {/* Right: JSON Editor */}
        <div className="bg-slate-900/50 border border-blue-500/30 rounded-lg p-6">
          <h2 className="text-2xl font-semibold text-blue-200 mb-4">JSON Editor</h2>
          <textarea
            className="w-full h-96 bg-slate-800 border border-blue-500/50 rounded p-4 text-blue-100 font-mono text-sm"
            value={blueprint ? JSON.stringify(blueprint, null, 2) : ''}
            onChange={(e) => {
              try {
                setBlueprint(JSON.parse(e.target.value))
              } catch {}
            }}
          />
          
          <button
            onClick={handleExecute}
            className="mt-4 w-full bg-gradient-to-r from-blue-600 to-cyan-600 text-white py-3 rounded font-semibold"
          >
            Execute Blueprint
          </button>
        </div>
      </div>
    </div>
  )
}
```

## Phase 8: Kernel Extensions完全コピー

```bash
# 既存のkernel-extensionsディレクトリ全体をコピー
cp -r ../codex/kernel-extensions ./

# 必要な修正
# - パスの更新
# - README更新
```

## Phase 9: VSCode Extension (新規)

**extensions/vscode/package.json**:

```json
{
  "name": "prism-vscode",
  "displayName": "Prism",
  "version": "1.0.0",
  "engines": {
    "vscode": "^1.80.0"
  },
  "activationEvents": ["*"],
  "main": "./dist/extension.js",
  "contributes": {
    "commands": [
      {
        "command": "prism.orchestrate",
        "title": "Prism: Orchestrate AI"
      },
      {
        "command": "prism.blueprint",
        "title": "Prism: Execute Blueprint"
      }
    ]
  }
}
```

## Phase 10: Documentation & CI/CD

### 10.1 README.md (300行)

````markdown
# Prism - AI-Native Code Intelligence Platform

Multi-LLM orchestration with conflict-free concurrent development using Git worktrees.

## Features

- 🤖 **Multi-AI Orchestration**: Run unlimited AIs in parallel
- 🌳 **Git Worktree Integration**: Conflict-free development
- 🎯 **Blueprint System**: Define complex workflows
- 📊 **3D Git Visualization**: Kamui4d-inspired visualization
- 🔔 **Webhook System**: Event-driven integrations
- 🖥️ **AI-Native OS**: Kernel-level optimizations
- 🔌 **10+ MCP Servers**: Extensible architecture

## Quick Start

```bash
npm install -g @prism/cli
prism init
prism orchestrate --mode competition --task "Implement auth"
````

## Architecture

[アーキテクチャ図]

## License

MIT

````

### 10.2 CI/CD

**.github/workflows/ci.yml**:
```yaml
name: CI

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - run: npm install
      - run: npm run build
      - run: npm run type-check
      - run: npm run lint
      - run: npm test
      - run: npm audit --production
````

## 実装優先順位

### Week 1: Foundation

1. リポジトリ作成・Turborepo初期化
2. 型定義パッケージ完成
3. Git Coreパッケージ完成
4. Supervisorパッケージ完成

### Week 2: Integration

5. Blueprint & Webhookパッケージ
6. MCP Clientsパッケージ
7. 全MCPサーバー移植

### Week 3: UI

8. Web App API Routes
9. Orchestration Dashboard
10. Blueprint Editor
11. Visualizationコンポーネント移植

### Week 4: Polish

12. Kernel Extensions移植
13. VSCode Extension
14. Documentation
15. CI/CD

## 成功基準

- ✅ TypeScript errors: 0
- ✅ ESLint warnings: 0
- ✅ npm audit vulnerabilities: 0
- ✅ All tests passing
- ✅ 10 AI並列: < 3秒起動
- ✅ Git worktree: コンフリクト0
- ✅ 50K commits可視化: 35+ FPS
- ✅ Kernel modules: Linux + Windows動作

## 総見積もり

- パッケージ: 8個
- MCPサーバー: 10個
- API Routes: 10個
- UIページ: 5個
- 型定義: strict、any禁止
- 総行数: ~25,000行

### To-dos

- [ ] lib/ai/multi-launcher.ts実装（ParallelAILauncherクラス、Codex/Gemini/Claude起動ロジック）
- [ ] components/multi-ai-panel.tsx実装（React UI、リアルタイム出力表示、ステータス管理）
- [ ] child_processでプロセス管理、stdout/stderr監視、エラーハンドリング
- [ ] Supervisor consensus機構をTypeScriptに移植、結果集約・投票・最良解選択
- [ ] Next.js API Routes実装（/api/launch-ai、/api/ai-status、WebSocketサポート）
- [ ] APIキー個別暗号化、プロセスサンドボックス、出力サニタイズ