# 贡献指南（Contributing）

> 本文是 `docs/contributing.md` 的中文概览版本，仅概述流程与注意事项。详细要求请以英文原文为准。

欢迎对 Codex 提交 Issue 和 Pull Request！

## 贡献前的准备

- 先浏览：
  - `README.md`：了解项目用途和基本使用。
  - `docs/install.md`：本地构建与运行方法。
  - `docs/architecture.md`：整体架构和模块划分。
- 查看现有 Issue，避免重复。
- 如有较大改动建议，先在 Issue 或讨论区确认设计方向。

## 本地开发

常见步骤（以 Rust CLI 为例）：

- 克隆仓库并进入目录。
- 按 `docs/install.md` 中的说明安装依赖（Rust 工具链、just、pnpm 等）。
- 对 Rust 代码：
  - 运行 `just fmt` 保持格式一致。
  - 使用 `just fix -p <crate>` 解决 Clippy 问题。
  - 运行相关的 `cargo test`，在改动核心/共享 crate 时考虑跑全量测试。

## 提交修改

- 保持 PR 专注：一次只解决一个问题或一类改动。
- 尽量附上：
  - 变更动机和背景。
  - 效果截图（如 UI 变化）。
  - 测试说明：运行了哪些测试，是否有待补充的用例。
- 对公共 API 变更或用户可见行为变更：
  - 更新 `docs/` 中相关文档。
  - 必要时在 `CHANGELOG.md` 中添加条目。

## 行为准则与社区

- 遵守项目的行为准则（Code of Conduct）。
- 保持沟通友好、专业。
- 对 review 意见积极反馈，可以提出不同看法但保持尊重。

更多细节请参考英文原文 `docs/contributing.md`。

