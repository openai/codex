<!-- 035d7493-1dfd-4f5f-a8b2-e7f9e080a1af 543fb583-f843-4034-b16f-6d806cc5d49e -->
# AI-Native OS v2.0 - Complete Implementation Plan

Codexカーネル統合 + GPU最適化 + Kamui4d超えGit可視化 + AIオーケストレーション開発環境

## Phase 1: 警告修正とコード品質向上 (v1.2.1)

### 1.1 Rust警告の修正

**File**: `codex-rs/core/src/blueprint/execution_log.rs`

- 削除: 未使用の`HashMap` import（8行目）
- 確認: `HashMap`が実際に使用されているか検証

**File**: `codex-rs/core/src/blueprint/executor.rs`

- 削除: 未使用の`BlueprintState` import（6行目）
- 確認: `BlueprintState`の使用状況を確認
```bash
cd codex-rs
cargo fix --lib -p codex-core --allow-dirty
cargo build --release
```


### 1.2 TypeScript完全型安全化

**File**: `codex-rs/tauri-gui/src/components/git/Scene3D.tsx`

- すでに完了 ✅

## Phase 2: GPU描画最適化 (v1.3.0)

### 2.1 Windows AI Kernel Driver拡張

**File**: `kernel-extensions/windows/ai_driver/gpu_rendering.c` (新規)

```c
// GPU-accelerated rendering for 3D visualization
#include <d3d12.h>
#include "ai_driver.h"

typedef struct _GPU_RENDER_CONTEXT {
    ID3D12Device* device;
    ID3D12CommandQueue* commandQueue;
    ID3D12CommandAllocator* commandAllocator;
    ID3D12GraphicsCommandList* commandList;
    
    // Commit graph optimization
    UINT maxCommits;
    FLOAT* vertexBuffer;
    UINT* indexBuffer;
} GPU_RENDER_CONTEXT;

NTSTATUS InitGPURenderContext(GPU_RENDER_CONTEXT* context) {
    // DirectX 12初期化
    // コミットグラフ用GPU最適化バッファ確保
}

NTSTATUS OptimizeCommitRendering(
    GPU_RENDER_CONTEXT* context,
    COMMIT_DATA* commits,
    UINT commitCount
) {
    // GPU上でインスタンスレンダリング
    // LOD (Level of Detail) 自動調整
    // フラストラムカリング
}
```

**File**: `kernel-extensions/windows/codex_win_api/src/lib.rs`

新しいIOCTL追加：

```rust
pub const IOCTL_GPU_INIT_RENDER: u32 = 0x80002040;
pub const IOCTL_GPU_OPTIMIZE_COMMIT: u32 = 0x80002044;

#[repr(C)]
pub struct CommitRenderData {
    pub commit_count: u32,
    pub vertex_data: *mut f32,
    pub color_data: *mut u32,
    pub lod_level: u32,
}

pub fn gpu_optimize_commit_rendering(
    driver: &Handle,
    commits: &[CommitRenderData]
) -> Result<()> {
    // GPU最適化レンダリング呼び出し
}
```

### 2.2 React Three Fiber GPU最適化

**File**: `codex-rs/tauri-gui/src/components/git/GPUOptimizer.tsx` (新規)

```typescript
import { useEffect } from 'react'
import { useThree } from '@react-three/fiber'
import * as THREE from 'three'

interface GPUOptimizerProps {
  commitCount: number
  maxFPS?: number
}

export function GPUOptimizer({ commitCount, maxFPS = 60 }: GPUOptimizerProps) {
  const { gl, scene, camera } = useThree()
  
  useEffect(() => {
    // WebGL最適化設定
    gl.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    gl.shadowMap.enabled = false  // 初期はシャドウOFF
    
    // 大量コミット時の自動LOD
    if (commitCount > 100) {
      scene.traverse((object) => {
        if (object instanceof THREE.Mesh) {
          // ジオメトリ簡略化
          object.geometry = simplifyGeometry(object.geometry, 0.5)
        }
      })
    }
    
    // FPSリミッター
    gl.setAnimationLoop((time) => {
      const delta = time - lastTime
      if (delta >= 1000 / maxFPS) {
        // レンダリング
        lastTime = time
      }
    })
  }, [commitCount, maxFPS, gl, scene])
  
  return null
}

function simplifyGeometry(
  geometry: THREE.BufferGeometry,
  ratio: number
): THREE.BufferGeometry {
  // メッシュ簡略化アルゴリズム
  // 遠距離のコミットは低ポリゴン化
}
```

### 2.3 カーネル統合Tauriコマンド

**File**: `codex-rs/tauri-gui/src-tauri/src/gpu_bridge.rs` (新規)

