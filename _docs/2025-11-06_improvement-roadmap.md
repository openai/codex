# Codex v2.x 改善ロードマップ

**作成日時**: 2025-11-06 18:35:00  
**基準バージョン**: codex-cli 2.0.0  
**レビュー**: _docs/2025-11-06_code-review-evaluation.md 参照

---

## 🎯 改善方針の優先度分類

### P0 (Critical - v2.0.0に必須)
v2.0.0リリース前に必ず実装

### P1 (High - v2.1.0目標)
次期マイナーバージョンで実装

### P2 (Medium - v2.2.0以降)
機能追加・最適化

### P3 (Low - v3.0.0検討)
大規模リファクタリング

---

## 🔥 P0: v2.0.0必須項目

### 1. Git 4D可視化 (xyz+t)

**現状の問題**:
- TUIは3D表示のみ（時刻軸なし）
- GUIは未実装

**実装タスク**:
```rust
// codex-rs/tui/src/git_visualizer.rs
pub struct CommitNode4D {
    pub pos: (f32, f32, f32),      // xyz
    pub timestamp: DateTime<Utc>,  // t
    pub heat: f32,                 // 変更頻度
    pub size: f32,                 // 変更量
    pub connections: Vec<String>,
}

pub struct TimelineControl {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub current_time: DateTime<Utc>,
    pub playback_speed: f32,        // 1.0 = リアルタイム
}
```

**期限**: Week 2-3  
**担当**: git-4d-tui, git-4d-gui TODO

---

### 2. VR基本対応（Quest 2）

**現状の問題**:
- VR機能が完全に未実装

**実装タスク**:
```typescript
// codex-rs/tauri-gui/src/pages/GitVR.tsx
import { Canvas } from '@react-three/fiber'
import { VRButton, XR, Controllers } from '@react-three/xr'

export default function GitVR() {
  return (
    <>
      <VRButton />
      <Canvas>
        <XR>
          <GitGraph4D />
          <Controllers />
        </XR>
      </Canvas>
    </>
  )
}
```

**Quest 2特化**:
- 解像度: 1832x1920/eye
- リフレッシュレート: 90Hz
- メモリ: 6GB制約
- コントローラー優先UI

**期限**: Week 4-5  
**担当**: vr-quest2 TODO

---

### 3. npm パッケージ化

**現状の問題**:
- Rustビルドが必須（ユーザー環境依存）

**実装タスク**:
```json
// package.json
{
  "name": "@zapabob/codex-cli",
  "version": "2.0.0",
  "description": "AI-Native OS with VR/AR Git visualization",
  "bin": {
    "codex": "./bin/codex"
  },
  "scripts": {
    "postinstall": "node scripts/install-binary.js"
  },
  "os": ["win32", "darwin", "linux"],
  "cpu": ["x64", "arm64"]
}
```

**binary配布戦略**:
- GitHub Releases からダウンロード
- プラットフォーム自動検出
- SHA256検証

**期限**: Week 3  
**担当**: npm-package TODO

---

## 🚀 P1: v2.1.0目標

### 4. CUDA LLM推論統合

**技術選定**:
- **vLLM**: 高スループット推論（推奨）
- **TensorRT-LLM**: NVIDIA最適化
- **llama.cpp CUDA**: 軽量

**実装方針**:
```rust
// codex-rs/cuda-runtime/src/inference.rs
pub struct CudaInferenceEngine {
    model_path: PathBuf,
    quantization: Quantization,  // INT8, INT4
    context: CudaContext,
}

impl CudaInferenceEngine {
    pub async fn infer(&self, prompt: &str, max_tokens: usize) -> Result<String> {
        // vLLM Python binding経由
        // or TensorRT C++ API
    }
}
```

**性能目標**:
- 10-100x CPU比高速化
- 100 tokens/sec @ RTX 3080
- バッチ推論対応

**期限**: v2.1.0（1-2ヶ月）

---

### 5. CI/CD完全構築

**実装ファイル**:
```yaml
# .github/workflows/ci.yml
name: Codex CI

on:
  push:
    branches: [main, develop]
  pull_request:

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-22.04, windows-2022, macos-13]
        rust: [stable, nightly]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@${{ matrix.rust }}
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features
      
  coverage:
    runs-on: ubuntu-22.04
    steps:
      - run: cargo tarpaulin --all-features --out Xml
      - uses: codecov/codecov-action@v4
      
  release:
    if: startsWith(github.ref, 'refs/tags/v')
    needs: [test]
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
          - os: windows-2022
            target: x86_64-pc-windows-msvc
          - os: macos-13
            target: x86_64-apple-darwin
    steps:
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
```

