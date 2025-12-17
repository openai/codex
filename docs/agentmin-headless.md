# Agentmin × Codex：`codex exec` 无头执行与交付报告

这份文档面向 “Agentmin 工单” 的最小闭环交付：用 `codex exec` 在本地无头跑完任务、输出可复核的验证步骤与风险说明，并把项目状态（`PROJECT.md` 的 `Why/Status/Next`）交给中央 `portfolio` 扫描上收。

## 1) 无头执行（headless）推荐姿势

### 最小可用：只读、零交互

```sh
codex exec --ask-for-approval never --sandbox read-only "Explain this repo"
```

适用：只需要阅读/分析，不需要改文件、不需要跑会写盘的命令。

### 交付模式：允许写入仓库（仍然不需要交互）

```sh
codex exec \
  --ask-for-approval never \
  --sandbox workspace-write \
  "按 work_orders/<WO>.md 完成最小闭环交付：更新 PROJECT.md / 新增文档 / 输出验证步骤与 git status"
```

适用：需要在当前仓库写文件（例如补齐 `PROJECT.md`、新增 docs）。

### 跨仓库联动：同时写入 Agentmin（写 report / 跑 scan 产物落盘）

如果你的任务需要把报告写到 `Agentmin` 仓库（或让 scan 产物写到 `Agentmin/portfolio/`），用 `--add-dir` 把 Agentmin 作为额外可写根目录暴露给 Codex：

```sh
codex exec \
  --ask-for-approval never \
  --sandbox workspace-write \
  --add-dir /Users/galaxy/Project/Agentmin \
  "完成工单并把最终报告写到 /Users/galaxy/Project/Agentmin/work_orders/reports/<WO>.report.md"
```

说明：
- `--sandbox workspace-write`：允许在工作区写文件；仍然受限于 sandbox（例如默认不允许网络）。
- `--ask-for-approval never`：无头执行，不弹确认。
- `--add-dir`：把额外目录加入可写根，方便跨仓库交付。

## 2) 输出模式：把结果“落盘”给其他工具消费

### 只保存最终总结（推荐用于 report 初稿）

```sh
codex exec -o /tmp/codex-last-message.md \
  --ask-for-approval never \
  --sandbox workspace-write \
  "完成任务并用要点总结 What changed / How to verify / Risk / Next"
```

`-o/--output-last-message` 只保存最后一条 assistant 消息，便于复制进最终 `.report.md`。

### JSONL 事件流（需要更强可观测性时）

```sh
codex exec --json \
  --ask-for-approval never \
  --sandbox workspace-write \
  "..." \
  > /tmp/codex-events.jsonl
```

`--json` 会把事件流输出到 stdout（JSON Lines），便于后续用脚本聚合“跑过哪些命令 / 改了哪些文件”等信息。

## 3) Report 怎么写（Agentmin 交付格式）

Agentmin 的工单报告文件路径约定：

`/Users/galaxy/Project/Agentmin/work_orders/reports/<WO>.report.md`

报告必须包含以下 4 段（标题可一致即可）：

```md
## What changed
- ...

## How to verify
- ...

## Risk or Blocked
- ...

## Next
- ...
```

推荐在 `How to verify` 里写“可直接复制执行”的命令（离线可跑），并在报告末尾附上：
- `git status --porcelain=v1`（至少目标仓库；若改了 Agentmin，也贴 Agentmin 的）
- 本次跑过的关键命令清单（scan / tests / lint / 生成物等）

## 4) 中央扫描上收：让 `Why/Status/Next` 在 portfolio 可见

完成目标仓库的 `PROJECT.md` 更新后，跑一次中央扫描（注意：在 `Agentmin` 仓库目录下执行，让输出落到 `Agentmin/portfolio/`）：

```sh
python3 /Users/galaxy/Project/Agentmin/scripts/portfolio/scan_repos.py \
  --roots /Users/galaxy/Project \
  --exclude /Users/galaxy/Project/IntelligentLakeCompany \
  --max-depth 6
```

然后检查 `Agentmin/portfolio/portfolio.md` 中目标仓库（例如 `Galaxy-0/codex`）这一行的 `Why/Status/Next` 是否已更新。
