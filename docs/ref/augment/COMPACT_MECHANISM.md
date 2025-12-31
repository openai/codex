# Augment Compact 机制深度分析

## 文档信息
- **分析时间**: 2025-12-04
- **源文件**: `chunks.84.mjs`, `chunks.61.mjs`, `chunks.58.mjs`, `chunks.78.mjs`
- **分析范围**: 对话历史压缩机制的完整技术实现

---

## 核心发现

### 架构概览

Augment 实现了**两层压缩机制**：

```
┌─────────────────────────────────────────────────────────────┐
│                    Augment Compact 架构                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌───────────────────┐    ┌───────────────────────────────┐ │
│  │   UI 层压缩        │    │       Context 层压缩           │ │
│  │  (Compact Mode)    │    │  (History Summarization)      │ │
│  ├───────────────────┤    ├───────────────────────────────┤ │
│  │ • --compact flag  │    │ • LLM 生成摘要                 │ │
│  │ • 输出格式简化      │    │ • 替换旧对话历史               │ │
│  │ • 零 Token 成本    │    │ • 版本控制机制                 │ │
│  └───────────────────┘    └───────────────────────────────┘ │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 1. 触发条件详解

### 1.1 双重触发机制

**文件位置**: `chunks.84.mjs:1010-1016, 1423`

```
触发方式 1: 主动检查（每次 Agent Loop 迭代前）
┌────────────────────────────────────────────────────────────┐
│ Agent Loop 迭代开始                                         │
│       │                                                    │
│       ▼                                                    │
│ maybeAddHistorySummaryNode(isCacheAboutToExpire=false)    │
│       │                                                    │
│       ▼                                                    │
│ 检查 triggerOnHistorySizeChars 阈值                        │
└────────────────────────────────────────────────────────────┘

触发方式 2: 定时调度（缓存即将过期时）
┌────────────────────────────────────────────────────────────┐
│ maybeScheduleSummarization(cacheAge)                       │
│       │                                                    │
│       ▼                                                    │
│ 计算: delay = cacheTTL - cacheAge - bufferTime             │
│       │                                                    │
│       ▼                                                    │
│ setTimeout(() => maybeAddHistorySummaryNode(true), delay)  │
└────────────────────────────────────────────────────────────┘
```

### 1.2 触发条件判断逻辑

**文件位置**: `chunks.84.mjs:1026-1037`

```javascript
async maybeAddHistorySummaryNode(isCacheAboutToExpire = false, abortSignal) {
    // 条件 1: 必须配置了 prompt
    if (!this._params.prompt || this._params.prompt.trim() === "") {
        return false;  // 未配置摘要 prompt，不触发
    }

    // 条件 2: 根据触发模式选择阈值
    let threshold = isCacheAboutToExpire
        ? this._params.triggerOnHistorySizeCharsWhenCacheExpiring  // 缓存过期模式
        : this._params.triggerOnHistorySizeChars;                   // 普通模式

    if (threshold <= 0) return false;  // 阈值为 0 表示禁用

    // 条件 3: 分割历史，检查是否有足够内容需要摘要
    let { head, tail, headSizeChars, tailSizeChars } =
        Wtt(chatHistory, historyTailSizeCharsToExclude, threshold, 1);

    // 条件 4: head 为空则无需摘要
    if (head.length === 0) return false;

    // 所有条件满足，开始生成摘要...
}
```

### 1.3 触发条件参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `triggerOnHistorySizeChars` | number | 0 (禁用) | 主动模式触发阈值（字符数） |
| `triggerOnHistorySizeCharsWhenCacheExpiring` | number | 0 (禁用) | 缓存过期模式触发阈值 |
| `historyTailSizeCharsToExclude` | number | 0 | 最近 N 字符的历史不压缩 |
| `cacheTTLMs` | number | 0 | Prompt Cache TTL（毫秒） |
| `bufferTimeBeforeCacheExpirationMs` | number | 0 | 缓存过期前的缓冲时间 |

### 1.4 isHistorySummaryEnabled 完整计算公式

**文件位置**: `chunks.84.mjs:1198-1204`

```javascript
// Agent Loop 构造函数中
let isCodeReviewBot = botType === 1 || botType === "CODE_REVIEW_BOT" ||
                      botType === 2 || botType === "CODE_REVIEW_EVAL_BOT";

// 启用条件：不是 Code Review Bot 且 Feature Flag 指定了版本号
isHistorySummaryEnabled = !isCodeReviewBot && historySummaryMinVersion !== ""
```

**启用条件分解**：

| 条件 | 说明 | 来源 |
|------|------|------|
| `!isCodeReviewBot` | 不是代码审查机器人 | Bot 类型判断 |
| `historySummaryMinVersion !== ""` | Feature Flag 指定了最小版本号 | 后端 API |

**注意**：即使 `isHistorySummaryEnabled = true`，还需要满足以下条件才会实际触发摘要：
1. `this._params.prompt` 不为空
2. `triggerOnHistorySizeChars > 0` 或 `triggerOnHistorySizeCharsWhenCacheExpiring > 0`
3. 历史大小超过阈值

### 1.5 maybeScheduleSummarization 实际未被使用

**重要发现**：虽然代码中定义了 `maybeScheduleSummarization()` 用于缓存过期触发，但在当前代码库中**几乎没有被调用**。

```javascript
// chunks.84.mjs:1010-1016
maybeScheduleSummarization(cacheAge) {
    // 条件 1：必须启用 + 阈值 > 0
    if (!this._config.isHistorySummaryEnabled ||
        this._params.triggerOnHistorySizeCharsWhenCacheExpiring <= 0) return;

    // 条件 2：计算延迟时间
    let delay = this._params.cacheTTLMs - cacheAge - this._params.bufferTimeBeforeCacheExpirationMs;

    // 条件 3：设置定时回调
    if (delay > 0) {
        this._callbacksManager.addCallback(signal => {
            this.maybeAddHistorySummaryNode(true, signal)  // isCacheAboutToExpire=true
        }, delay);
    }
}
```

**当前状态**：缓存过期触发机制被禁用，只有主动触发（每次 Agent Loop 迭代前）在工作。

### 1.6 Feature Flags 配置来源

**配置流程图**：

```
┌─────────────────────────────────────────────────────────────────┐
│ 后端 API (/get-models)                                           │
├─────────────────────────────────────────────────────────────────┤
│ response.feature_flags: {                                        │
│   "history_summary_min_version": "3",      ← 版本号             │
│   "history_summary_params": "{...}"        ← JSON 配置字符串     │
│ }                                                                │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ API Server.toGetModelsResult() (chunks.72.mjs:1072)              │
├─────────────────────────────────────────────────────────────────┤
│ // snake_case → camelCase 转换                                   │
│ featureFlags: {                                                  │
│   historySummaryMinVersion: "3",                                 │
│   historySummaryParams: "{...}"                                  │
│ }                                                                │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ Agent Loop 构造函数 (chunks.84.mjs:1199-1205)                    │
├─────────────────────────────────────────────────────────────────┤
│ this.chatHistorySummarizationModel = new Ype({                   │
│     agentState: this.agentState,                                 │
│     sendSilentExchangeNonStreamingText: ...                      │
│ }, {                                                             │
│     historySummaryParams: featureFlags.historySummaryParams,     │
│     isHistorySummaryEnabled: !isCodeReviewBot &&                 │
│                              historySummaryMinVersion !== ""     │
│ });                                                              │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ mGt() 参数解析 (chunks.84.mjs:779-814)                           │
├─────────────────────────────────────────────────────────────────┤
│ // JSON 字符串解析为配置对象                                     │
│ this._params = {                                                 │
│     triggerOnHistorySizeChars: ...,                              │
│     prompt: ...,                                                 │
│     abridgedHistoryParams: {...}                                 │
│ }                                                                │
└─────────────────────────────────────────────────────────────────┘
```

**关键配置项 JSON 格式**：

```json
{
  "trigger_on_history_size_chars": 50000,
  "trigger_on_history_size_chars_when_cache_expiring": 30000,
  "history_tail_size_chars_to_exclude": 5000,
  "prompt": "<实际的摘要生成 prompt，见下文详细说明>",
  "cache_ttl_ms": 300000,
  "buffer_time_before_cache_expiration_ms": 30000,
  "abridged_history_params": {
    "total_chars_limit": 10000,
    "user_message_chars_limit": 1000,
    "agent_response_chars_limit": 2000
  }
}
```

> ⚠️ **重要说明**：
> - `prompt` 字段包含实际的摘要生成 prompt，由后端 API 下发
> - 代码中**没有硬编码**的 prompt 内容
> - 默认值为空字符串 `""`，必须通过 Feature Flags 配置才能启用摘要功能
> - 实际 prompt 内容见下文"Prompt 系统详解"章节

---

## 2. 多级 Compact 机制分析

### 2.1 结论：**单级压缩 + 版本迭代**

Augment **没有**实现传统意义上的多级压缩（如 L1/L2/L3 缓存），而是采用：

```
┌──────────────────────────────────────────────────────────────┐
│                    压缩策略：替换而非嵌套                      │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  第一次压缩:                                                  │
│  [Exchange 1] [Exchange 2] ... [Exchange N]                  │
│       │                                                      │
│       ▼                                                      │
│  [Summary Node v3] [Exchange N-3] [Exchange N-2] [Exchange N]│
│                                                              │
│  第二次压缩（当对话继续增长后）:                               │
│  [Summary Node v3] [Exchange N-3] ... [Exchange M]           │
│       │                                                      │
│       ▼                                                      │
│  [New Summary Node v3] [Exchange M-3] [Exchange M-2] [Exchange M]│
│                                                              │
│  注意：旧的 Summary Node 被**丢弃**，不会嵌套在新 Summary 中   │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 2.2 版本控制机制

