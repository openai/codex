# 发布管理（Release Management）

> 本文是 `docs/release_management.md` 的中文概览版本。具体流程和脚本以英文原文为准。

本仓库会通过一套半自动化流程来管理 Codex 的版本发布，包括：

- 版本号管理。
- 生成变更日志（Changelog）。
- 构建和发布多平台二进制。

## 版本与 Changelog

- 使用约定式提交或变更标签，辅助自动生成 `CHANGELOG.md`。
- 在发布前整理：
  - 新功能。
  - bug 修复。
  - 破坏性变更与迁移指南。

## 构建与发布

- 使用脚本（位于 `scripts/` 或 `codex-cli`/`codex-rs` 下）完成：
  - 多平台构建。
  - 打包归档。
  - 上传到 GitHub Releases 或其他分发渠道。

开发者如需本地模拟发布流程，请参考英文文档 `docs/release_management.md` 与脚本中的注释。