```rust
use codex_win_api::{gpu_optimize_commit_rendering, CommitRenderData};
use tauri::command;

#[command]
pub async fn optimize_gpu_rendering(
    commit_count: u32,
    lod_level: u32
) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let driver = codex_win_api::open_driver()
            .map_err(|e| e.to_string())?;
        
        gpu_optimize_commit_rendering(&driver, commit_count, lod_level)
            .map_err(|e| e.to_string())?;
        
        Ok("GPU rendering optimized".to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    Ok("GPU optimization not available on this platform".to_string())
}
```

## Phase 3: Kamui4d超えGit可視化 (v1.4.0)

### 3.1 AIコミット品質スコア

**File**: `codex-rs/tauri-gui/src/components/git/CommitQualityAnalyzer.tsx` (新規)

```typescript
interface CommitWithQuality extends Commit4D {
  aiScore: {
    codeQuality: number      // 0-100
    testCoverage: number     // 0-100
    documentation: number    // 0-100
    complexity: number       // 0-100 (低いほど良い)
    overall: number          // 総合スコア
  }
}

async function analyzeCommitQuality(
  commit: Commit4D
): Promise<CommitWithQuality> {
  // Codex API経由でAI分析
  const response = await invoke('analyze_commit_quality', {
    sha: commit.sha
  })
  
  return {
    ...commit,
    aiScore: response.score
  }
}

// 可視化：色でスコア表現
function getCommitColor(score: number): string {
  if (score >= 80) return '#00ff00'  // 緑：高品質
  if (score >= 60) return '#ffff00'  // 黄：中品質
  if (score >= 40) return '#ff8800'  // オレンジ：要改善
  return '#ff0000'  // 赤：低品質
}
```

### 3.2 リアルタイムWorktree Diff可視化

**File**: `codex-rs/tauri-gui/src/components/git/WorktreeDiffView.tsx` (新規)

```typescript
import { useEffect, useState } from 'react'
import * as THREE from 'three'

interface WorktreeBranch {
  name: string
  commits: Commit4D[]
  position: THREE.Vector3  // 3D空間での位置
  aiAgent: 'codex' | 'gemini' | 'claude'
}

export function WorktreeDiffView() {
  const [worktrees, setWorktrees] = useState<WorktreeBranch[]>([])
  const [conflicts, setConflicts] = useState<ConflictPrediction[]>([])
  
  useEffect(() => {
    // リアルタイムでworktree監視
    const unlisten = listen('worktree:update', (event) => {
      updateWorktreeVisualization(event.payload)
    })
    
    return () => { unlisten.then(fn => fn()) }
  }, [])
  
  return (
    <Canvas>
      {/* 各worktreeを異なる高さに配置 */}
      {worktrees.map((wt, i) => (
        <WorktreeVisualization
          key={wt.name}
          branch={wt}
          yOffset={i * 30}
          conflicts={conflicts.filter(c => c.branch === wt.name)}
        />
      ))}
      
      {/* コンフリクト予測線 */}
      {conflicts.map(conflict => (
        <ConflictWarningLine
          key={conflict.id}
          from={conflict.source}
          to={conflict.target}
          severity={conflict.severity}
        />
      ))}
    </Canvas>
  )
}
```

### 3.3 AIコンフリクト予測

**File**: `codex-rs/core/src/git/conflict_predictor.rs` (新規)

```rust
use git2::Repository;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ConflictPrediction {
    pub id: String,
    pub source_branch: String,
    pub target_branch: String,
    pub file_path: String,
    pub severity: f32,  // 0.0-1.0
    pub predicted_conflict_type: ConflictType,
    pub suggested_resolution: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ConflictType {
    TextualConflict,
    LogicalConflict,  // 同じ関数を異なる方法で変更
    SemanticConflict, // 型不整合など
}

pub async fn predict_conflicts(
    repo: &Repository,
    source: &str,
    target: &str
) -> Result<Vec<ConflictPrediction>> {
    // AI分析でコンフリクト予測
    // 1. 両ブランチの変更ファイル取得
    // 2. AIで意味的な衝突を検出
    // 3. 解決方法を提案
}
```

## Phase 4: AIオーケストレーション開発環境 (v2.0.0)

### 4.1 Worktree Manager

**File**: `codex-rs/core/src/git/worktree_manager.rs` (新規)

