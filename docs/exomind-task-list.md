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

## B. 想法一（规范治理体系文档）
- [x] 产出系统化实施文档：`docs/exomind-norm-governance-plan.md`
- [x] 建立跟踪 issue：`#1`
- [x] 在 issue 内维护 checklist 与上下文同步约定
- [-] 在 PR 中持续同步文档版本与后续里程碑

## C. 想法二（RLPH 单队列持续重复机制）
- [x] 需求语义化同步到 issue body：共享队列、轮转、颜色区分、Alt+Up 复用
- [x] 输入通道：`Ctrl+Tab` / `Shift+Tab` 进入 BufferedQueue
- [x] 单队列复用：持续消息与普通队列消息共用 `queued_user_messages`
- [x] 轮转机制：持续消息派发后回到队尾
- [x] 支持持续重复 `queue` + 持续重复 `steer`（通过 `steer_mode`）
- [x] 预览 UI 四态区分：普通/持续 × queue/steer
- [x] 复用 `Alt+Up` 编辑（共享队列天然生效）
- [x] 补齐受影响测试构造与匹配分支（类型层面）

## D. 编译检查与质量验证
- [x] `cargo check -p codex-tui` 通过（代码主线）
- [x] 每轮改动后执行编译检查并中途修复编译错误
- [x] `cargo test -p codex-tui --lib --no-run` 通过（切换 `CARGO_TARGET_DIR=G:\codex-rs-target`）
- [x] 运行目标测试集并处理 snapshot 变更（`pending_input_preview::tests`）

## E. 提交与汇报
- [x] 提交 commit（关联 `#1` `#2` `#3`）
- [x] push 分支
- [x] 创建 PR（在描述中写明最新需求与已完成项）
- [x] 在 issue `#2` / `#3` 追加阶段总结与下一步计划
- [-] PR 审阅/合并与 issue 收尾关闭