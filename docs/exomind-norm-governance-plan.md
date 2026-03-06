# Exomind Codex 规范全量内化与自治治理体系（想法一实施方案）

## 1. 文档目的

将团队既有编码规范（风格、架构、安全）转化为 Codex CLI 可执行、可追踪、可演进的治理系统，形成“规则定义 -> 实时执行 -> 审查拦截 -> 规则迭代”的闭环。

## 2. 成功标准（Definition of Success）

- 规则执行覆盖率 >= 90%（核心语言与核心仓库）。
- 高优先级规则（架构/安全）漏检率 <= 2%。
- PR 首轮自动审查通过率提升 >= 30%。
- 规则建议采纳周期 <= 14 天（从发现到落地）。

## 3. 范围与边界

### In Scope

- 规范数字化吸收（文档/现有 lint 规则/代码评审惯例）。
- 本地实时校验与自动修复（Agent 生成与编辑时）。
- CI/CD Gate（PR 阶段强制规则检查）。
- 规则冲突检测与优化建议流程。

### Out of Scope（首期）

- 完全替代人工架构评审。
- 对历史全部仓库一次性零人工迁移。
- 自动批准 PR（只做建议和阻断，不直接 merge）。

## 4. 总体架构

1. 规则源层（Norm Sources）

- 输入：风格指南、架构文档、安全基线、历史 PR 评审语料。
- 输出：统一规则清单（Rule Catalog）。

2. 规则编译层（Norm Compiler）

- 将自然语言规范编译为可执行规则对象：
  - `metadata`：id、owner、severity、scope、version
  - `matcher`：AST 模式/正则/语义条件
  - `action`：warn、block、autofix、refactor_hint
  - `evidence`：命中文本、文件、行号、上下文

3. 执行层（Policy Runtime）

- 微观：Codex CLI 本地会话中对每次代码改动实时判定。
- 宏观：CI Pipeline 中进行批量扫描与阻断。

4. 治理层（Governance Loop）

- 规则冲突检测（重复、互斥、优先级冲突）。
- 盲区发现（漏检样本聚类）。
- 规则优化建议（proposal -> review -> approve -> release）。

## 5. 规则优先级模型

采用“宪法级 -> 法律级 -> 约定级”三级权重：

- L1 宪法级（Architecture/Security）：最高优先，冲突时必胜，默认 `block`。
- L2 法律级（Domain Patterns/Testing）：默认 `warn | block`，允许豁免流程。
- L3 约定级（Style/Formatting）：默认 `warn | autofix`。

冲突决策策略：

1. 按 level 决策（L1 > L2 > L3）。
2. 同级按 `severity`（critical > high > medium > low）。
3. 仍冲突则按最新 `version` 与 owner 仲裁。

## 6. Codex CLI 集成设计

### 6.1 本地编辑会话（微观）

- Hook 点：生成代码后、写文件前、执行命令前。
- 动作：
  - `block`：阻止落盘并给出可执行修复建议。
  - `autofix`：自动重写并展示 diff。
  - `warn`：记录告警并允许继续。

### 6.2 CI / PR 审查（宏观）

- PR 触发统一规则检查作业：
  - 失败条件：任一 L1 违规未豁免。
  - 报告产出：按文件/规则/责任域聚合。
- 与 GitHub 检查项集成：
  - `norm-governance/check`（required）
  - `norm-governance/report`（artifact）

## 7. 规则自演进闭环

1. 采样：收集误报、漏报、人工复审意见。
2. 归因：将问题映射到规则缺失/规则冲突/上下文不足。
3. 建议：自动生成规则变更提案（含影响面评估）。
4. 审核：规则 owner + 架构委员会审批。
5. 发布：灰度发布（10% repo -> 50% -> 全量）。

## 8. 数据模型与追踪字段（最小集）

- `rule_id`, `rule_level`, `severity`, `scope`, `owner`, `version`
- `hit_count`, `block_count`, `autofix_count`, `false_positive_rate`
- `repo`, `branch`, `pr_number`, `commit_sha`, `file_path`, `line`
- `decision`, `waiver_id`, `waiver_expiry`

## 9. 分阶段路线图

### Phase 0（1-2 周）

- 完成规则目录模板与优先级模型。
- 接入 1 个语言栈的基础规则（如 Rust/TS）。

### Phase 1（2-4 周）

- 在 Codex CLI 本地会话实现实时校验 MVP。
- CI 中启用只告警模式（不阻断）。

### Phase 2（2-4 周）

- 启用 L1 阻断，L2/L3 告警+自动修复。
- 建立误报反馈与豁免审批流程。

### Phase 3（持续）

- 上线规则建议自动生成与冲突检测。
- 增加跨仓库治理看板与趋势指标。

## 10. 风险与缓解

- 误报过高导致团队反感：先 warn 后 block，严格灰度。
- 规则过多导致性能下降：增量扫描 + 缓存 + 并行执行。
- 架构规则难以形式化：保留人工 override 与“建议模式”。

## 11. 与想法二的衔接

- 想法二（RLPH）提供“自动读取上下文/执行命令/队列调度”能力。
- 想法一可作为 RLPH 的策略层输入：在队列执行前后进行规范判定，实现自动化流程中的治理内嵌。

## 12. 交付物清单

- 规则目录规范（Rule Catalog Spec）。
- 本地实时校验引擎（MVP）。
- CI 审查 Gate 与报告模板。
- 规则演进提案模板与审批流程。

## 13. 里程碑交付快照（2026-03-06）

对应 issue：`#6` `#7` `#8` `#9`

- `#7` Rule Catalog 模板与样例
  - `docs/exomind-rule-catalog-template.json`
  - `docs/exomind-rule-catalog-spec.md`
- `#9` 冲突裁决接口草案
  - `docs/exomind-rule-conflict-resolution-interface.md`
- `#6` CI Warn 模式入口与基础报告
  - `scripts/exomind_norm_governance_warn.py`
  - `.github/workflows/exomind-norm-governance-warn.yml`
- `#8` 误报/漏报反馈闭环模板
  - `docs/exomind-norm-feedback-template.md`
  - `.github/ISSUE_TEMPLATE/7-norm-rule-feedback.yml`

## 14. 后续里程碑（已建 issue）

- `#11` 本地会话治理 Hook MVP（生成后/写入前/命令前）。
- `#12` CI Block 模式与 Waiver 机制。

## 15. M5/M6 落地快照（2026-03-06）

- `#11` 本地会话治理 Hook MVP

  - `codex-rs/hooks/src/types.rs` 新增 `before_tool_use` 事件模型。
  - `codex-rs/core/src/tools/registry.rs` 在工具执行前接入 `before_tool_use` 分发。
  - `codex-rs/hooks/src/norm_governance.rs` 提供治理规则判定与统一 evidence 输出。
  - `docs/exomind-governance-runtime-mvp.md` 记录启用方式与决策模型。

- `#12` CI Block + Waiver
  - `scripts/exomind_norm_governance_warn.py` 支持 `--mode warn|block` 与 `--waivers`。
  - `.github/workflows/exomind-norm-governance-warn.yml` 接入可配置模式与 waiver 文件。
  - `docs/exomind-norm-waivers.json` 初始化 waiver 存储。
  - `docs/exomind-norm-waiver-spec.md` 定义 waiver 结构与失效语义。