**文件位置**: `chunks.84.mjs:982, 1017-1025`

```javascript
class ChatHistorySummarizationModel {
    historySummaryVersion = 3;  // 当前版本号

    preprocessChatHistory(history) {
        let result = history.concat();

        // 1. 从后往前找最后一个有效的 Summary Node
        let lastValidSummaryIndex = -1;
        for (let i = result.length - 1; i >= 0; i--) {
            if (result[i].isHistorySummary &&
                result[i].historySummaryVersion === this.historySummaryVersion) {
                lastValidSummaryIndex = i;
                break;
            }
        }

        if (this._config.isHistorySummaryEnabled) {
            // 2. 从最后一个有效 Summary 开始截断历史
            if (lastValidSummaryIndex > 0) {
                // ⚠️ 关键：slice(n) 会保留 index=n 的元素
                // 即 Summary Node 本身会被保留在结果中
                result = result.slice(lastValidSummaryIndex);
            }

            // 3. 过滤掉版本不匹配的 Summary Node
            result = result.filter(item =>
                !item.isHistorySummary ||
                item.historySummaryVersion === this.historySummaryVersion
            );
        } else {
            // 禁用摘要时，移除所有 Summary Node
            result = result.filter(item => !item.isHistorySummary);
        }

        return result;
    }
}

/*
 * slice(n) 行为说明：
 *
 * 原始历史: [E0, E1, E2, Summary@idx3, E4, E5, E6]
 *                         ↑
 *                  lastValidSummaryIndex = 3
 *
 * slice(3) 结果: [Summary@idx3, E4, E5, E6]
 *                 ↑
 *            Summary Node 被保留！
 *
 * 这意味着 API 请求中会包含 Summary Node 的内容
 */
```

### 2.3 版本升级处理

```
场景：系统从 v2 升级到 v3

升级前的历史:
[Exchange 1] [Summary v2] [Exchange 5] [Exchange 6]

升级后首次处理:
1. 找不到 v3 的 Summary Node
2. 过滤掉 v2 的 Summary Node
3. 结果: [Exchange 1] [Exchange 5] [Exchange 6]
4. 如果触发摘要，会创建新的 v3 Summary
```

---

## 3. 多次 Compact 时 Summary 处理

### 3.1 处理流程

**文件位置**: `chunks.84.mjs:992-1009, 1026-1105`

```
┌─────────────────────────────────────────────────────────────────┐
│                    多次 Compact 处理流程                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Step 1: generateAbridgedHistoryText()                          │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ 过滤条件: filter(item => !item.isHistorySummary)          │  │
│  │                                                           │  │
│  │ 只处理非 Summary 的 exchange，Summary 节点被跳过           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  Step 2: Wtt() 分割历史                                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ chatHistoryForAPI 已经过 preprocessChatHistory 处理        │  │
│  │ 从最后一个有效 Summary 开始，不包含更早的历史               │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  Step 3: 创建新 Summary Node                                    │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ 新 Summary 基于 Step 2 分割出的 head 部分生成              │  │
│  │ 不包含任何旧的 Summary 内容                                │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  结果: 旧 Summary 被遗忘，新 Summary 替代                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 关键代码：Abridged History 生成

**文件位置**: `chunks.84.mjs:992-1009`

```javascript
generateAbridgedHistoryText(excludeAfterRequestId) {
    // 关键：过滤掉所有 isHistorySummary 的节点
    let rawHistory = this._agentLoop.agentState.chatHistory
        .filter(item => !item.isHistorySummary)  // ← 跳过 Summary 节点
        .map(item => item.exchange);

    // 如果提供了截止 ID，只取该 ID 之前的历史
    if (excludeAfterRequestId) {
        let index = rawHistory.findIndex(ex => ex.request_id === excludeAfterRequestId);
        if (index >= 0) rawHistory = rawHistory.slice(0, index);
    }

    // 转换为结构化格式并累积
    let structuredExchanges = cZt(rawHistory);
    let totalChars = 0;
    let abridgedExchanges = [];

    // 从最新往回累积，直到达到字符限制
    for (let i = structuredExchanges.length - 1; i >= 0; i--) {
        let exchange = structuredExchanges[i];
        let formattedText = lZt(exchange, this._params.abridgedHistoryParams);

        if (totalChars + formattedText.length > this._params.abridgedHistoryParams.totalCharsLimit) {
            break;
        }

        abridgedExchanges.push(formattedText);
        totalChars += formattedText.length;
    }

    return abridgedExchanges.reverse().join('\n');
}
```

### 3.3 Summary 信息丢失分析

```
┌──────────────────────────────────────────────────────────────┐
│                    信息保留 vs 丢失                           │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ✅ 保留的信息:                                               │
│  • Tail 部分的完整 exchange                                   │
│  • Abridged History（结构化的历史摘要，10K 字符限制）          │
│  • LLM 生成的自然语言 Summary                                 │
│                                                              │
│  ❌ 丢失的信息:                                               │
│  • 旧的 Summary Node 内容（被过滤掉）                         │
│  • Head 部分的完整 exchange 内容                              │
│  • 超过 Abridged History 字符限制的早期历史                   │
│                                                              │
│  ⚠️ 潜在问题:                                                │
│  • 多次 Compact 后，早期历史完全丢失                          │
│  • Summary 质量依赖 LLM，可能遗漏重要细节                     │
│  • 无法回溯到原始对话内容                                     │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## 4. 消息保留策略详解

