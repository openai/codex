# LLM Stream 实现分析

> 目标文件: `docs/llm_stream.md`

---

## 1. 默认模式

| Wire API | 默认模式 | 配置控制 |
|----------|----------|----------|
| **Responses API** | 原始流（无聚合层） | 不可配置 |
| **Chat API** | `AggregatedOnly`（聚合到完成时） | `show_raw_agent_reasoning` |

**关键代码** (`core/src/client.rs:255-274`):

```rust
pub async fn stream(&mut self, prompt: &Prompt) -> Result<ResponseStream> {
    match self.state.provider.wire_api {
        WireApi::Responses => self.stream_responses_api(prompt).await,  // 原始流
        WireApi::ResponsesWebsocket => self.stream_responses_websocket(prompt).await,
        WireApi::Chat => {
            let api_stream = self.stream_chat_completions(prompt).await?;

            if self.state.config.show_raw_agent_reasoning {
                // Streaming 模式: delta 立即转发
                Ok(map_response_stream(api_stream.streaming_mode(), ...))
            } else {
                // AggregatedOnly 模式: 聚合到完成时 (默认)
                Ok(map_response_stream(api_stream.aggregate(), ...))
            }
        }
    }
}
```

---

## 2. 配置方式

### 2.1 配置项: `show_raw_agent_reasoning`

| 值 | Chat API 行为 | 推理内容显示 |
|----|---------------|--------------|
| `false` (默认) | `aggregate()` - 聚合模式 | 隐藏原始推理 delta |
| `true` | `streaming_mode()` - 流式模式 | 显示原始推理 delta |

### 2.2 配置方法

**方法 1: 配置文件 (`~/.codex/config.toml`)**

```toml
show_raw_agent_reasoning = true
```

**方法 2: CLI 参数**

```bash
codex --oss  # 启用 OSS 模式，自动设置 show_raw_agent_reasoning = true
```

**方法 3: 项目级配置 (`codex.toml`)**

```toml
show_raw_agent_reasoning = true
```

### 2.3 配置定义

**文件**: `core/src/config/mod.rs:149-151`

```rust
/// When set to `true`, `AgentReasoningRawContentEvent` events will be shown in the UI/output.
/// Defaults to `false`.
pub show_raw_agent_reasoning: bool,
```

---

## 3. 两种聚合模式详解

### 3.1 AggregatedOnly 模式 (默认)

**行为**:
- `OutputTextDelta` → 累积到 `cumulative` 字符串，**不立即返回**
- `ReasoningContentDelta` → 累积到 `cumulative_reasoning`，**不立即返回**
- `Completed` 事件时 → 一次性返回:
  1. `OutputItemDone(Reasoning { ... })` - 完整推理内容
  2. `OutputItemDone(Message { ... })` - 完整文本内容
  3. `Completed` 事件

**代码** (`codex-api/src/endpoint/chat.rs:233-239`):

```rust
Poll::Ready(Some(Ok(ResponseEvent::OutputTextDelta(delta)))) => {
    this.cumulative.push_str(&delta);
    if matches!(this.mode, AggregateMode::Streaming) {
        return Poll::Ready(Some(Ok(ResponseEvent::OutputTextDelta(delta))));
    } else {
        continue;  // AggregatedOnly: 跳过，不返回
    }
}
```

**适用场景**:
- 批量处理
- 不需要实时 UI 更新
- 减少 UI 刷新频率

### 3.2 Streaming 模式

**行为**:
- `OutputTextDelta` → **立即返回**每个 delta
- `ReasoningContentDelta` → **立即返回**每个 delta
- 同时累积到内部状态

**适用场景**:
- 实时 UI 更新
- 打字机效果
- 显示原始推理过程

### 3.3 模式切换实现

**代码** (`codex-api/src/endpoint/chat.rs:267-281`):

```rust
pub trait AggregateStreamExt {
    fn aggregate(self) -> AggregatedStream;
    fn streaming_mode(self) -> ResponseStream;
}

impl AggregateStreamExt for ResponseStream {
    fn aggregate(self) -> AggregatedStream {
        AggregatedStream::new(self, AggregateMode::AggregatedOnly)  // 包装
    }

    fn streaming_mode(self) -> ResponseStream {
        self  // 直接返回，无包装
    }
}
```

**关键区别**:
| 方法 | 返回类型 | 行为 |
|------|----------|------|
| `aggregate()` | `AggregatedStream` | 包装原始流，缓冲 delta 直到完成 |
| `streaming_mode()` | `ResponseStream` | **直接返回 `self`**，无任何包装 |

---

## 4. Response API vs Chat API 事件流对比

### 4.1 Response API (OpenAI Responses)

```
SSE Event Flow:
┌─────────────────────────────────────────────────────┐
│ response.created                                    │
│ response.output_item.added (item 开始)              │
│ response.output_text.delta* (多个文本 delta)        │
│ response.reasoning_text.delta* (多个推理 delta)     │
│ response.output_item.done (item 完成)               │
│ response.completed                                  │
└─────────────────────────────────────────────────────┘
```

