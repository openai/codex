# Codex Agents Starter Kit — Sub-Agents × Deep Research（実装テンプレ集）

> このmdは、前回の「Meta-Prompt for Codex: Sub-Agents × Deep Research」と**同様の形式**で、すぐ使えるテンプレ・雛形・チェックリストをまとめたスターターキットです。  
> 想定対象：`openai/codex` と `zapabob/codex` の両系での運用。`CLI/IDE/Web/GitHub` 連携を前提。

---

## 0) 目的（Why this kit）
- サブエージェントとDeep Researchの**実運用**を「定義→委任→検証→PR/レポ出力」まで一気通貫にする。
- **安全性（権限最小・監査）**, **再現性（Artifacts/ログ）**, **コスト管理（Token Budgeter/軽量版）** を初期から組み込む。

---

## 1) ディレクトリ構成（提案）
```text
.codex/
  agents/
    researcher.yaml
    test-gen.yaml
    typing-strict.yaml
    sec-audit.yaml
  policies/
    net.allowlist
    mcp.allowlist
    secrets.sample.env
  prompts/
    meta-prompt.md              # 前回のMeta-Promptを配置
  scripts/
    run_research.sh
    run_delegate.sh
artifacts/
  .gitkeep
README.md
```

---

## 2) サブエージェント雛形（YAML / 3例）

### 2.1 Deep Researcher
```yaml
# .codex/agents/researcher.yaml
name: "Deep Researcher"
goal: "計画→探索→反証→出典付きレポを生成する"
tools:
  - mcp: ["search", "crawler", "pdf_reader"]
  - fs.read
  - fs.write: ["./artifacts"]
  - net.allow: ["https://*", "http://*"]    # 実運用では policies/net.allowlist を参照
policies:
  shell: ["none"]
  context:
    max_tokens: 24000
    retention: "job"
success_criteria:
  - "複数ドメインの出典"
  - "矛盾検知ログが添付"
  - "要約は結論→根拠→限界の順で簡潔"
artifacts:
  - "artifacts/report.md"
  - "artifacts/evidence/*.json"
```

### 2.2 Test Generator
```yaml
# .codex/agents/test-gen.yaml
name: "Test Generator"
goal: "差分に対するユニット/回帰テストを自動生成しカバレッジ+10%"
tools:
  - fs.read
  - fs.write
  - shell.exec: ["npm", "pytest", "go", "cargo"]
  - mcp: ["code_indexer"]
policies:
  net: ["none"]
  context:
    max_tokens: 16000
    retention: "job"
success_criteria:
  - "CI green"
  - "coverage_delta >= 10%"
artifacts:
  - "artifacts/test-report.md"
```

### 2.3 Security Auditor
```yaml
# .codex/agents/sec-audit.yaml
name: "Security Auditor"
goal: "CVE横断・依存監査・静的解析→脆弱性要約と修正提案PR"
tools:
  - fs.read
  - fs.write
  - shell.exec: ["npm", "pip", "cargo", "go", "snyk", "trivy"]
  - mcp: ["code_indexer","search","crawler","pdf_reader"]
policies:
  net.allow: ["https://nvd.nist.gov","https://github.com","https://cve.mitre.org","https://*"]
  context:
    max_tokens: 20000
    retention: "job"
success_criteria:
  - "脆弱性一覧とCVSS要約"
  - "修正PR（最小差分）"
  - "再発防止チェックリスト"
artifacts:
  - "artifacts/sec-audit.md"
  - "artifacts/patches/*.diff"
```

---

## 3) Meta-Prompt 連携例（最小）
`prompts/meta-prompt.md`（前回ファイル）を**常に最初の指示**として読み込み、その上で `mode` と `constraints` を付与。

```yaml
objective: "Rustのプロセス分離（2023-2025）を比較し、推奨構成を示す"
mode: "research"
constraints:
  budget_tokens: 60000
  sla_minutes: 90
  citations: "required"
  lightweight_fallback: true
tools:
  mcp: ["search","crawler","pdf_reader"]
```

---

## 4) CLI テンプレ（スクリプト付）

### 4.1 研究実行
```bash
# scripts/run_research.sh
set -euo pipefail
topic="${1:-"Rustのプロセス分離 2023-2025比較"}"
codex research "$topic"   --depth 3 --breadth 8 --budget 60000 --citations required   --mcp search,crawler,pdf_reader --lightweight-fallback   --out artifacts/report.md
```

### 4.2 委任実行
```bash
# scripts/run_delegate.sh
set -euo pipefail
agent="${1:-"test-gen"}"
codex delegate "$agent" --scope ./src --deadline 2h --budget 40000   --out artifacts/delegate-${agent}.md
```

---

## 5) 成果物（Artifacts）スキーマ例
```yaml
report.md:
  sections: ["Summary","Findings","Contradictions","Citations"]
  quality:
    diversity_score: float
    contradiction_count: int
    confidence: "low|medium|high"
prs:
  - title: "feat: tighten types in module X"
    risk: "low"
    tests_added: true
    ci_status: "passing"
```

---

## 6) 受け入れ基準（チェックリスト）
- [ ] **複数ドメイン**の出典を提示（URL/引用）。
- [ ] **矛盾検知**のログを artifacts に保存。
- [ ] **予算・SLA** を満たし、必要なら軽量版フォールバック。
- [ ] サブエージェントは **独立コンテキスト**・**権限最小** で動作。
- [ ] 生成PRは **最小差分**＋**テスト**＋**CI緑**。

---

## 7) セキュリティ & 運用
- **権限最小化**：`policies/net.allowlist` と `mcp.allowlist` を必須化、シークレットは `.env` でスコープ限定。  
- **監査証跡**：計画・探索ログ・決定理由・差分パッチを artifacts に永続化。  
- **失敗時**：指数バックオフ→代替検索→軽量版→人間レビュー。

---

## 8) 参考リンク（貼り付け欄）
- MCP 仕様/実装ガイド  
  - https://modelcontextprotocol.io/specification/latest  
  - https://openai.github.io/openai-agents-js/guides/mcp/  
  - https://platform.openai.com/docs/mcp
- Deep Research（計画型探索・軽量版）  
  - https://openai.com/index/introducing-deep-research/  
  - https://openai.com/index/deep-research/  
- Claude Code / Subagents  
  - https://docs.claude.com/en/docs/claude-code/sub-agents

---

## 9) 署名
- Kit Author: zapabob / Ryo Minegishi
- Version: 0.1.0
- License: MIT (テンプレ群)