### 4.1 历史分割函数 Wtt()

**文件位置**: `chunks.61.mjs:1243-1271`

```javascript
function Wtt(history, tailSizeCharsToExclude, threshold, minTailExchanges) {
    if (history.length === 0) {
        return { head: [], tail: [], headSizeChars: 0, tailSizeChars: 0 };
    }

    let head = [];      // 需要被摘要的部分
    let tail = [];      // 需要保留的部分
    let totalChars = 0;
    let headChars = 0;
    let tailChars = 0;

    // 从后往前遍历
    for (let i = history.length - 1; i >= 0; i--) {
        let exchange = history[i];
        let exchangeSize = Ftt(exchange);  // 计算 exchange 大小

        // 条件：进入 tail 的情况
        // 1. 当前累积字符数 < tailSizeCharsToExclude
        // 2. 或者 tail 数量 < minTailExchanges (至少保留 N 条)
        if (totalChars + exchangeSize < tailSizeCharsToExclude ||
            tail.length < minTailExchanges) {
            tail.push(exchange);
            tailChars += exchangeSize;
        } else {
            head.push(exchange);
            headChars += exchangeSize;
        }

        totalChars += exchangeSize;
    }

    // 如果总大小未超过阈值，不需要摘要
    if (totalChars < threshold) {
        tail.push(...head);
        return { head: [], tail: tail.reverse(), headSizeChars: 0, tailSizeChars: totalChars };
    }

    return {
        head: head.reverse(),
        tail: tail.reverse(),
        headSizeChars: headChars,
        tailSizeChars: tailChars
    };
}
```

### 4.2 分割策略图解

```
原始历史: [E1] [E2] [E3] [E4] [E5] [E6] [E7] [E8] [E9] [E10]
         ←──────────────── 按时间排序 ────────────────────→

参数设置:
• triggerOnHistorySizeChars = 10000
• historyTailSizeCharsToExclude = 3000
• minTailExchanges = 1

分割过程 (从后往前):
┌─────────────────────────────────────────────────────────────┐
│ 步骤 1: 累积 tail                                            │
│ [E10] → tailChars = 800  (< 3000, 加入 tail)                │
│ [E9]  → tailChars = 1600 (< 3000, 加入 tail)                │
│ [E8]  → tailChars = 2400 (< 3000, 加入 tail)                │
│ [E7]  → tailChars = 3200 (> 3000, 但检查 minTailExchanges)  │
│        → tail.length = 3 >= 1, 不再加入 tail                 │
├─────────────────────────────────────────────────────────────┤
│ 步骤 2: 累积 head                                            │
│ [E7] [E6] [E5] [E4] [E3] [E2] [E1] → 加入 head              │
├─────────────────────────────────────────────────────────────┤
│ 步骤 3: 检查是否需要摘要                                     │
│ totalChars (10800) > threshold (10000) → 需要摘要            │
└─────────────────────────────────────────────────────────────┘

分割结果:
• head: [E1] [E2] [E3] [E4] [E5] [E6] [E7]  ← 将被摘要
• tail: [E8] [E9] [E10]                      ← 将被保留
```

### 4.3 二次截断函数 zee()

**文件位置**: `chunks.61.mjs:1218-1232`

```javascript
P6r = 800000;  // 800KB 总限制

function zee(history) {
    let segmentSize = P6r / 3;  // ~266KB 每段
    let segments = [0];  // 记录段起始位置
    let currentSize = 0;

    for (let i = 0; i < history.length; i++) {
        let exchange = history[i];
        let exchangeSize = JSON.stringify(exchange).length;

        // 如果当前段超过限制，开始新段
        if (currentSize + exchangeSize > segmentSize) {
            segments.push(i);
            currentSize = 0;
        }
        currentSize += exchangeSize;
    }

    // 如果段数少于 4，保留全部
    if (segments.length < 4) return history;

    // 保留后半部分的段
    let startSegment = 2 * Math.floor((segments.length - 2) / 2);
    return history.slice(segments[startSegment]);
}
```

**zee() 截断逻辑图解**:

```
假设历史总大小 1.2MB，segmentSize = 266KB

段划分:
┌─────────┬─────────┬─────────┬─────────┬─────────┐
│  Seg 0  │  Seg 1  │  Seg 2  │  Seg 3  │  Seg 4  │
│  266KB  │  266KB  │  266KB  │  266KB  │  200KB  │
└─────────┴─────────┴─────────┴─────────┴─────────┘

segments.length = 5 (>= 4)
startSegment = 2 * Math.floor((5-2)/2) = 2 * 1 = 2

结果: 从 Seg 2 开始保留
┌─────────┬─────────┬─────────┐
│  Seg 2  │  Seg 3  │  Seg 4  │  ← 保留 (约 700KB)
└─────────┴─────────┴─────────┘
```

### 4.4 保留策略总结

| 策略层级 | 函数 | 保留规则 | 作用 |
|---------|------|----------|------|
| **L1: Tail 保留** | `Wtt()` | `historyTailSizeCharsToExclude` 字符内 | 保留最近对话 |
| **L2: 最小保留** | `Wtt()` | 至少 `minTailExchanges` 条 | 防止全部被摘要 |
| **L3: 大小截断** | `zee()` | 总大小 < 800KB 的后半部分 | 防止 API 超限 |
| **L4: Summary 替换** | `maybeAddHistorySummaryNode()` | head 转为 Summary | 减少 token 消耗 |

---

## 5. Summary Node 结构详解

### 5.1 Summary Node 数据结构

**文件位置**: `chunks.84.mjs:1082-1098`

```javascript
let summaryNode = {
    request_id: summaryRequestId,
    request_message: "",
    response_text: "",
    request_nodes: [{
        id: 1,
        type: 0,  // TEXT_NODE
        text_node: {
            content: summaryRequestMessage  // 包含 abridged_history + summary
        }
    }],
    response_nodes: [{
        id: nextId,
        type: 0,  // TEXT_NODE
        content: "Ok. I will continue the conversation from this point."
    }, ...toolNodes]  // 保留最后一个 head exchange 的 tool_use 节点
};
```

### 5.1.1 sequenceId 插值计算

**文件位置**: `chunks.61.mjs:1447-1454`

当 Summary Node 被插入到历史中间位置时，需要计算一个介于相邻节点之间的 `sequenceId`：

```javascript
// addExchangeToHistory 中的插值计算
function addExchangeToHistory(exchange, completed, metadata, insertPosition) {
    let chatHistory = this.chatHistory;

    // insertPosition 指定插入位置（如果未指定则追加到末尾）
    if (insertPosition !== undefined && insertPosition < chatHistory.length) {
        // 插值计算公式
        let prevSeqId = insertPosition > 0
            ? chatHistory[insertPosition - 1].sequenceId
            : 0;
        let nextSeqId = chatHistory[insertPosition].sequenceId;

        // 取中间值，保留 6 位小数精度
        let newSeqId = Math.round((prevSeqId + nextSeqId) / 2 * 1e6) / 1e6;

        exchange.sequenceId = newSeqId;

        // 在指定位置插入
        chatHistory.splice(insertPosition, 0, exchange);
    } else {
        // 追加到末尾
        exchange.sequenceId = chatHistory.length;
        chatHistory.push(exchange);
    }
}
```

**插值示例**：