```rust
use git2::{Repository, Worktree};
use std::path::PathBuf;

pub struct WorktreeCompetition {
    repo: Repository,
    worktrees: Vec<CompetitionWorktree>,
}

pub struct CompetitionWorktree {
    pub name: String,
    pub path: PathBuf,
    pub ai_agent: AIAgent,
    pub task: String,
    pub status: WorktreeStatus,
}

pub enum AIAgent {
    Codex,
    GeminiCLI,
    ClaudeCode,
}

pub enum WorktreeStatus {
    InProgress,
    ReadyForReview,
    Merged,
    Failed,
}

impl WorktreeCompetition {
    pub async fn start_competition(
        &mut self,
        task: &str,
        agents: Vec<AIAgent>
    ) -> Result<Vec<String>> {
        let mut worktree_ids = Vec::new();
        
        for agent in agents {
            // 各AIエージェント用のworktree作成
            let branch_name = format!("ai-{}-{}", agent.name(), uuid::Uuid::new_v4());
            let worktree_path = self.repo.path().join(&branch_name);
            
            let worktree = self.repo.worktree(
                &branch_name,
                &worktree_path,
                None
            )?;
            
            // AIエージェントにタスク割り当て
            self.assign_task_to_agent(&agent, &worktree, task).await?;
            
            worktree_ids.push(branch_name);
        }
        
        Ok(worktree_ids)
    }
    
    async fn assign_task_to_agent(
        &self,
        agent: &AIAgent,
        worktree: &Worktree,
        task: &str
    ) -> Result<()> {
        match agent {
            AIAgent::Codex => {
                // Codex APIでタスク実行
                codex_api::execute_task(worktree.path(), task).await?;
            }
            AIAgent::GeminiCLI => {
                // Gemini CLI MCPでタスク実行
                gemini_cli::execute_task(worktree.path(), task).await?;
            }
            AIAgent::ClaudeCode => {
                // Claude Code APIでタスク実行
                claude_api::execute_task(worktree.path(), task).await?;
            }
        }
        Ok(())
    }
}
```

### 4.2 AI Orchestrator

**File**: `codex-rs/core/src/orchestration/ai_orchestrator.rs` (新規)

```rust
use tokio::sync::mpsc;

pub struct AIOrchestrator {
    agents: Vec<Box<dyn AIAgentTrait>>,
    results: mpsc::Receiver<AgentResult>,
    consensus: ConsensusEngine,
}

#[async_trait]
pub trait AIAgentTrait: Send + Sync {
    async fn execute_task(&self, task: &Task) -> Result<AgentResult>;
    fn name(&self) -> &str;
    fn capabilities(&self) -> Vec<Capability>;
}

pub struct ConsensusEngine {
    voting_strategy: VotingStrategy,
}

pub enum VotingStrategy {
    Majority,           // 過半数
    WeightedScore,      // AIスコア加重
    BestOfBreed,        // 最高品質を選択
}

impl AIOrchestrator {
    pub async fn execute_parallel(
        &mut self,
        task: Task
    ) -> Result<ConsensusSolution> {
        let mut handles = vec![];
        
        // 並列実行
        for agent in &self.agents {
            let agent = agent.clone();
            let task = task.clone();
            
            let handle = tokio::spawn(async move {
                agent.execute_task(&task).await
            });
            
            handles.push(handle);
        }
        
        // 結果収集
        let results = futures::future::join_all(handles).await;
        
        // コンセンサス形成
        self.consensus.determine_best_solution(results)
    }
}
```

### 4.3 MCP Servers統合

**File**: `codex-rs/tauri-gui/src-tauri/src/mcp_integration.rs` (新規)

```rust
use serde_json::Value;

pub struct MCPServerManager {
    servers: HashMap<String, MCPServer>,
}

pub struct MCPServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub capabilities: Vec<String>,
}

impl MCPServerManager {
    pub fn load_from_config() -> Result<Self> {
        // ~/.codex/config.tomlから読み込み
        let config_path = dirs::home_dir()
            .unwrap()
            .join(".codex")
            .join("config.toml");
        
        // MCP serversセクションを解析
    }
    
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args: Value
    ) -> Result<Value> {
        // MCPサーバーのツール呼び出し
        // 例: gemini-cli-mcp の googleSearch
        // 例: filesystem の read_file
        // 例: github の create_pull_request
    }
}
```

### 4.4 統合UI

**File**: `codex-rs/tauri-gui/src/pages/AIOrchestration.tsx` (新規)

```typescript
export default function AIOrchestration() {
  const [competition, setCompetition] = useState<Competition | null>(null)
  const [agents] = useState([
    { id: 'codex', name: 'Codex', enabled: true },
    { id: 'gemini', name: 'Gemini CLI', enabled: true },
    { id: 'claude', name: 'Claude Code', enabled: true }
  ])
  
  const startCompetition = async () => {
    const task = taskInput.value
    const enabledAgents = agents.filter(a => a.enabled)
    
    const result = await invoke('start_ai_competition', {
      task,
      agents: enabledAgents.map(a => a.id)
    })
    
    setCompetition(result)
  }
  
  return (
    <div className="orchestration-page">
      <h1>AI Worktree Competition</h1>
      
      <div className="task-input">
        <textarea placeholder="Enter task description..." />
        <button onClick={startCompetition}>Start Competition</button>
      </div>
      
      {competition && (
        <div className="competition-view">
          <WorktreeDiffView worktrees={competition.worktrees} />
          
          <div className="results-panel">
            {competition.results.map(result => (
              <AgentResultCard key={result.agent} result={result} />
            ))}
          </div>
          
          <div className="consensus">
            <h3>Consensus Solution</h3>
            <CodeDiff
              original={competition.original}
              solution={competition.consensusSolution}
            />
          </div>
        </div>
      )}
    </div>
  )
}
```

