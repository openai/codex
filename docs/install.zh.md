# 安装与构建（Install）

> 本文是 `docs/install.md` 的中文概览版本。不同平台的详细步骤与系统要求以英文文档为准。

## 快速安装

推荐的安装方式有两种：

### 通过 npm 安装

```bash
npm install -g @openai/codex
codex
```

### 通过 Homebrew 安装（macOS）

```bash
brew install --cask codex
codex
```

## 系统要求

具体的操作系统版本、终端要求、依赖等，请参考英文文档：

- `docs/install.md` 中的 “System Requirements” 章节。

一般建议：

- 使用较新版本的 macOS / Linux。
- 确保终端支持 24-bit 颜色和较好的 UTF-8 渲染。

## 从源码构建

如果你想本地构建 Rust 版本的 CLI：

1. 安装 Rust 工具链（例如通过 `rustup`）。
2. 克隆仓库：

```bash
git clone https://github.com/openai/codex.git
cd codex
```

3. 在 `codex-rs` 目录下使用 `cargo` 构建：

```bash
cd codex-rs
cargo build --release
```

> 实际构建命令和辅助工具（如 `just`、`nix`）的用法，请以 `docs/install.md` 为准。