```
原始历史:
[E0: seqId=0] [E1: seqId=1] [E2: seqId=2] [E3: seqId=3] [E4: seqId=4]

插入 Summary Node 到位置 3 (E3 之前):
prevSeqId = chatHistory[2].sequenceId = 2
nextSeqId = chatHistory[3].sequenceId = 3
newSeqId = (2 + 3) / 2 = 2.5

结果:
[E0: seqId=0] [E1: seqId=1] [E2: seqId=2] [Summary: seqId=2.5] [E3: seqId=3] [E4: seqId=4]
```

**设计目的**：
- 保持历史记录的时间顺序
- 避免重新计算所有后续节点的 sequenceId
- 支持无限次插入（每次取中间值，永远能找到可用的 sequenceId）

### 5.2 Summary Node 模板

**文件位置**: `chunks.84.mjs:711-723`

```xml
<supervisor>
Conversation history between Agent(you) and the user and history of tool calls
was abridged and summarized to reduce context size.

Abridged conversation history:
<abridged_history>
{abridged_history}
</abridged_history>

Summary was generated by Agent(you) so 'I' in the summary represents Agent(you).
Here is the summary:
<summary>
{summary}
</summary>

Continue the conversation and finish the task given by the user from this point.
</supervisor>
```

### 5.3 Abridged History 模板

**文件位置**: `chunks.84.mjs:834-886`

```handlebars
<user>
{{{userMessage}}}
</user>
{{#if hasActions}}
<agent_actions>
{{#if filesModified}}
    <files_modified>
{{#each filesModified}}
        {{{this}}}
{{/each}}
    </files_modified>
{{/if}}
{{#if filesCreated}}
    <files_created>
{{#each filesCreated}}
        {{{this}}}
{{/each}}
    </files_created>
{{/if}}
{{#if filesDeleted}}
    <files_deleted>
{{#each filesDeleted}}
        {{{this}}}
{{/each}}
    </files_deleted>
{{/if}}
{{#if filesViewed}}
    <files_viewed>
{{#each filesViewed}}
        {{{this}}}
{{/each}}
    </files_viewed>
{{/if}}
{{#if terminalCommands}}
    <terminal_commands>
{{#each terminalCommands}}
        {{{this}}}
{{/each}}
    </terminal_commands>
{{/if}}
</agent_actions>
{{/if}}
{{#if agentResponse}}
<agent_response>
{{{agentResponse}}}
</agent_response>
{{else if wasInterrupted}}
<agent_was_interrupted/>
{{else if continues}}
<agent_continues/>
{{/if}}
```

---

## 6. Prompt 系统详解

### 6.1 History Summarization Prompt

**文件位置**: `chunks.84.mjs:708, 1027`

#### 6.1.1 Prompt 来源

```javascript
// 默认配置
jp = {
    prompt: "",  // ⚠️ 默认为空字符串
    ...
}

// 实际使用
async maybeAddHistorySummaryNode(isCacheAboutToExpire, abortSignal) {
    // 检查 prompt 是否配置
    if (!this._params.prompt || this._params.prompt.trim() === "") {
        return false;  // prompt 为空则不触发摘要
    }

    // 调用 LLM 生成摘要
    let response = await sendSilentExchangeNonStreamingText(
        this._params.prompt,  // ← 摘要 prompt
        true,                 // resetCheckpoint
        truncatedHead,        // 需要摘要的历史
        undefined,            // chatMode
        abortSignal
    );
}
```

**配置方式**：

```
后端 API (/get-models)
    ↓
response.feature_flags.history_summary_params = JSON 字符串
    ↓
解析后：{
    "prompt": "<实际的摘要 prompt>",
    ...
}
    ↓
传递给 ChatHistorySummarizationModel
```

#### 6.1.2 Prompt 作用

该 prompt 用于指导 LLM 生成对话历史的摘要，主要作用：

1. **压缩历史内容**：将多条 exchange 压缩为简洁的摘要文本
2. **保持上下文连贯性**：确保摘要包含关键信息，使后续对话能够理解之前的内容
3. **提取关键决策**：记录重要的用户需求、Agent 操作、问题解决方案等
4. **格式化输出**：生成符合预期格式的摘要文本

#### 6.1.3 Prompt 输入内容

LLM 在生成摘要时接收以下输入：

```javascript
// 1. Prompt 本身（来自 this._params.prompt）
// 2. 聊天历史（truncatedHead - 经过 zee() 截断的 head 部分）
let truncatedHead = zee(head);  // 防止发送给 LLM 的历史过大

// truncatedHead 的内容示例：
[
    {
        request_id: "req_1",
        request_message: "用户的问题",
        request_nodes: [...],
        response_text: "Agent 的回复",
        response_nodes: [...]
    },
    // ... 更多 exchanges
]
```

**关键点**：
- 输入的是**完整的 exchange 对象**（包含 request + response + tool calls）
- 已经过 `zee()` 截断，防止超过 LLM 的上下文限制
- 不包含已有的 Summary Node（通过 `preprocessChatHistory` 过滤）

#### 6.1.4 Prompt 预期输出

LLM 应该生成：

```
<summary>
# 对话摘要

## 用户需求
- 用户要求实现功能 X
- 用户提供了约束条件 Y

## Agent 操作
- 查看了文件 A.js
- 修改了文件 B.js，添加了功能 Z
- 执行了测试，发现问题 W

## 关键决策
- 决定使用技术方案 M 而非 N，因为 M 更适合当前场景
- 保留了文件 C.js 以兼容旧代码

## 待完成任务
- 需要优化性能瓶颈 P
- 等待用户确认设计方案 Q
</summary>
```

**格式要求**：
- 使用第一人称（"I" 代表 Agent）
- 简洁但包含关键信息
- 结构化（可以使用标题、列表）
- 聚焦于"为什么"而非"是什么"

#### 6.1.5 Prompt 设计建议

基于 Augment 的架构，一个好的摘要 prompt 应该：

```xml
<system>
You are generating a summary of conversation history to reduce context size while preserving critical information.

## Input
You will receive a chat history containing multiple exchanges between the user and the Agent (you).

## Task
Generate a concise summary that:
1. Captures the user's main requirements and constraints
2. Records key actions taken by the Agent (file modifications, commands executed, etc.)
3. Highlights important decisions and their rationales
4. Lists pending tasks or open questions

## Guidelines
- Use first person ("I" represents the Agent)
- Focus on "why" rather than "what"
- Be concise but comprehensive (target: 500-1000 words)
- Use structured format (headers, bullet points)
- Prioritize recent and important information over old and trivial details

## Output Format
Wrap your summary in <summary></summary> tags.
Use markdown formatting for readability.

## Example
<summary>
# Conversation Summary

## User Requirements
- Implement user authentication system
- Support OAuth2 and JWT tokens

## Agent Actions
- Created auth/ directory and AuthService class
- Integrated bcrypt for password hashing
- Added middleware for token validation

## Key Decisions
- Chose JWT over sessions for better scalability
- Stored refresh tokens in Redis for fast lookup

## Pending Tasks
- Write unit tests for AuthService
- Update API documentation
</summary>
</system>
```

### 6.2 Summary Node 请求模板

**文件位置**: `chunks.84.mjs:711-723`

当摘要生成后，会被插入到一个特殊的 Summary Node 中：