**特点**:
- 事件类型明确标识 item 生命周期
- 不需要聚合层，item 天然完整
- 直接使用原始流

### 4.2 Chat API (OpenAI Chat Completions)

```
SSE Event Flow:
┌─────────────────────────────────────────────────────┐
│ choices[].delta.content (文本 delta)                │
│ choices[].delta.reasoning (推理 delta)              │
│ choices[].delta.tool_calls (工具调用 delta)         │
│ finish_reason: "stop" | "tool_calls" | "length"    │
│ [DONE]                                              │
└─────────────────────────────────────────────────────┘
```

**特点**:
- 依赖 `finish_reason` 判断完成
- Tool calls 需要按 index 合并
- 需要聚合层组装完整 item

### 4.3 完整事件处理序列图

```
┌──────────┐     ┌─────────────┐     ┌──────────────┐     ┌─────────────┐     ┌─────┐
│  Client  │     │ ModelClient │     │  codex-api   │     │ SSE Process │     │ LLM │
└────┬─────┘     └──────┬──────┘     └──────┬───────┘     └──────┬──────┘     └──┬──┘
     │                  │                   │                    │               │
     │ stream(prompt)   │                   │                    │               │
     │─────────────────>│                   │                    │               │
     │                  │                   │                    │               │
     │                  │ stream_chat_completions()              │               │
     │                  │──────────────────>│                    │               │
     │                  │                   │                    │               │
     │                  │                   │ HTTP POST /chat/completions        │
     │                  │                   │───────────────────────────────────>│
     │                  │                   │                    │               │
     │                  │                   │<─ ─ ─ ─ ─ ─ ─ SSE Stream ─ ─ ─ ─ ─│
     │                  │                   │                    │               │
     │                  │                   │ spawn_chat_stream()│               │
     │                  │                   │───────────────────>│               │
     │                  │                   │                    │               │
     │                  │<── ResponseStream ┼────────────────────│               │
     │                  │                   │                    │               │
     │                  │ aggregate() 或 streaming_mode()        │               │
     │                  │───────┐           │                    │               │
     │                  │       │ wrap/     │                    │               │
     │                  │<──────┘ pass-through                   │               │
     │                  │                   │                    │               │
     │<─ ResponseStream │                   │                    │               │
     │                  │                   │                    │               │
     │                  │                   │        ┌───────────┴───────────┐   │
     │                  │                   │        │ Loop: process events  │   │
     │                  │                   │        │ - OutputTextDelta     │   │
     │                  │                   │        │ - ReasoningDelta      │   │
     │                  │                   │        │ - OutputItemDone      │   │
     │                  │                   │        │ - Completed           │   │
     │                  │                   │        │ - [Error handling]    │   │
     │                  │                   │        └───────────┬───────────┘   │
     │                  │                   │                    │               │
     │ poll events      │                   │                    │               │
     │<═══════════════════════════════════════════════════════════               │
     │                  │                   │                    │               │
```

**流程说明**:
1. Client 调用 `stream(prompt)`
2. ModelClient 根据 WireApi 选择 Chat 或 Responses API
3. codex-api 发起 HTTP 请求，接收 SSE 流
4. 异步任务 spawn，持续处理 SSE 事件
5. 事件通过 channel 传递到 ResponseStream
6. `aggregate()` 包装或 `streaming_mode()` 透传
7. Client 通过 poll 获取事件

---

## 5. 事件处理时机

### 5.1 SSE 层 (逐事件处理)

**文件**: `codex-api/src/sse/responses.rs:311-317`

```rust
match process_responses_event(event) {
    Ok(Some(event)) => {
        if matches!(event, ResponseEvent::Completed { .. }) {
            response_completed = Some(event);  // Completed 延迟到流结束
        } else if tx_event.send(Ok(event)).await.is_err() {
            return;  // 其他事件立即发送
        }
    }
}
```

### 5.2 History 更新时机

| 事件类型 | History 更新 | UI 刷新 |
|----------|-------------|---------|
| `OutputTextDelta` | ❌ 不更新 | ✅ 立即刷新 (Streaming 模式) |
| `OutputItemAdded` | ❌ 不更新 | ✅ 通知 item 开始 |
| `OutputItemDone` | ✅ **立即持久化** | ✅ 通知 item 完成 |
| `Completed` | ✅ 更新 response_id | ✅ 发送 TurnDiff |

### 5.3 TUI Newline-Gated 更新

TUI 不是每个 delta 都重绘，而是按行累积:

```rust
// tui/src/streaming/controller.rs
pub fn push(&mut self, delta: &str) -> bool {
    state.collector.push_delta(delta);
    if delta.contains('\n') {
        let completed_lines = state.collector.commit_complete_lines();
        if !completed_lines.is_empty() {
            state.enqueue(completed_lines);
            return true;  // 需要重绘
        }
    }
    false // 不需要立即重绘
}
```