**期限**: v2.1.0

---

### 6. テストカバレッジ80%達成

**戦略**:
1. **Unit tests**: 各関数の単体テスト
2. **Integration tests**: モジュール間連携
3. **E2E tests**: ユーザーシナリオ
4. **Property-based**: QuickCheck / proptest
5. **Fuzzing**: cargo-fuzz

**ツール導入**:
```bash
# カバレッジ計測
cargo install cargo-tarpaulin
cargo tarpaulin --all-features --out Html

# Fuzzing
cargo install cargo-fuzz
cargo fuzz run target_name
```

**期限**: v2.1.0

---

## 🌟 P2: v2.2.0以降

### 7. コスト追跡ダッシュボード

**UI実装**:
```typescript
// tauri-gui/src/pages/CostDashboard.tsx
export default function CostDashboard() {
  const { totalCost, breakdown } = useCostTracking()
  
  return (
    <div>
      <h2>Total Cost: ${totalCost}</h2>
      <PieChart data={breakdown} />
      <CostTimeline />
    </div>
  )
}
```

**バックエンド**:
```rust
// core/src/cost/tracker.rs
pub struct CostTracker {
    model_prices: HashMap<String, ModelPricing>,
    usage_log: Vec<UsageRecord>,
}

pub struct ModelPricing {
    input_per_1k: f64,  // $/1K tokens
    output_per_1k: f64,
}
```

**期限**: v2.2.0

---

### 8. プロンプトA/Bテスト

**実装**:
```rust
// core/src/prompts/ab_test.rs
pub struct PromptExperiment {
    name: String,
    variant_a: String,
    variant_b: String,
    traffic_split: f32,  // 0.5 = 50/50
}

pub struct ExperimentResult {
    variant: String,
    success_rate: f32,
    avg_latency_ms: f64,
    user_satisfaction: f32,
}
```

**期限**: v2.2.0

---

### 9. エージェント学習機能

**概念**:
```rust
// core/src/agents/learning.rs
pub struct AgentLearner {
    execution_history: Vec<ExecutionRecord>,
    success_patterns: Vec<Pattern>,
}

impl AgentLearner {
    pub fn learn_from_execution(&mut self, result: &AgentResult) {
        // 成功パターン抽出
        // 失敗原因分析
        // 次回実行時のプロンプト最適化
    }
}
```

**期限**: v2.3.0

---

## 🔮 P3: v3.0.0検討事項

### 10. 完全分散型アーキテクチャ

**現状**: 中央集権型オーケストレーション  
**目標**: P2P型エージェントネットワーク

**技術**:
- libp2p
- IPFS
- ブロックチェーン（実行記録）

**期限**: v3.0.0

---

### 11. Quantum Computing準備

**長期ビジョン**:
- IBM Qiskit統合
- 量子アニーリング（最適化問題）
- 量子ML（QAOA）

**期限**: v3.0.0+

---

## 📅 リリースタイムライン

```
v2.0.0 (Week 3-4)
├─ Git 4D可視化
├─ VR基本対応 (Quest 2)
├─ npm パッケージ化
└─ ドキュメント完全版

v2.1.0 (Month 2-3)
├─ GPU LLM推論
├─ CI/CD完全構築
├─ テストカバレッジ80%
├─ Quest 3/Pro完全対応
└─ Vision Pro対応

v2.2.0 (Month 4-6)
├─ コスト追跡ダッシュボード
├─ プロンプトA/Bテスト
├─ SteamVR対応
└─ パフォーマンス最適化

v2.3.0 (Month 7-9)
├─ エージェント学習機能
├─ 分散型オーケストレーション（POC）
└─ マルチGPU対応

v3.0.0 (Year 2)
├─ 完全分散型アーキテクチャ
├─ Quantum Computing統合
└─ AI OS完全体
```

---

## 🎬 アクションアイテム

### 即座に着手（今週）
- [x] コードレビュー完了
- [x] 評価ログ作成
- [ ] 改善ロードマップ作成（本ドキュメント）
- [ ] README.md更新
- [ ] architecture-v2.0.0.mmd作成
- [ ] Git 4D可視化TUI実装開始

### 今月中
- [ ] VR Quest 2基本実装
- [ ] npm パッケージ公開
- [ ] v2.0.0リリース

### 次期バージョン
- [ ] GPU LLM推論（v2.1.0）
- [ ] CI/CD構築（v2.1.0）
- [ ] Quest 3/Pro対応（v2.1.0）

---

**🎵 終わったぜ！このロードマップで進めれば、Kamui4Dどころか、Kamui10Dくらいまで行けるやろ！**