```xml
<supervisor>
Conversation history between Agent(you) and the user and history of tool calls
was abridged and summarized to reduce context size.

Abridged conversation history:
<abridged_history>
{abridged_history}  ← 插入 Abridged History
</abridged_history>

Summary was generated by Agent(you) so 'I' in the summary represents Agent(you).
Here is the summary:
<summary>
{summary}  ← 插入 LLM 生成的摘要
</summary>

Continue the conversation and finish the task given by the user from this point.
</supervisor>
```

**作用**：
1. 告知 LLM 历史已被压缩
2. 提供双重信息：
   - **Abridged History**：结构化的历史记录（工具操作、文件修改等）
   - **Summary**：自然语言摘要（上下文、决策、意图）
3. 指示 LLM 继续任务

### 6.3 Abridged History 模板

**文件位置**: `chunks.84.mjs:834-886`

Abridged History 使用 Handlebars 模板生成结构化的历史摘要：

```handlebars
<user>
{{{userMessage}}}
</user>
{{#if hasActions}}
<agent_actions>
{{#if filesModified}}
    <files_modified>
{{#each filesModified}}
        {{{this}}}
{{/each}}
    </files_modified>
{{/if}}
{{#if filesCreated}}
    <files_created>
{{#each filesCreated}}
        {{{this}}}
{{/each}}
    </files_created>
{{/if}}
{{#if filesDeleted}}
    <files_deleted>
{{#each filesDeleted}}
        {{{this}}}
{{/each}}
    </files_deleted>
{{/if}}
{{#if filesViewed}}
    <files_viewed>
{{#each filesViewed}}
        {{{this}}}
{{/each}}
    </files_viewed>
{{/if}}
{{#if terminalCommands}}
    <terminal_commands>
{{#each terminalCommands}}
        {{{this}}}
{{/each}}
    </terminal_commands>
{{/if}}
</agent_actions>
{{/if}}
{{#if agentResponse}}
<agent_response>
{{{agentResponse}}}
</agent_response>
{{else if wasInterrupted}}
<agent_was_interrupted/>
{{else if continues}}
<agent_continues/>
{{/if}}
```

**生成示例**：

```xml
<user>
Fix the authentication bug in login.js
</user>
<agent_actions>
    <files_viewed>
        /src/auth/login.js
        /src/utils/jwt.js
    </files_viewed>
    <files_modified>
        /src/auth/login.js
    </files_modified>
    <terminal_commands>
        npm test auth
    </terminal_commands>
</agent_actions>
<agent_response>
Fixed the token validation logic. The bug was caused by incorrect expiration time comparison.
</agent_response>
```

**字符限制** (`chunks.84.mjs:737-748, 992-1009`)：

| 项目 | 默认限制 |
|------|---------|
| 总字符数 | 10,000 |
| 用户消息 | 1,000 |
| Agent 响应 | 2,000 |
| 文件路径/命令 | 200 |
| 文件操作数量 | 10 (每类) |

**截断策略**：
- 从最新的 exchange 往回累积
- 直到达到总字符限制
- 使用 `lpe()` 函数进行中间截断（保留开头和结尾）

### 6.4 MEMORIES_COMPRESSION Prompt

**文件位置**: `chunks.77.mjs:1387-1409`

这是一个**独立的**压缩机制，用于压缩 Agent 的长期记忆（而非对话历史）。

#### 6.4.1 Prompt 来源

```javascript
// 从 Feature Flags 读取
let compressionPrompt = ei().flags.memoriesParams.compression_prompt;

if (!compressionPrompt) {
    // prompt 缺失则跳过压缩
    return memories;
}
```

**配置方式**：

```
后端 API (/get-models)
    ↓
response.feature_flags.memories_params = {
    "compression_prompt": "<记忆压缩 prompt>",
    "compression_target": 50,  // 目标行数
    "num_recent_memories_to_keep": 10,
    "recent_memories_subprompt": "<最近记忆处理 prompt>"
}
```

#### 6.4.2 Prompt 作用

用于压缩 `agentMemories`（跨会话的长期记忆）：

```
触发条件: memories.split('\n').length > maxLines

输入: 当前的 agentMemories（文本格式）
输出: 压缩后的 agentMemories
```

#### 6.4.3 与 History Summarization 的区别

| 特性 | History Summarization | MEMORIES_COMPRESSION |
|------|----------------------|----------------------|
| **目标数据** | chatHistory (对话历史) | agentMemories (长期记忆) |
| **触发条件** | 字符数超过阈值 | 行数超过限制 |
| **输出位置** | 插入 Summary Node | 替换 agentMemories |
| **Prompt 来源** | `history_summary_params.prompt` | `memories_params.compression_prompt` |
| **Chat Mode** | 默认 (AGENT) | "MEMORIES_COMPRESSION" |
| **工具可用** | ✅ 是 | ❓ 取决于 chatMode |
| **代码位置** | chunks.84.mjs:1026-1106 | chunks.77.mjs:1383-1409 |

#### 6.4.4 Prompt 设计建议

```xml
<system>
You are compressing the Agent's long-term memories to reduce storage size while preserving important information.

## Input
You will receive a list of memories accumulated across multiple conversations.

## Task
Compress the memories by:
1. Removing redundant information
2. Merging similar memories
3. Keeping recent and important memories intact
4. Summarizing old and less critical information

## Guidelines
- Target output: {compression_target} lines or fewer
- Preserve {num_recent_memories_to_keep} most recent memories verbatim
- Focus on user preferences, project context, and recurring patterns
- Use bullet points for readability

## Output Format
Return the compressed memories as plain text (no XML tags).
One memory per line or use bullet points.

## Example Input
```
- User prefers TypeScript over JavaScript
- Project uses React with Vite
- User asked about authentication 3 times
- User's timezone is UTC+8
- Project has test coverage requirement of 80%
- User mentioned deadline is next Friday
- ...100 more lines...
```

## Example Output
```
- User prefers TypeScript, uses React + Vite stack
- Authentication is a recurring topic (asked 3x)
- Project requirements: 80% test coverage, deadline next Friday
- User timezone: UTC+8
```
</system>
```

### 6.5 Prompt 配置最佳实践

#### 6.5.1 配置检查清单

在部署前确保：

```javascript
// History Summarization
✅ history_summary_params.prompt !== ""
✅ prompt 包含明确的任务描述
✅ prompt 指定输出格式（<summary></summary>）
✅ prompt 说明"I"代表 Agent
✅ trigger_on_history_size_chars > 0

// MEMORIES_COMPRESSION
✅ memories_params.compression_prompt !== ""
✅ compression_target 设置合理（建议 50-100 行）
✅ num_recent_memories_to_keep 设置合理（建议 10-20 条）
```

#### 6.5.2 避免的陷阱

1. **Prompt 为空**：
   ```javascript
   // ❌ 错误：忘记配置 prompt
   { "prompt": "" }  // 导致摘要功能完全禁用

   // ✅ 正确
   { "prompt": "Generate a concise summary of..." }
   ```

2. **缺少格式指示**：
   ```javascript
   // ❌ 错误：没有告诉 LLM 使用 <summary> 标签
   "Summarize the conversation."

   // ✅ 正确
   "Summarize the conversation. Wrap your output in <summary></summary> tags."
   ```

3. **过于宽泛**：
   ```javascript
   // ❌ 错误：太宽泛，LLM 不知道重点是什么
   "Tell me what happened."

   // ✅ 正确
   "Summarize: 1) user requirements, 2) agent actions, 3) key decisions, 4) pending tasks."
   ```

4. **忽略角色定义**：
   ```javascript
   // ❌ 错误：没有说明"I"代表谁
   "Summarize what I did."  // LLM 可能混淆 user 和 agent

   // ✅ 正确
   "Summarize what the Agent (you) did. Use first person ('I') to represent the Agent."
   ```

