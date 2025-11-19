# codex-app-server（中文说明）

`codex app-server` 是 Codex 用来驱动富客户端（例如 [Codex VS Code 扩展](https://marketplace.visualstudio.com/items?itemName=openai.chatgpt)）的接口。消息 schema 目前尚不稳定，但如果你想在 Codex 之上构建实验性的 UI，这部分仍然很有参考价值。

## 目录

- [协议](#协议)
- [消息 Schema](#消息-schema)
- [生命周期概览](#生命周期概览)
- [初始化](#初始化)
- [核心原语](#核心原语)
- [线程与回合接口](#线程与回合接口)
- [认证接口](#认证接口)
- [事件（WIP）](#事件work-in-progress)

## 协议

和 [MCP](https://modelcontextprotocol.io/) 类似，`codex app-server` 支持双向通信，通过 stdio 传输 JSONL 流。协议基于 JSON-RPC 2.0，不过省略了 `\"jsonrpc\":\"2.0\"` 这个字段。

## 消息 Schema

目前可以通过以下命令导出 schema：

- 使用 `codex app-server generate-ts` 生成 TypeScript 类型；
- 使用 `codex app-server generate-json-schema` 生成 JSON Schema bundle。

生成结果与运行命令所用 Codex 版本一一对应，因此得到的类型/Schema 与该版本是严格匹配的：

```shell
codex app-server generate-ts --out DIR
codex app-server generate-json-schema --out DIR
```

## 生命周期概览

- **初始化一次**：启动 codex app-server 进程后，首先发送 `initialize` 请求附带客户端元信息，然后发送 `initialized` 通知。在此握手完成前发送的其他请求都会被拒绝。
- **启动或恢复线程**：调用 `thread/start` 打开一个新的会话，响应会返回 thread 对象，同时你会收到 `thread/started` 通知；如果要继续既有会话，则改用 `thread/resume` 并传入 id。
- **开始一个回合（turn）**：要发送用户输入，调用 `turn/start` 并提供 `threadId` 和用户输入。你可以在参数里覆写 model、cwd、sandbox policy 等。调用会立即返回新 turn 对象并触发 `turn/started` 通知。
- **流式事件**：在 `turn/start` 之后，持续从 stdout 读取 JSON-RPC 通知；你会看到 `item/started`、`item/completed`，以及 `item/agentMessage/delta`、各种工具进度等，它们共同描述了模型的流式输出以及所有副作用（命令、工具调用、reasoning 等）。
- **结束回合**：当模型完成（或你通过 `turn/interrupt` 中断）后，服务端会发送 `turn/completed`，其中包含最终的 turn 状态以及 token 使用情况。

## 初始化

客户端必须在调用任何其他方法之前，先发送一次 `initialize` 请求，然后发送 `initialized` 通知。服务端会返回其在访问上游服务时使用的 user agent 字符串；在完成这个过程之前发送的请求都会收到 `\"Not initialized\"` 错误，多次调用 `initialize` 则会得到 `\"Already initialized\"`。

示例：

```json
{ "method": "initialize", "id": 0, "params": {
    "clientInfo": { "name": "codex-vscode", "title": "Codex VS Code Extension", "version": "0.1.0" }
} }
{ "id": 0, "result": { "userAgent": "codex-app-server/0.1.0 codex-vscode/0.1.0" } }
{ "method": "initialized" }
```

## 核心原语

有三个顶层原语：

- **Thread（线程）**：表示 Codex agent 与用户之间的一次对话，一个 thread 里包含多个 turn。
- **Turn（回合）**：一次对话回合，通常以用户消息开始，以 agent 消息结束，一个 turn 内会产生多个 item。
- **Item（条目）**：每个 turn 内的输入/输出单元，既包含用户输入，也包含 agent 输出，同时用于持久化和后续上下文构建。

## 线程与回合接口

JSON-RPC API 暴露了一组接口用于管理 Codex 会话。Threads 保存长生命周期的会话元数据，Turns 保存每次消息往返（输入 → Codex 输出，包括流式 item）。线程接口用于创建、列出或归档会话；回合接口和通知则驱动具体对话。

### 快速参考

- `thread/start` —— 创建新线程；会发出 `thread/started`，并自动订阅该线程的 turn/item 事件。
- `thread/resume` —— 通过 id 重开一个已存在的线程，之后的 `turn/start` 都追加到这个线程。
- `thread/list` —— 分页列出已保存的 rollout，支持基于 cursor 的分页以及按 `modelProviders` 过滤。
- `thread/archive` —— 把线程的 rollout 文件移入归档目录；成功时返回 `{}`。
- `turn/start` —— 向线程追加用户输入并启动 Codex 生成；同步返回初始 `turn` 对象，并流式发送 `turn/started`、`item/*`、`turn/completed` 通知。
- `turn/interrupt` —— 通过 `(thread_id, turn_id)` 请求中断正在运行的 turn；成功时返回空 `{}`，并以 `status: "interrupted"` 结束该回合。
- `review/start` —— 启动该线程的自动代码审查；响应会返回一个 `codeReview` 类型的 item，并且你会收到对应的 `item/started` / `item/completed` 通知（详见原 README 中的示例）。

## 认证接口

JSON-RPC 的认证/账号接口通过请求-响应方法加上服务端主动通知（无 `id`）的方式暴露。你可以使用这些接口来查看认证状态、发起/取消登录、退出登录，以及查看 ChatGPT 的 rate limits。

### 快速参考

- `account/read` —— 读取当前账号信息，可选刷新 token。
- `account/login/start` —— 启动登录（`apiKey` 或 `chatgpt`）。
- `account/login/completed`（通知）—— 登录完成（成功或失败）后发送。
- `account/login/cancel` —— 通过 `loginId` 取消一个未完成的 ChatGPT 登录。
- `account/logout` —— 退出登录；会触发 `account/updated`。
- `account/updated`（通知）—— 认证模式变化时发送（`authMode`: `apikey`、`chatgpt` 或 `null`）。
- `account/rateLimits/read` —— 读取 ChatGPT 的限流信息；更新通过 `account/rateLimits/updated` 通知推送。

（登录流程、退出、rate limits 示例与原文一致，这里不逐行翻译 JSON，以免冗长；你可以直接对照英文 README 中的例子。）

### 开发者提示

- `codex app-server generate-ts --out <dir>` 会在 `v2/` 目录下生成 v2 类型。
- `codex app-server generate-json-schema --out <dir>` 会输出 `codex_app_server_protocol.schemas.json`。
- 更多配置项可参考根目录 `docs/config.md` 中“Authentication and authorization”部分。

## 事件（work-in-progress）

事件通知是服务器发起的事件流，覆盖线程生命周期、回合生命周期以及其中的各类 item。启动或恢复线程后，持续从 stdout 读取 `thread/started`、`turn/*` 和 `item/*` 通知即可。

### Turn 事件

在一个 turn 运行期间，app-server 会不断发送 JSON-RPC 通知。每个 turn 以 `turn/started` 开始（包含初始 `turn`），以 `turn/completed` 结束（包含最终 `turn` 以及 token `usage`）。客户端可以订阅自己关心的事件，并在 UI 中增量渲染。每个 item 的生命周期固定为：`item/started` → 若干该 item 专属的增量事件 → `item/completed`。

#### ThreadItem

`ThreadItem` 是 turn 响应和 `item/*` 通知中携带的联合类型，目前支持的 item 包括：

- `userMessage` —— `{id, content}`，其中 `content` 是用户输入列表（`text`、`image` 或 `localImage`）。
- `agentMessage` —— `{id, text}`，包含累积的 agent 回复。
- `reasoning` —— `{id, summary, content}`，`summary` 用于流式推理总结（多数 OpenAI 模型支持），`content` 用于原始推理块（例如一些开源模型）。
- `mcpToolCall` —— `{id, server, tool, status, arguments, result?, error?}`，描述 MCP 调用；`status` 为 `inProgress`、`completed` 或 `failed`。
- `webSearch` —— `{id, query}`，表示 agent 发起的 Web 搜索请求。

所有 item 都会发出两个通用生命周期事件：

- `item/started` —— 新的工作单元开始时，发送完整 `item`，以便 UI 立即渲染；其中的 `item.id` 对应后续增量事件使用的 `itemId`。
- `item/completed` —— 工作结束时，发送最终 `item`，将其视为 authoritative 状态。

#### agentMessage

- `item/agentMessage/delta` —— 流式追加 agent 回复的文本；同一个 `itemId` 的多个 `delta` 需要按顺序拼接。

#### reasoning

- `item/reasoning/summaryTextDelta` —— 推理摘要的流式增量；`summaryIndex` 标记当前摘要段落索引。
- `item/reasoning/summaryPartAdded` —— 标记摘要段落之间的边界；之后的 `summaryTextDelta` 会共享同一个 `summaryIndex`。
- `item/reasoning/textDelta` —— 推理原文的流式增量（通常用于开源模型）；`contentIndex` 用来把属于同一段内容的增量归组后再展示。

