# 为本 Fork 贡献

语言： [English](./CONTRIBUTING.md) | [中文（简体）](./CONTRIBUTING.zh-CN.md)

本文档描述了如何在尽量贴近上游（https://github.com/openai/codex） 的前提下，开发与发布本 fork，并保持与上游清晰区分。

目标：上游版本可复现（基于 tag），fork 版本可追溯（-fork.N），流程简洁且适合团队协作。

## 分支模型

- `main`：本 fork 的主分支；以上游发布的 tag 为基线，叠加 fork 的改动。
- `sync/...`：通过同步上游生成的临时分支；以 PR 合并回 `main`。

合并策略：默认采用 merge，更简单、稳定；如需更干净的历史可选择 rebase（在共享分支请谨慎）。

## 版本与标签

- 版本号：在上游版本后追加 `-fork.N`（示例：`0.21.0-fork.1`）。
- 标签：发布由 `rust-v<version>` 触发，标签需与 `codex-rs/Cargo.toml` 完全一致（示例：`rust-v0.21.0-fork.1`）。

为什么这样做？

- 一眼可见“基于哪个上游版本 + 第几次 fork 增量”；
- GitHub Actions 的发布流水线以 `rust-v*` 标签触发，天然兼容。

## 跟进上游（两种方式）

1) 本地脚本（手动）

使用 `scripts/sync_upstream.sh`（需要工作区干净）：

- 列出上游最新标签：

```
scripts/sync_upstream.sh list-tags --limit 10
```

- 合并上游 main：

```
scripts/sync_upstream.sh merge-main --branch main --push
```

- 合并指定 tag：

```
scripts/sync_upstream.sh merge-tag rust-v0.21.0 --branch main --push
```

选项：`--rebase` 使用变基；`--no-branch` 直接在目标分支操作；`--upstream-url` 覆盖默认上游；`--push` 推送产生的分支到 origin。
再补充：`--dry-run` 可进行“预览模式”，打印将要创建/切换的分支、要进行的合并/变基、基线 tag、推送动作等，
但不会对本地仓库做任何改动。dry-run 会进行一次轻量网络连通性探测，若检测到网络受限会明确提示，之后使用本地已有的 tag 视图（不执行 fetch）。
如需拉取最新标签，请去掉 `--dry-run` 运行；若 fetch 失败，脚本会提示并继续使用本地视图。

另有：`--force-tags` 用于在抓取上游时强制更新本地 tag 与上游对齐（等价于 `git fetch upstream --tags --prune --force`）。
仅在你明确希望覆盖本地与上游“同名但指向不同”的标签时使用。

基线标记（推荐）：

- 在合并上游标签（如 `rust-v0.21.0`）时，可加 `--tag-baseline`，在合并结果上创建
  `base-rust-v0.21.0` 这样的“基线 tag”，方便追踪当前代码所基于的上游版本。
- 搭配 `--push-tags` 可将该基线 tag 推送到 origin。
- 注意：基线 tag 的前缀为 `base-`，不会触发发布工作流（发布触发仅匹配 `rust-v*`）。

查询当前基线：

```
scripts/sync_upstream.sh current-baseline
```
优先输出最近的基线 tag（如 `base-rust-v0.21.0`）；若不存在，则回退到日志中最近一次
“Merge upstream rust-v...” 的提交来推断。

自动连续合并上游标签：

```
scripts/sync_upstream.sh merge-series --branch main --push --push-tags --limit 10
```

行为说明：

- 自动识别当前基线（`base-rust-v*`），依次合并其后的上游 `rust-v*` 标签（默认仅稳定版本）；每合并
  成功一个标签，会在结果上打一个新的基线 tag，便于中断后续接续。
- 若合并出现冲突，脚本会在该标签处停止；请手动解决冲突并提交，然后再次运行同一命令，脚本会
  从下一个标签继续。
- 当未检测到基线时，可用 `--from <rust-vX.Y.Z>` 指定起始基线；也可用 `--to <rust-vX.Y.Z>` 指定
  终止的上游标签。
 - 可加 `--dry-run` 先预览完整的合并计划，确保无误后再正式执行。
 - 标签选择：
   - 默认仅合并稳定版本（匹配 `rust-vX.Y.Z`）
   - `--include-pre`：包含预发布标签（如 `-alpha/-beta/-rc`）
   - `--pre-only`：仅包含预发布标签

在全新 fork（还没有任何基线 tag）时初始化一次基线：

```
scripts/sync_upstream.sh init-baseline rust-v0.21.0 --push-tags
```

该命令会在当前 `HEAD` 上创建 `base-rust-v0.21.0`，用来标记“本地当前代码基于上游的 rust-v0.21.0”。
之后再运行 `merge-series` 时即可自动识别出这个基线并从其后的标签开始连续合并。

2) GitHub Action（自动/手动）

工作流：`.github/workflows/upstream-sync.yml`

- 定时（每日）：选择上游最新 `rust-v*` 标签，创建 `sync/...` 分支并自动开 PR 到 `main`。
- 手动触发：在 Actions 中选择 `upstream-sync`，可指定：
  - `upstream_repo`（可选，默认 `openai/codex`）
  - `ref`（可选；如 `upstream/main` 或 `rust-v0.21.0`）
  - `mode`（`merge` 或 `rebase`，默认 `merge`）

## 发布 fork 版本

方式一（推荐，一键）：

```
scripts/release_fork.sh --dry-run               # 预览：基线与即将发布的版本号
scripts/release_fork.sh                         # 从 main 发布；内部调用 codex-rs/scripts/create_github_release.sh
scripts/release_fork.sh --baseline 0.30.0       # 覆盖基线（可选）
scripts/release_fork.sh --version 0.30.0-fork.2 # 完全显式指定版本
```

方式二（手动等价流程）：

- 确认 `main` 已合入目标上游 tag 或 `upstream/main`，并完成冲突解决与测试。
- 更新 `codex-rs/Cargo.toml` 的版本为 `X.Y.Z-fork.N`。
- 打标签并推送：`rust-vX.Y.Z-fork.N`。

随后会自动触发 `rust-release` 工作流：

- 校验标签格式并比对 `Cargo.toml` 版本；
- 多平台构建产物并上传；
- 创建 GitHub Release（`-fork.*` 视为正式版本；仅 `-alpha/-beta/-rc` 标记为 Pre-release）；
- 生成 npm 打包产物（如需发布到 npm，请手动执行脚本并确保权限）。

可选：发布到 npm（需要权限）：

```
VERSION=0.21.0-fork.1
# 本 fork（默认仓库：yoyooyooo/codex）：
./scripts/publish_to_npm.py "$VERSION"
# 若需从上游 Release 获取：
./scripts/publish_to_npm.py "$VERSION" --repo openai/codex
```

## 编码、CI 与文档约定

- 尽量保持与上游一致，避免无关重构；
- 遵循上游构建与测试说明（见 `docs/`）；
- 根目录 `README.md` 要求 ASCII-only（CI 检查），中文内容请放在 `README.zh-CN.md`；
- 如果你要在 README 中添加 ToC，请使用 `<!-- Begin ToC -->`/`<!-- End ToC -->` 标记，便于 CI 校验。

## 问题反馈

如有疑问或建议，欢迎在本 fork 提 Issue 或直接提交 PR 改进本文档与脚本。