### 6.6 Prompt 系统总结

```
┌────────────────────────────────────────────────────────────────┐
│                    Augment Prompt 系统                          │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌──────────────────────┐      ┌──────────────────────────┐   │
│  │ History Summarization │      │  MEMORIES_COMPRESSION    │   │
│  ├──────────────────────┤      ├──────────────────────────┤   │
│  │ • 压缩 chatHistory    │      │ • 压缩 agentMemories     │   │
│  │ • 生成自然语言摘要     │      │ • 减少行数               │   │
│  │ • 配合 Abridged History│     │ • 保留最近 N 条          │   │
│  └──────────┬───────────┘      └──────────┬───────────────┘   │
│             │                             │                   │
│             ▼                             ▼                   │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │            Summary Node 请求模板                          │ │
│  │  <supervisor>                                            │ │
│  │    <abridged_history>...</abridged_history>              │ │
│  │    <summary>...</summary>                                │ │
│  │  </supervisor>                                           │ │
│  └──────────────────────────────────────────────────────────┘ │
│                             │                                 │
│                             ▼                                 │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │              LLM 使用双重信息继续对话                      │ │
│  │  • Abridged History: 结构化操作记录                       │ │
│  │  • Summary: 语义理解和上下文                              │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

**关键设计理念**：

1. **双重信息保留**：
   - Abridged History: 精确的操作记录（文件、命令）
   - Summary: 语义理解（意图、决策、上下文）

2. **模板 + LLM 生成**：
   - 模板：保证格式一致性
   - LLM 生成：保证语义连贯性

3. **分层压缩**：
   - L1: Tail 保留（最近的完整历史）
   - L2: Abridged History（结构化摘要）
   - L3: LLM Summary（自然语言摘要）

4. **可配置性**：
   - 所有 prompt 通过 Feature Flags 配置
   - 支持 A/B 测试和动态调整

---

## 7. 完整工作流程

### 7.1 Agent Loop 中的触发点

**文件位置**: `chunks.84.mjs:1421-1423`

```
┌──────────────────────────────────────────────────────────────────┐
│                    Agent Loop 迭代流程                            │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  for (iteration = 0; iteration < maxIterations; iteration++) {   │
│      │                                                           │
│      ├── 1. 检查中断                                             │
│      │   └── checkInterrupt("top of agent loop")                │
│      │                                                           │
│      ├── 2. 触发摘要检查 ←────── 关键触发点                       │
│      │   ├── if (chatHistorySummarizationModel?.isEnabled) {    │
│      │   │   └── await maybeAddHistorySummaryNode(false)        │
│      │   └── }                                                  │
│      │                                                           │
│      ├── 3. 开始新请求                                          │
│      │   └── state.beginRequest(createRequestId())              │
│      │                                                           │
│      ├── 4. 创建工作区快照                                       │
│      │   └── workspaceManager.createSnapshot()                  │
│      │                                                           │
│      ├── 5. 发送 Chat Stream                                    │
│      │   └── chatStreamWithRetries(requestId)                   │
│      │       └── apiServer.chatStream(..., chatHistoryForAPI)   │
│      │                    ↑                                      │
│      │           preprocessChatHistory() 在这里被调用            │
│      │                                                           │
│      ├── 6. 处理响应                                            │
│      │   └── state.pushResponseChunk(chunk)                     │
│      │                                                           │
│      ├── 7. 执行工具调用                                        │
│      │   └── executeSequentialTools() / executeParallelSubAgents()│
│      │                                                           │
│      └── 8. 检查是否结束                                        │
│          └── if (status === idle) return "end_turn"             │
│  }                                                               │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 7.2 摘要生成详细流程