---

## 6. 流式错误处理

### 6.1 错误类型

**文件**: `codex-api/src/error.rs`

| 错误类型 | 触发条件 | 是否可重试 |
|----------|----------|-----------|
| `ContextWindowExceeded` | error code = `"context_length_exceeded"` | ❌ 不可重试 |
| `QuotaExceeded` | error code = `"insufficient_quota"` | ❌ 不可重试 |
| `UsageNotIncluded` | error code = `"usage_not_included"` | ❌ 不可重试 |
| `Retryable { message, delay }` | 其他错误 (如 rate_limit) | ✅ 可重试，含延迟 |
| `Stream(message)` | SSE 解析错误、超时、连接断开 | ⚠️ 视情况 |

### 6.2 错误检测与分类

**文件**: `codex-api/src/sse/responses.rs:190-213`

```rust
"response.failed" => {
    if let Some(error) = resp_val.get("error") {
        if is_context_window_error(&error) {
            return Err(ApiError::ContextWindowExceeded);
        } else if is_quota_exceeded_error(&error) {
            return Err(ApiError::QuotaExceeded);
        } else {
            let delay = try_parse_retry_after(&error);  // 解析 "try again in Xs"
            return Err(ApiError::Retryable { message, delay });
        }
    }
}
```

### 6.3 错误传播路径

```
SSE 事件流
    ↓
process_sse() [sse/responses.rs:255]
    ├─ timeout(idle_timeout, stream.next())
    │   └─ [超时] → ApiError::Stream("idle timeout...")
    ├─ SSE 解析错误 → 跳过 (continue)
    ├─ "response.failed" 事件
    │   └─ 分类为 ContextWindowExceeded / QuotaExceeded / Retryable
    └─ tx_event.send(Err(error))
           ↓
    ResponseStream { rx_event }
           ↓
    AggregatedStream (如使用)
    └─ Poll::Ready(Some(Err(e))) → 错误直接透传
```

### 6.4 空闲超时处理

```rust
// sse/responses.rs:267-296
let response = timeout(idle_timeout, stream.next()).await;
match response {
    Err(_) => {
        tx_event.send(Err(ApiError::Stream("idle timeout waiting for SSE".into()))).await;
        return;
    }
    // ...
}
```

**行为**: 每次等待 SSE 事件时启动超时计时器，超时则终止流并返回错误。

### 6.5 重试策略

| 阶段 | 是否重试 | 说明 |
|------|----------|------|
| HTTP 请求建立 | ✅ | 根据 `provider.retry` 策略 |
| SSE 流处理中 | ❌ | 错误直接传播，不自动重试 |
| 客户端层 | ⚠️ | `Retryable.delay` 提供建议延迟 |

---

## 7. hyper-sdk Crush 模式

**文件**: `provider-sdks/hyper-sdk/src/stream/processor.rs`

另一种流处理模式，提供累积快照:

```rust
processor.on_update(|snapshot| async move {
    // 每次回调收到完整累积状态 (不是 delta)
    db.update_message(msg_id, &snapshot.text).await?;
    Ok(())
}).await?;
```

**特点**:
- 每次回调收到 `StreamSnapshot` (完整累积状态)
- 适合 "更新同一条消息" 的 UI 模式
- 内部自动处理 delta 合并

---

## 8. 配置优先级

```
1. CLI 参数 (--oss)
2. 项目配置 (codex.toml)
3. 用户配置 (~/.codex/config.toml)
4. 默认值 (false)
```

**代码** (`core/src/config/mod.rs:1425-1427`):

```rust
show_raw_agent_reasoning: cfg
    .show_raw_agent_reasoning
    .or(show_raw_agent_reasoning)  // 优先使用项目配置
    .unwrap_or(false),             // 默认 false
```

---

## 9. 关键文件索引

| 文件 | 作用 |
|------|------|
| `core/src/client.rs:255-274` | 模式选择逻辑 |
| `core/src/config/mod.rs:149-151` | 配置定义 |
| `codex-api/src/endpoint/chat.rs:113-293` | `AggregatedStream` 实现 |
| `codex-api/src/sse/responses.rs` | Response API SSE 处理 |
| `codex-api/src/sse/chat.rs` | Chat API SSE 处理 |
| `tui/src/streaming/controller.rs` | TUI newline-gated 更新 |
| `provider-sdks/hyper-sdk/src/stream/processor.rs` | Crush 模式处理器 |

---

## 10. 总结

| 问题 | 答案 |
|------|------|
| **默认模式?** | Chat API: `AggregatedOnly`; Responses API: 原始流 |
| **如何启用 Streaming?** | 设置 `show_raw_agent_reasoning = true` |
| **每个 delta 都刷新 UI?** | Streaming 模式下会发送事件，但 TUI 按行累积后才重绘 |
| **每个 delta 都更新 History?** | 否，只在 `OutputItemDone` 时持久化 |
