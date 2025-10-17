# Codex：Sub-Agents & Deep Research 拡張—要件定義書

**Status:** Draft · **Audience:** Codex core, cloud, CLI/IDE, integrations, and zapabob maintainers

## 0. 背景と目的
* OpenAI **Codex** はローカルCLIからIDE拡張、Web/クラウドまで横断する開発者向けエージェント。`AGENTS.md` などでプロジェクト方針を設定し、GitHub連携や @codex による PR レビューなど運用面も拡充している。[1]
* **Claude Code** は **Subagents** を標準機能として備え、**個別コンテキスト**・**ツール権限**・**専用システムプロンプト**で委任と並列化を実現する。Codex でも同等以上の一次機能を提供し、トークン予算を**動的配分**して上位互換を狙う。[2]
* **Deep Research** は計画立案→多段探索→出典付き要約まで自動化する公式リサーチ機能。軽量版やビジュアルブラウザ連携など最新拡張を取り込み、Codex の公式 Web 検索を**上位互換**として再設計する。[3]
* **MCP**（Model Context Protocol）を介して検索・スクレイピング・社内データ源・静的解析器などを安全に接続し、ツール権限を**サーバ側で管理**できるようにする。[9]

**目標**：
1. Codex に **サブエージェント機構**（トークン動的配分・権限境界・並列実行・PR 分割）を中核機能として組み込む。[2]
2. Codex の **Web 検索** を **Deep Research 準拠の計画型探索**へ拡張し、出典必須・反証プローブ・可観測性を備えたリサーチパイプラインを実現する。[3]
3. 既存 CLI / IDE / Web / GitHub 動線を壊さず **プラガブル増築**し、`openai/codex` と `zapabob/codex` の両系で保守できるようにする。[1]

---

## 1. スコープ

### In Scope

- Extend Codex core/orchestrator, CLI, IDE, web, Slack, and GitHub surfaces to expose Sub-Agent delegation and Deep Research without breaking existing entry points.[1]
- Provide a declarative configuration format for bundled and user-defined agents (`.codex/agents/*.md|*.yaml`).
- Integrate MCP tooling discovery so Deep Research can compose with sanctioned external data providers.[3][9]
- Ensure feature parity across upstream and forked deployments with environment-based capability toggles.

### Out of Scope

- Training or fine-tuning new foundation models; rely on existing Codex-accessible models and APIs.[3]
- Replacing existing workflow runners (e.g., seatbelt sandbox); instead, extend them with policy aware hooks.[7]
- Non-Codex products (e.g., ChatGPT UI) except where Codex already integrates via shared services.

### Related Work and References

- OpenAI Codex product updates and repo documentation.[1][4]
- Claude Subagents concepts and persona definitions.[2]
- OpenAI Deep Research specifications and policy constraints.[3][5]
- MCP community tooling (e.g., GPT Researcher MCP, LangChain Deep Research adapters).[9]

## Use Cases

1. **Large-scale library modernization**: main Codex agent plans work, delegates to specialized refactor/test agents, merges PRs, and reports in Slack.[6]
2. **Exploratory technology evaluation**: Deep Research generates plans, executes multi-hop searches, synthesizes comparative matrices, and cites sources.[3]
3. **Security triage and patching**: security Sub-Agent tracks CVEs, produces PoCs, proposes fixes, and validates in isolated sandboxes before raising PRs.[7]

## Functional Requirements

### 3.1 Sub-Agent Platform

- **Agent definitions**: load from `.codex/agents/*.md|*.yaml` with role, system prompt, capability list, tool policy, token budget, and concurrency limits; support hot reload for developer iteration.[2]
- **Context isolation**: each Sub-Agent owns a discrete conversation buffer, memory store, and telemetry stream. Token budgeting is per-agent with coordinator-managed rebalancing.
- **Capability boundaries**: enforce allowlists for file I/O, shell commands, network domains, credentials, and MCP tools. Main Codex evaluates delegation requests against agent policies before launch.[2]
- **Delegation API**: expose `codex.delegate(agent_id, goal, inputs, budget, deadline)` across SDK, CLI, IDE, Slack, and GitHub bots. Responses surface structured artifacts (diffs, reports, metrics) alongside logs.[8]
- **Parallel execution**: allow multiple Sub-Agents per workspace, isolate git worktrees or branches, reconcile changes via staging merges, and automatically queue CI before hand-off.[6]

### 3.2 Deep Research Engine

- **Plan synthesis**: given a topic, produce ranked sub-queries, evaluation criteria, and stop conditions. Plans must indicate required evidence depth and fallback paths.[3]
- **Iterative exploration**: orchestrate search, retrieval (web/PDF/imagery), extraction, contradiction checks, and summary with mandatory citations per finding.[3]
- **Tool chaining**: auto-discover MCP-compatible tools, register them with scoped credentials, and sandbox their execution; gracefully downgrade when tools are unavailable.[3][9]
- **Observability**: surface live trace logs, source reliability scoring, coverage metrics, and conflict detection in CLI/IDE dashboards.[10]
- **Lightweight fallback**: when resource quotas tighten, switch to a constrained Deep Research mode that maintains citations but shortens summaries and reduces breadth.[11]