```
┌───────────────────────────────────────────────────────────────────┐
│                 maybeAddHistorySummaryNode() 详细流程              │
├───────────────────────────────────────────────────────────────────┤
│                                                                   │
│  1. 前置检查                                                      │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ if (!prompt || prompt.trim() === "") return false;           │ │
│  │ if (threshold <= 0) return false;                            │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                          │                                        │
│                          ▼                                        │
│  2. 分割历史 Wtt()                                                │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ { head, tail } = Wtt(chatHistoryForAPI,                      │ │
│  │                      historyTailSizeCharsToExclude,          │ │
│  │                      threshold,                               │ │
│  │                      minTailExchanges=1)                     │ │
│  │                                                              │ │
│  │ if (head.length === 0) return false;  // 无需摘要            │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                          │                                        │
│                          ▼                                        │
│  3. 保留 Tool Nodes                                              │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ toolNodes = head.at(-1).response_nodes.filter(n => n.type===5)│ │
│  │ // Tool Use 节点需要保留到 Summary Node 中                    │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                          │                                        │
│                          ▼                                        │
│  4. 生成 Abridged History                                        │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ abridgedHistoryText = generateAbridgedHistoryText(tailFirstId)│ │
│  │ // 从 tail 开始往前累积，跳过 summary 节点                   │ │
│  │ // 限制：totalCharsLimit (默认 10K)                          │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                          │                                        │
│                          ▼                                        │
│  5. 截断 Head 历史                                               │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ truncatedHead = zee(head);                                   │ │
│  │ // 防止发送给 LLM 的历史过大                                 │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                          │                                        │
│                          ▼                                        │
│  6. 调用 LLM 生成摘要                                            │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ response = await sendSilentExchangeNonStreamingText(         │ │
│  │     this._params.prompt,  // 摘要 prompt（⚠️ 默认为空！）    │ │
│  │     true,                 // resetCheckpoint                 │ │
│  │     truncatedHead,        // 需要摘要的历史                  │ │
│  │     undefined,            // chatMode（使用默认值）          │ │
│  │     abortSignal                                              │ │
│  │ )                                                            │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                          │                                        │
│                          ▼                                        │
│  7. 构建 Summary Node                                            │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ summaryRequestMessage = template                             │ │
│  │     .replace("{summary}", "<summary>..." + responseText)     │ │
│  │     .replace("{abridged_history}", abridgedHistoryText)      │ │
│  │                                                              │ │
│  │ summaryNode = {                                              │ │
│  │     request_id,                                              │ │
│  │     request_nodes: [{ type: 0, text_node: { content } }],    │ │
│  │     response_nodes: [{ type: 0, content: "Ok..." }, ...toolNodes]│ │
│  │ }                                                            │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                          │                                        │
│                          ▼                                        │
│  8. 插入 Summary Node                                            │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ insertPosition = 找到 head 最后一个 exchange 的位置          │ │
│  │                                                              │ │
│  │ addExchangeToHistory(summaryNode, completed=true, {          │ │
│  │     isHistorySummary: true,                                  │ │
│  │     historySummaryVersion: 3                                 │ │
│  │ }, insertPosition)                                           │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                          │                                        │
│                          ▼                                        │
│  9. 返回成功                                                     │
│  └── return true                                                 │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

### 7.3 sendSilentExchange LLM 调用细节

**文件位置**: `chunks.84.mjs:1534-1558`

**⚠️ 重要发现：chatMode 不是 "CHAT"！**

```javascript
// sendSilentExchange 实现
async sendSilentExchange(message, isSilent, chatHistory, chatMode, abortSignal) {
    // 关键点 1: chatMode 默认值
    chatMode = chatMode ?? this.toolsModel.chatMode;  // ← 不是 "CHAT"！

    // 关键点 2: 工具定义加载
    let toolDefinitions = chatMode === "CHAT"
        ? []  // 仅 CHAT 模式不加载工具
        : this.toolsModel.sidecarToolHost.getToolDefinitions(chatMode);  // ← 会加载工具！

    return this.apiServer.chatStream(
        requestId,
        message,
        chatHistory,
        blobs,
        userGuidelines,
        workspaceGuidelines,
        toolDefinitions,  // ← 传递工具定义
        requestNodes,
        chatMode,
        agentMemories,
        rules,
        conversationId,
        abortSignal,
        this.state.systemPrompt,
        this.state.systemPromptReplacements
    );
}
```

**LLM 调用配置总结**：

| 配置项 | 实际值 | 说明 |
|--------|--------|------|
| `chatMode` | `this.toolsModel.chatMode` | **不是** "CHAT"，通常是 "AGENT" |
| `toolDefinitions` | 根据 chatMode 加载 | **会加载工具定义**（除非 chatMode === "CHAT"） |
| `prompt` | `this._params.prompt` | **默认为空字符串**，需通过 Feature Flags 配置 |
| `chatHistory` | `truncatedHead` | 经过 zee() 截断的 head 部分 |

**影响分析**：

1. **工具可用**: 摘要生成时 LLM 可以使用工具（如 View, Search）
2. **非 CHAT 模式**: 意味着可能会有不同的 System Prompt
3. **prompt 默认为空**: 如果 Feature Flags 未配置 prompt，摘要功能实际上被禁用

---

## 8. 配置参数详解

### 8.1 默认配置

**文件位置**: `chunks.84.mjs:704-735`

> ⚠️ **关键发现：History Summarization 默认是禁用的！**
>
> 1. `triggerOnHistorySizeChars = 0` → 阈值为 0 表示不触发
> 2. `prompt = ""` → **prompt 默认为空字符串**
>
> 即使 `isHistorySummaryEnabled = true`，如果 prompt 为空，`maybeAddHistorySummaryNode()` 会直接返回 false（参见 chunks.84.mjs:1026-1028）。
>
> **生产环境必须通过 Feature Flags 配置实际的 prompt 值！**

```javascript
jp = {
    // 触发条件
    triggerOnHistorySizeChars: 0,                    // 默认禁用
    historyTailSizeCharsToExclude: 0,                // 不保留 tail
    triggerOnHistorySizeCharsWhenCacheExpiring: 0,   // 缓存过期触发禁用

    // 摘要生成
    prompt: "",                                       // ⚠️ 默认为空！需要配置

    // 缓存管理
    cacheTTLMs: 0,                                   // 缓存 TTL
    bufferTimeBeforeCacheExpirationMs: 0,           // 缓冲时间

    // Summary Node 模板
    summaryNodeRequestMessageTemplate: `<supervisor>...</supervisor>`,
    summaryNodeResponseMessage: "Ok. I will continue the conversation from this point.",

    // Abridged History 参数
    abridgedHistoryParams: {
        totalCharsLimit: 10000,           // 总字符限制 (10K)
        userMessageCharsLimit: 1000,      // 用户消息限制 (1K)
        agentResponseCharsLimit: 2000,    // Agent 响应限制 (2K)
        actionCharsLimit: 200,            // 文件路径/命令限制 (200)

        // 文件操作数量限制
        numFilesModifiedLimit: 10,
        numFilesCreatedLimit: 10,
        numFilesDeletedLimit: 10,
        numFilesViewedLimit: 10,
        numTerminalCommandsLimit: 10
    }
};
```

### 8.2 配置解析

**文件位置**: `chunks.84.mjs:779-814`

```javascript
function mGt(historySummaryParams) {
    if (!historySummaryParams) {
        logger.info("historySummaryParams is empty. Using default params");
        return jp;  // 返回默认值
    }

    let parsed = JSON.parse(historySummaryParams);

    let config = {
        // 从 JSON 解析，使用 snake_case → camelCase 转换
        triggerOnHistorySizeChars:
            parsed.trigger_on_history_size_chars ?? jp.triggerOnHistorySizeChars,
        historyTailSizeCharsToExclude:
            parsed.history_tail_size_chars_to_exclude ?? jp.historyTailSizeCharsToExclude,
        // ... 其他参数类似
    };

    // 验证模板
    if (!config.summaryNodeRequestMessageTemplate.includes("{summary}")) {
        logger.error("template must contain {summary}");
        config.summaryNodeRequestMessageTemplate = jp.summaryNodeRequestMessageTemplate;
    }

    return config;
}
```

---

## 9. UI 层 Compact 模式

### 9.1 CLI 参数

**文件位置**: `chunks.58.mjs:36-49`

```javascript
// --compact 参数定义
t.addOption(new Option(
    "--compact",
    "Enable compact output mode. Tool calls, tool results, and intermediate " +
    "assistant messages are shown in a single line each. The last assistant " +
    "message in a turn gets shown in full."
).hideHelp())

// Verbosity 配置
buildConfig(opts) {
    let verbosity;

    if (opts.quiet) {
        verbosity = "quiet";      // 最简模式
    } else if (opts.compact) {
        verbosity = "compact";    // 压缩模式
    } else {
        verbosity = "default";    // 完整模式
    }

    return { verbosity, ... };
}
```

### 9.2 Verbosity 模式对比

| 模式 | 工具调用显示 | 工具结果显示 | 中间响应 | 最终响应 | 使用场景 |
|------|-------------|-------------|---------|---------|---------|
| `default` | 完整 | 完整 | 完整 | 完整 | 交互式开发 |
| `compact` | 单行 | 单行 | 单行 | 完整 | 减少滚动 |
| `quiet` | 隐藏 | 隐藏 | 隐藏 | 仅最终 | CI/CD 脚本 |

---

## 10. 关键代码位置总结

| 功能 | 文件 | 行号 | 说明 |
|------|------|------|------|
| 默认参数 `jp` | chunks.84.mjs | 704-735 | 摘要配置默认值 (⚠️ prompt 默认为空) |
| 参数解析 `mGt()` | chunks.84.mjs | 779-814 | JSON → 配置对象 |
| 截断函数 `lpe()` | chunks.84.mjs | 737-748 | 中间截断算法 |
| Abridged History 模板 | chunks.84.mjs | 834-886 | Handlebars 模板 |
| 工具调用提取 `VHn()` | chunks.84.mjs | 925-947 | 从 tool_use 提取操作 |
| **ChatHistorySummarizationModel** | chunks.84.mjs | 976-1107 | **核心摘要类** |
| `generateAbridgedHistoryText()` | chunks.84.mjs | 992-1009 | 生成简化历史 |
| `maybeScheduleSummarization()` | chunks.84.mjs | 1010-1016 | 缓存触发调度 (⚠️ 实际未被调用) |
| `preprocessChatHistory()` | chunks.84.mjs | 1017-1025 | 预处理历史 (slice(n) 保留 Summary) |
| `maybeAddHistorySummaryNode()` | chunks.84.mjs | 1026-1106 | **主要摘要生成** |
| `isHistorySummaryEnabled` 计算 | chunks.84.mjs | 1198-1204 | 启用条件判断 |
| `isCodeReviewBot` 判断 | chunks.84.mjs | 1199 | Bot 类型检查 |
| 历史分割 `Wtt()` | chunks.61.mjs | 1243-1271 | head/tail 分割 |
| 大小计算 `Jee()` | chunks.61.mjs | 1234-1236 | 总字符数计算 |
| Exchange 大小 `Ftt()` | chunks.61.mjs | 1238-1241 | 单条 exchange 大小 |
| 二次截断 `zee()` | chunks.61.mjs | 1220-1232 | 800KB 限制截断 |
| `chatHistoryForAPI` getter | chunks.61.mjs | 1421-1432 | API 请求时的历史 |
| `addExchangeToHistory()` | chunks.61.mjs | 1445-1467 | 添加 exchange |
| **sequenceId 插值计算** | chunks.61.mjs | 1447-1454 | Summary Node 位置计算 |
| Agent Loop 触发点 | chunks.84.mjs | 1421-1423 | 每次迭代前检查 |
| `sendSilentExchange()` | chunks.84.mjs | 1534-1558 | 静默 LLM 调用 (chatMode 默认非 CHAT) |
| `sendSilentExchangeNonStreamingText()` | chunks.84.mjs | 1559-1581 | 非流式文本响应 |
| Feature Flags 解析 | chunks.72.mjs | 1072 | snake_case → camelCase |
| **MEMORIES_COMPRESSION** | chunks.77.mjs | 1383-1409 | Agent 记忆压缩 (区别于历史摘要) |
| `--compact` CLI | chunks.58.mjs | 36 | UI 压缩参数 |
| Verbosity 配置 | chunks.58.mjs | 44-49 | 输出模式选择 |

---

## 11. 与 MEMORIES_COMPRESSION 的区别

Augment 中存在**两种不同的压缩机制**，容易混淆：

### 11.1 功能对比表

| 特性 | History Summarization | MEMORIES_COMPRESSION |
|------|----------------------|----------------------|
| **目的** | 压缩对话历史 | 压缩长期 Agent 记忆 |
| **数据源** | `chatHistory` | `agentMemories` |
| **触发条件** | 历史大小超过阈值 | 记忆行数超过限制 |
| **Chat Mode** | 系统默认（非 "CHAT"） | 显式 "MEMORIES_COMPRESSION" |
| **Prompt 来源** | `historySummaryParams.prompt` | `memoriesParams.compression_prompt` |
| **输出位置** | 插入 Summary Node 到历史 | 更新 Agent Memories |
| **代码位置** | `chunks.84.mjs:1026-1106` | `chunks.77.mjs:1383-1409` |

### 11.2 MEMORIES_COMPRESSION 实现概览

**文件位置**: `chunks.77.mjs:1383-1409`

```javascript
// MEMORIES_COMPRESSION 触发条件
async maybeCompressMemories() {
    let memories = this.agentState.agentMemories;

    // 行数超过限制时触发
    if (memories.split('\n').length > this.memoriesParams.maxLines) {
        let compressed = await this.compressMemories(memories);
        this.agentState.setAgentMemories(compressed);
    }
}