## Phase 5: VR/AR統合 (v2.1.0)

### 5.1 VR再統合

**Dependencies再追加**:

```json
{
  "@react-three/xr": "^6.2.0"
}
```

**File**: `codex-rs/tauri-gui/src/components/vr/VRGitVisualization.tsx` (新規)

```typescript
import { XR, VRButton, useXR } from '@react-three/xr'
import Scene4D from '../git/Scene4D'

export function VRGitVisualization() {
  const { isPresenting } = useXR()
  
  return (
    <>
      <VRButton />
      <Canvas>
        <XR>
          <Scene4D commits={commits} />
          {isPresenting && <VRControls />}
        </XR>
      </Canvas>
    </>
  )
}
```

## Phase 6: リソース自動コントロール (v2.2.0)

### 6.1 カーネルレベルリソース管理

**File**: `kernel-extensions/windows/ai_driver/resource_controller.c` (新規)

```c
// AI負荷に応じたリソース自動調整
typedef struct _RESOURCE_POLICY {
    UINT maxCpuPercent;
    UINT maxMemoryMB;
    UINT maxGpuPercent;
    BOOLEAN dynamicScaling;
} RESOURCE_POLICY;

NTSTATUS ApplyResourcePolicy(
    HANDLE processHandle,
    RESOURCE_POLICY* policy
) {
    // CPU affinity設定
    // メモリクォータ設定
    // GPU使用率制限
}
```

## Success Criteria

### v1.2.1 (警告修正)

- ✅ Rustコンパイル警告 0件
- ✅ TypeScript型エラー 0件

### v1.3.0 (GPU最適化)

- ✅ 1000+コミットでも60 FPS維持
- ✅ GPU使用率 < 70%
- ✅ カーネルドライバー統合動作

### v1.4.0 (Git可視化)

- ✅ AIコミット品質スコア表示
- ✅ Worktree diff リアルタイム表示
- ✅ コンフリクト予測精度 > 80%

### v2.0.0 (AIオーケストレーション)

- ✅ 3 AIエージェント並列実行
- ✅ Worktreeコンペティション動作
- ✅ コンセンサスアルゴリズム実装

### v2.1.0 (VR/AR)

- ✅ Quest 2/3対応
- ✅ VRでのGit操作
- ✅ 空間UI実装

### v2.2.0 (リソース管理)

- ✅ 無制限AIエージェント実行
- ✅ 自動リソーススケーリング
- ✅ カーネルレベル最適化

## Implementation Order

1. **Phase 1** (1日) - 警告修正
2. **Phase 2** (3-5日) - GPU最適化
3. **Phase 3** (5-7日) - Git可視化強化
4. **Phase 4** (10-14日) - AIオーケストレーション
5. **Phase 5** (3-5日) - VR/AR統合
6. **Phase 6** (5-7日) - リソース管理

合計: **27-39日** で完全実装

### To-dos

- [ ] Install @types/three and verify React Three Fiber dependencies
- [ ] Create desktop-only Scene3D.tsx component with instanced rendering
- [ ] Create Scene4D.tsx with time-travel axis (W-dimension)
- [ ] Update GitVR.tsx page to use Scene4D component
- [ ] Remove @react-three/xr and delete VR-specific files
- [ ] Update tsconfig.json with proper Three.js types configuration
- [ ] Clean install dependencies and verify TypeScript build (0 errors)
- [ ] Build Tauri MSI and install v1.2.0
- [ ] Install @types/three and verify React Three Fiber dependencies
- [ ] Create desktop-only Scene3D.tsx component with instanced rendering
- [ ] Create Scene4D.tsx with time-travel axis (W-dimension)
- [ ] Update GitVR.tsx page to use Scene4D component
- [ ] Remove @react-three/xr and delete VR-specific files
- [ ] Update tsconfig.json with proper Three.js types configuration
- [ ] Clean install dependencies and verify TypeScript build (0 errors)
- [ ] Build Tauri MSI and install v1.2.0
- [ ] Install @types/three and verify React Three Fiber dependencies
- [ ] Create desktop-only Scene3D.tsx component with instanced rendering
- [ ] Create Scene4D.tsx with time-travel axis (W-dimension)
- [ ] Update GitVR.tsx page to use Scene4D component
- [ ] Remove @react-three/xr and delete VR-specific files
- [ ] Update tsconfig.json with proper Three.js types configuration
- [ ] Clean install dependencies and verify TypeScript build (0 errors)