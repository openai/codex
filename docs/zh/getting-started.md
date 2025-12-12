## Getting started

想找特定内容？可以直接跳转：

- [Tips & shortcuts](#tips--shortcuts) – 热键、恢复会话、提示词
- [Non-interactive runs](../exec.md) – 使用 `codex exec` 做自动化
- 需要更深度的自定义？参见 [`advanced.md`](../advanced.md)

### CLI usage

| 命令               | 用途                     | 示例                            |
| ------------------ | ------------------------ | ------------------------------- |
| `codex`            | 交互式 TUI               | `codex`                         |
| `codex "..."`      | 交互式 TUI 的初始提示词  | `codex "fix lint errors"`       |
| `codex exec "..."` | 非交互“自动化模式”       | `codex exec "explain utils.ts"` |

常用参数：`--model/-m`、`--ask-for-approval/-a`。

### Resuming interactive sessions

- 运行 `codex resume` 打开会话选择器 UI
- 恢复最近一次会话：`codex resume --last`
- 按 id 恢复：`codex resume <SESSION_ID>`（session id 可从 `/status` 或 `~/.codex/sessions/` 获取）
- 选择器会展示会话记录时的工作目录；若可用，还会显示当时的 Git 分支

示例：

```shell
# 打开最近会话的选择器
codex resume

# 恢复最近一次会话
codex resume --last

# 按 id 恢复指定会话
codex resume 7f9f9a2e-1b3c-4c7a-9b0e-123456789abc
```

### Running with a prompt as input

你也可以把提示词直接作为参数传给 Codex CLI：

```shell
codex "explain this codebase to me"
```

### Example prompts

下面是一些可以直接复制粘贴的小例子。把引号里的文本替换成你自己的任务即可。

| ✨  | 你输入什么                                                                     | 会发生什么                                                                 |
| --- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------------- |
| 1   | `codex "Refactor the Dashboard component to React Hooks"`                      | Codex 会把类组件改成 Hooks，运行 `npm test`，并展示 diff。                  |
| 2   | `codex "Generate SQL migrations for adding a users table"`                     | 推断你的 ORM，生成 migration 文件，并在沙箱 DB 里执行。                     |
| 3   | `codex "Write unit tests for utils/date.ts"`                                   | 生成测试、执行测试，并迭代到测试通过。                                     |
| 4   | `codex "Bulk-rename *.jpeg -> *.jpg with git mv"`                              | 安全地重命名文件，并更新 import/引用。                                     |
| 5   | `codex "Explain what this regex does: ^(?=.*[A-Z]).{8,}$"`                     | 输出一步步的人类可读解释。                                                 |
| 6   | `codex "Carefully review this repo, and propose 3 high impact well-scoped PRs"` | 在当前代码库里提出影响大且范围清晰的 PR 建议。                              |
| 7   | `codex "Look for vulnerabilities and create a security review report"`         | 查找并解释潜在的安全问题。                                                 |

如果你想复用自己的指令，可以用 [custom prompts](../prompts.md) 创建 slash commands。

### Memory with AGENTS.md

你可以用 `AGENTS.md` 文件给 Codex 提供额外的指令与项目约束。Codex 会按下面的位置查找，并自上而下合并：

1. `~/.codex/AGENTS.md` - 个人全局指引
2. 从仓库根目录到当前工作目录（包含当前目录）沿途的每一层目录：在每个目录里，Codex 会优先查找 `AGENTS.override.md`；如果不存在则回退到 `AGENTS.md`。当你希望“替换掉”继承下来的指令时使用 override 形式。

更多关于 AGENTS.md 的使用方式，请参阅 [官方 AGENTS.md 文档](https://agents.md/)。

### Tips & shortcuts

#### Use `@` for file search

输入 `@` 会触发对工作区根目录的模糊文件名搜索。用上下键选择候选项，用 Tab 或 Enter 把 `@` 替换成选中的路径；用 Esc 取消搜索。

#### Esc–Esc to edit a previous message

当聊天编辑框为空时，按一次 Esc 会进入“回退（backtrack）”准备状态。再按一次 Esc 会打开一段 transcript 预览，默认高亮最后一条用户消息；继续按 Esc 可以逐步回到更早的用户消息。按 Enter 确认后，Codex 会从选中的位置 fork 对话、相应裁剪可见 transcript，并把选中的用户消息预填回编辑框，方便你修改后再次提交。

在 transcript 预览中，footer 会显示 `Esc edit prev` 提示，表示回退编辑已启用。

#### `--cd`/`-C` flag

有时你不方便先 `cd` 到想让 Codex 使用的目录再运行。`codex` 支持 `--cd` 参数来指定工作根目录。你可以在新会话开始时，检查 TUI 显示的 **workdir** 是否符合预期，来确认 `--cd` 已生效。

#### `--add-dir` flag

需要一次运行同时跨多个项目工作？可以多次传入 `--add-dir`，把额外目录作为“可写根目录”暴露给当前会话，同时保持主工作目录不变。例如：

```shell
codex --cd apps/frontend --add-dir ../backend --add-dir ../shared
```

这样 Codex 就能在你列出的每个目录中查看和修改文件，而不需要离开主工作区。

#### Shell completions

生成 shell 自动补全脚本：

```shell
codex completion bash
codex completion zsh
codex completion fish
```

#### Image input

你可以把图片直接粘贴进编辑框（Ctrl+V / Cmd+V）作为附件。也可以通过 CLI 参数 `-i/--image`（逗号分隔）附加图片文件：

```bash
codex -i screenshot.png "Explain this error"
codex --image img1.png,img2.jpg "Summarize these diagrams"
```

#### Environment variables and executables

建议在启动 Codex 之前就把环境准备好，避免 Codex 为了探测环境而浪费 token。例如：提前激活 Python virtualenv（或其他语言运行时）、启动必要的守护进程、以及预先导出你需要使用的环境变量等。