### 3.3 Workflow Integration

- **Command surfaces**: add `codex research` and `codex delegate` commands (and IDE palette equivalents) with configurable budgets, depth, and deadlines.[6]
- **GitHub automation**: `@codex` comment triggers for research and delegation, with annotated PR reviews and report attachments.[6]
- **Slack/Notifications**: progress pings (plan, in-flight, synthesis) and completion summaries, respecting workspace notification policies.[1]

## Non-Functional Requirements

- **Security**: least-privilege policies per agent, sandboxed execution, and explicit handling of secrets for networked tools.[7]
- **Reliability**: retry/backoff for network failures, incremental state snapshots for long-running tasks, and resumable Deep Research sessions.
- **Scalability**: horizontal scaling for concurrent agents and research threads; dynamic token budgeting to prevent starvation.
- **Auditability**: persist delegation plans, prompts, intermediate outputs, citations, and merge decisions for compliance review.
- **Privacy**: enforce fences around proprietary repos and secrets; Deep Research must honor domain allowlists and redact sensitive data by default.[5]
- **User experience**: expose structured progress (plan -> execution -> synthesis) across CLI, IDE, and Slack, mirroring Codex's existing UX patterns.[6]

## Architecture Overview

- **Main Orchestrator (Codex Core)**: decomposes tasks, assigns Sub-Agents, coordinates token budgets, and aggregates outputs.
- **Sub-Agent Runtime**: spins up agent sandboxes with policy-scoped capabilities, manages context stores, and emits structured events.
- **Research Engine**: pipelines search, retrieval, extraction, critique, and synthesis; supports multiple connectors (browser, APIs, MCP tools).[3]
- **Sandbox Executor**: reuses Codex seatbelt/VM isolation to run builds, tests, and static analysis per agent.[7]
- **Integration Layer**: bridges GitHub PR bots, Slack notifiers, IDE panes, and web dashboards with consistent telemetry schemas.[6]
- **Evidence Store**: archives source URLs, snippets, screenshots, confidence scores, and decision rationale for downstream consumption.[10]

## External Interfaces

- **SDK (TypeScript/Node, Go planned)**: `delegate()`, `research()`, `plan()`, `submitPR()`, `measure()`, and `defineAgent()` for runtime agent creation.
- **CLI**: `codex research "<topic>" --depth <n> --budget <tokens> --citations required`, `codex delegate <agent> --scope <path> --deadline <duration>`.
- **IDE**: command palette entries (Research, Delegate, Compare Plans, Insert Citations) and side panels for plan progress and evidence review.[1]
- **GitHub Bot**: `@codex run research: <topic>` and `@codex delegate: <agent>` comment verbs with status threads and attachments.[6]

## Deep Research Data Flow

1. **Goal intake** -> 2. **Plan generation** (sub-queries, success criteria) -> 3. **Acquisition** (search, crawl, fetch PDFs/images) -> 4. **Extraction** (key facts, quantitative data) -> 5. **Cross-check & refutation** -> 6. **Report synthesis** (mandatory citations) -> 7. **Feedback loop** for follow-up questions.[3]

## Security & Compliance Considerations

- Maintain per-agent manifests describing granted capabilities and audit logs for every privileged operation.[7]
- Enforce multi-domain corroboration for timely or news content; flag single-source claims for reviewer attention.[10]
- Require citations for all surfaced conclusions; reports must embed URL, retrieval timestamp, and confidence metadata.[3]
- Support redaction tooling so outputs destined for external surfaces (e.g., GitHub comments) strip sensitive repository paths or secrets by default.[5]

## Rollout and Compatibility

- Ship behind feature flags (e.g., `CODEX_AGENT_RUNTIME`, `CODEX_DEEP_RESEARCH`) with per-surface enablement toggles.
- Pilot in `zapabob/codex` forks to validate developer ergonomics before upstream PRs; keep configuration format stable to minimize merge drift.
- Provide migration guides for existing users: default configuration yields current single-agent behavior until explicit agent definitions are added.
- Document sandbox expectations (environment variables, seatbelt behavior) to ensure delegated agents and Deep Research tasks short-circuit gracefully when capabilities are unavailable.

## Open Questions

- Token arbitration: should Codex support bidding between agents or centrally allocate quotas?
- Evidence storage footprint: what retention policy satisfies compliance without inflating storage costs?
- IDE UX: do we expose multi-agent timelines inline or rely on a dedicated activity panel?
- Long-running research: how do we checkpoint and resume across CLI sessions without leaking sensitive state?

---

[1]: https://github.com/openai/codex
[2]: https://docs.anthropic.com/claude/docs/subagents
[3]: https://openai.com/index/deep-research
[4]: https://openai.com/blog/codex-updates
[5]: https://help.openai.com/en/articles/9795883-deep-research-overview
[6]: https://www.itpro.com/software/development/openai-codex-enterprise-rollout
[7]: https://openai.com/trust
[8]: https://platform.openai.com/docs/assistants/how-it-works#delegation
[9]: https://github.com/modelcontextprotocol/awesome-mcp
[10]: https://www.theverge.com/2024/9/12/openai-deep-research-observability
[11]: https://www.theverge.com/2024/9/12/openai-deep-research-lite-mode
