# Codex CLI (Galaxy-0/codex)

## Why
Codex CLI 本体项目：用于本地/团队的 agent 开发与执行；同时作为 Rust 学习与持续贡献的长期底座。

## Status
Done：WO-codex-triage-1 最小闭环已交付（PROJECT.md + Agentmin 无头/联动文档 + portfolio scan 上收 + report）。

## Next
30–90min：新增一个本仓库脚本/`just` 命令，一键调用 Agentmin 的 `scan_repos.py`（带默认 exclude/max-depth 参数），避免每次手写长路径并减少参数漂移。

## Stage (optional)
Maintain

## Staffing (optional)
- Owner: galaxy
- Primary agent: codex
- Review agent: claudecode (optional)

## Log (optional)
- 2025-12-17: 交付 WO-codex-triage-1：补齐 PROJECT.md、添加 docs/agentmin-headless.md、跑 Agentmin portfolio scan、写 report
- 2025-12-16: 修正 Next 引用（避免 shell 反引号）
- 2025-12-16: 联动实验：写入 PROJECT.md 以支持中央扫描上收
- YYYY-MM-DD: <milestone / decision / shipped>
