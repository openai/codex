# 入门指南（Getting Started）

> 本文是 `docs/getting-started.md` 的中文版本，内容略有压缩，示例与行为以英文原文为准。

## 快速导航

- [命令行用法](#命令行用法)
- [恢复会话](#恢复会话)
- [以 prompt 作为输入运行](#以-prompt-作为输入运行)
- [示例提示词](#示例提示词)
- [使用 AGENTS.md 作为记忆](#使用-agentsmd-作为记忆)
- [技巧与快捷操作](#技巧与快捷操作)

## 命令行用法

| 命令               | 作用                                  | 示例                             |
| ------------------ | ------------------------------------- | -------------------------------- |
| `codex`            | 交互式 TUI                           | `codex`                          |
| `codex "..."`      | 使用初始 prompt 启动交互式 TUI       | `codex "fix lint errors"`        |
| `codex exec "..."` | 非交互“自动化模式”                    | `codex exec "explain utils.ts"`  |

常用参数：`--model / -m`，`--ask-for-approval / -a`。

## 恢复会话

- 打开会话选择器：

```bash
codex resume
```

- 恢复最近一次会话：

```bash
codex resume --last
```

- 通过 id 恢复：

```bash
codex resume <SESSION_ID>
```

会话 id 可以从 `/status` 或 `~/.codex/sessions/` 中获得。

## 以 prompt 作为输入运行

你也可以直接用 prompt 启动 Codex CLI：

```bash
codex "explain this codebase to me"
```

适合快速发起一次任务，然后在 TUI 中继续迭代。

## 示例提示词

以下是一些可以直接复制的小例子（把引号中的内容替换成你的任务）：

| ✨  | 你输入的命令                                                            | Codex 做什么                                                           |
| --- | ----------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| 1   | `codex "Refactor the Dashboard component to React Hooks"`              | 将类组件改为 Hooks，运行 `npm test`，展示 diff。                       |
| 2   | `codex "Generate SQL migrations for adding a users table"`             | 推断 ORM，创建迁移文件，并在沙箱数据库中运行。                         |
| 3   | `codex "Write unit tests for utils/date.ts"`                           | 生成测试、运行测试、反复修正直到通过。                                 |
| 4   | `codex "Bulk-rename *.jpeg -> *.jpg with git mv"`                      | 安全批量重命名并更新引用。                                            |
| 5   | `codex "Explain what this regex does: ^(?=.*[A-Z]).{8,}$"`             | 给出分步骤的人类可读解释。                                             |
| 6   | `codex "Carefully review this repo, and propose 3 high impact PRs"`    | 结合仓库结构给出高价值改动建议。                                      |
| 7   | `codex "Look for vulnerabilities and create a security review report"` | 扫描并解释潜在安全问题。                                               |

想要复用自己的提示词？可以使用 [自定义 prompts 和 Slash Commands](./prompts.md)。

## 使用 AGENTS.md 作为记忆

Codex 会自动读取 `AGENTS.md` 文件来获取“额外说明”和“项目偏好”。查找和合并规则见：

- 全局：`~/.codex/AGENTS.md`
- 仓库根目录到当前目录路径上的各级 `AGENTS.md` 或 `AGENTS.override.md`

这相当于给 Codex 提供了一层“项目级系统提示词”，适合写：

- 代码风格/约定、依赖管理策略。
- 哪些目录是第三方代码不应修改。
- 业务领域背景说明等。

更多细节见 `docs/agents_md.md`。

## 技巧与快捷操作

### 使用 `@` 进行文件搜索

- 在输入框中键入 `@` 会触发工作区内的模糊文件名搜索。
- 使用上下方向键选择结果，按 Tab 或 Enter 插入路径，Esc 取消搜索。

### Esc–Esc 编辑上一条消息

- 当输入框为空时，按 Esc 进入“回溯（backtrack）”模式。
- 再次按 Esc 会出现最近用户消息的预览，高亮当前选中的那条。
- 多次按 Esc 可以向更早的用户消息移动，回车确认后：
  - 会话从该点分叉；
  - 旧的后续消息会被折叠；
  - 输入框预填该条消息内容，方便你修改后重新发送。

### `--cd` / `-C` 参数

不方便先 `cd` 到目标目录？可以使用：

```bash
codex --cd path/to/project
```

TUI 顶部会显示当前使用的 **workdir**，可用来确认。

### `--add-dir` 参数

需要一次跨多个项目工作？可以通过 `--add-dir` 添加额外可写目录：

```bash
codex --cd apps/frontend --add-dir ../backend --add-dir ../shared
```

Codex 可以同时读取并修改这些目录中的文件。

### Shell 补全

生成 shell 补全脚本：

```bash
codex completion bash
codex completion zsh
codex completion fish
```

### 图像输入

- 直接在 TUI 输入框粘贴图片（Ctrl+V / Cmd+V）。
- 或通过 CLI 参数绑定图片：

```bash
codex -i screenshot.png "Explain this error"
codex --image img1.png,img2.jpg "Summarize these diagrams"
```

