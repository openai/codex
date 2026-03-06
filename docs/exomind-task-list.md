# Exomind Codex 多步任务清单（持续跟踪）

## 状态规则
- `[ ]` 未开始
- `[-]` 进行中
- `[x]` 已完成

## A. 仓库与协作流程
- [x] Fork `openai/codex` 到 `exomind-team/codex`
- [x] 本地克隆并配置 `origin` / `upstream`
- [x] 建立分支 `feat/rlph-protocol-bootstrap`
- [x] 启用 GitHub Issues 并建立跟踪 issue：`#1` `#2` `#3`
- [x] 完成首个实现 PR 合并：`#4`

## B. 想法一（规范治理体系文档）
- [x] 产出系统化实施文档：`docs/exomind-norm-governance-plan.md`
- [x] 建立跟踪 issue：`#1`
- [x] 在 issue 内维护 checklist 与上下文同步约定
- [x] 在 PR 中同步文档版本（`#4` 已合并）
- [-] 持续迭代后续里程碑（Rule Catalog / CI Gate / 演进闭环）

## C. 想法二（RLPH 单队列持续重复机制）
- [x] 需求语义化同步到 issue body：共享队列、轮转、颜色区分、Alt+Up 复用
- [x] 输入通道：`Ctrl+Tab` / `Shift+Tab` 进入 BufferedQueue
- [x] 单队列复用：持续消息与普通队列消息共用 `queued_user_messages`
- [x] 轮转机制：持续消息派发后回到队尾
- [x] 支持持续重复 `queue` + 持续重复 `steer`（通过 `steer_mode`）
- [x] 预览 UI 四态区分：普通/持续 × queue/steer
- [x] 复用 `Alt+Up` 编辑（共享队列天然生效）
- [x] 补齐受影响测试构造与匹配分支（类型层面）
- [x] Epic issue 收尾关闭：`#2`

## D. 编译检查与质量验证
- [x] `cargo check -p codex-tui` 通过（代码主线）
- [x] 每轮改动后执行编译检查并中途修复编译错误
- [x] `cargo test -p codex-tui --lib --no-run` 通过（切换 `CARGO_TARGET_DIR=G:\codex-rs-target`）
- [x] 运行目标测试集并处理 snapshot 变更（`pending_input_preview::tests`）
- [x] 全局 Cargo 构建目录统一为 `G:/cargo-target`（`%USERPROFILE%/.cargo/config.toml`）
- [x] 无临时环境变量验证：`cargo check -p codex-hooks -v` 输出路径为 `G:/cargo-target`

## E. 提交与汇报
- [x] 提交 commit（关联 `#1` `#2` `#3`）
- [x] push 分支
- [x] 创建 PR（在描述中写明最新需求与已完成项）
- [x] 在 issue `#2` / `#3` 追加阶段总结与下一步计划
- [x] PR 审阅/合并（`#4`）与 issue `#2` 收尾关闭

## F. 持续跟踪机制（执行中）
- [-] 需求变更先更新 GitHub issue/PR 描述，再继续开发
- [-] 每轮任务结束执行编译检查，并把结果同步到 issue `#3`
- [-] 定期将最新需求、任务计划、风险与决策语义化存储到 issue `#3`
- [-] 想法一的长期治理任务持续拆解并通过 issue/PR 追踪至完成
- [-] 若出现 API 错误：记录报错与重试结果，不中断任务，优先继续本地可执行项并同步到 issue `#3`

## G. 下一阶段（已完成）
- [x] 设计并提交 Rule Catalog 模板（含 L1/L2/L3 示例，Issue `#7`）
- [x] 定义规则冲突裁决接口草案（映射到 Codex CLI 执行点，Issue `#9`）
- [x] 建立 CI “warn 模式”入口并输出基础报告格式（Issue `#6`）
- [x] 形成误报/漏报反馈闭环模板（issue/PR 模板或文档，Issue `#8`）

## H. 当前活跃 PR
- [x] 跟踪同步 PR：`#5`（任务清单/issue 状态同步，已合并）
- [x] 里程碑落地 PR：`#10`（#6/#7/#8/#9 交付物，已合并）

## I. Idea1 子任务实施（本轮）
- [x] `#7` 规则目录模板与样例：`docs/exomind-rule-catalog-template.json`
- [x] `#7` 规则目录规范文档：`docs/exomind-rule-catalog-spec.md`
- [x] `#9` 冲突裁决接口草案：`docs/exomind-rule-conflict-resolution-interface.md`
- [x] `#6` warn 模式脚本：`scripts/exomind_norm_governance_warn.py`
- [x] `#6` warn 模式 workflow：`.github/workflows/exomind-norm-governance-warn.yml`
- [x] `#8` 反馈流程模板文档：`docs/exomind-norm-feedback-template.md`
- [x] `#8` 反馈 issue 模板：`.github/ISSUE_TEMPLATE/7-norm-rule-feedback.yml`

## J. 下一里程碑（进行中）
- [x] `#11` 本地会话治理 Hook MVP（生成后/写入前/命令前）
- [x] `#12` CI Block 模式与 Waiver 机制

## K. M5/M6 交付物（本轮）
- [x] `#11` hook 事件扩展：`before_tool_use`（`codex-rs/hooks/src/types.rs`）
- [x] `#11` 工具执行前分发：`codex-rs/core/src/tools/registry.rs`
- [x] `#11` 治理判定引擎：`codex-rs/hooks/src/norm_governance.rs`
- [x] `#11` 运行说明：`docs/exomind-governance-runtime-mvp.md`
- [x] `#12` CI 模式升级：`scripts/exomind_norm_governance_warn.py`（warn/block + waivers）
- [x] `#12` CI 工作流接入：`.github/workflows/exomind-norm-governance-warn.yml`
- [x] `#12` waiver 文件与规范：`docs/exomind-norm-waivers.json` `docs/exomind-norm-waiver-spec.md`

## L. 运行环境基线（本轮）
- [x] 构建产物目录统一到 G 盘：`G:/cargo-target`
- [x] 同步更新 issue `#1` 与 `#3` 的需求快照和执行状态