// 压缩调用
async compressMemories(memories) {
    return await this.sendSilentExchangeNonStreamingText(
        this.memoriesParams.compressionPrompt,  // ← 专用 prompt
        false,
        [],  // 空历史
        "MEMORIES_COMPRESSION",  // ← 显式指定 chatMode
        abortSignal
    );
}
```

### 11.3 关系图

```
┌──────────────────────────────────────────────────────────────────┐
│                     Augment 压缩机制                              │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────┐    ┌─────────────────────────────┐  │
│  │  History Summarization   │    │    MEMORIES_COMPRESSION     │  │
│  ├─────────────────────────┤    ├─────────────────────────────┤  │
│  │ • 压缩 chatHistory       │    │ • 压缩 agentMemories        │  │
│  │ • 触发: 字符数阈值       │    │ • 触发: 行数限制            │  │
│  │ • 生成 Summary Node      │    │ • 生成压缩后的 memories     │  │
│  │ • 保留结构化 + 摘要       │    │ • 完全替换原 memories       │  │
│  └──────────┬──────────────┘    └──────────┬──────────────────┘  │
│             │                              │                     │
│             └──────────┬───────────────────┘                     │
│                        │                                         │
│                        ▼                                         │
│              ┌──────────────────┐                                │
│              │ sendSilentExchange│                                │
│              │ (共用 LLM 调用)   │                                │
│              └──────────────────┘                                │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 11.4 重要区别

1. **History Summarization 是"替换式"**：旧 Summary 会被新 Summary 替换
2. **MEMORIES_COMPRESSION 是"重写式"**：整个 memories 被 LLM 重写
3. **chatMode 不同**：
   - History Summarization: 使用系统默认 chatMode（可以使用工具）
   - MEMORIES_COMPRESSION: 显式指定，可能有专用工具集

---

## 12. 与其他系统对比

| 特性 | Augment | Claude Code | Cursor | Cody |
|------|---------|-------------|--------|------|
| **压缩触发** | 字符数阈值 + 缓存过期 | 自动（上下文满） | 滑动窗口 | Token 阈值 |
| **压缩方式** | LLM 摘要生成 | LLM 摘要 | 截断 | LLM 摘要 |
| **多级压缩** | ❌ 单级替换 | ❌ | ❌ | ❌ |
| **Abridged History** | ✅ 结构化 XML | ❌ | ❌ | ❌ |
| **版本控制** | ✅ v3 | ❌ | ❌ | ❌ |
| **缓存集成** | ✅ Prompt Cache | ✅ | ❌ | ❌ |
| **Tool Nodes 保留** | ✅ | ❌ | ❌ | ❌ |
| **可配置性** | ✅ 详细参数 | ⚠️ 有限 | ⚠️ 有限 | ⚠️ 有限 |
| **默认启用** | ❌ | ✅ | ✅ | ✅ |

---

## 13. 设计分析与评价

### 13.1 优点

1. **精细的保留策略**: 通过多参数控制（tail 大小、最小保留数、字符限制）平衡摘要质量和信息保留

2. **双重信息保留**: Abridged History + LLM Summary 互补
   - Abridged History: 结构化、可审计
   - LLM Summary: 语义理解、上下文连贯

3. **版本控制机制**: 支持摘要格式升级，旧版本自动过滤

4. **Tool Nodes 迁移**: 最后一个 exchange 的工具调用被保留到 Summary Node

5. **缓存集成**: 与 Prompt Cache 配合，在缓存即将过期时主动触发摘要

### 13.2 潜在问题

1. **信息丢失风险**:
   - 多次 Compact 后，早期历史完全消失
   - 依赖 LLM 摘要质量

2. **默认禁用**:
   - `triggerOnHistorySizeChars = 0` 表示默认不启用
   - `prompt = ""` 默认为空字符串
   - 必须通过 Feature Flags 配置才能启用

3. **无嵌套摘要**:
   - 旧 Summary 被丢弃，不会被包含在新 Summary 中
   - 长对话可能丢失重要早期上下文

4. **截断策略简单**:
   - `zee()` 函数使用固定 800KB/3 段划分
   - 可能误删重要内容

5. **maybeScheduleSummarization 未被使用**:
   - 缓存过期触发机制代码存在但未实际调用
   - 只有主动触发（每次迭代前检查）在工作

### 13.3 改进建议

1. **嵌套摘要**: 将旧 Summary 内容作为新摘要的输入之一

2. **重要性评分**: 对 exchange 进行重要性评分，优先保留关键对话

3. **增量摘要**: 只摘要新增部分，复用旧摘要

4. **用户控制**: 允许用户标记"重要"的对话不被摘要

---

**创建时间**: 2025-12-04
**最后更新**: 2025-12-05
**分析状态**: ✅ 深度分析完成（含技术实现验证 + Prompt 系统详解）
**版本**: v2.2 (新增完整 Prompt 系统详解章节)
