# Plan Mode 实现对比分析：Claude Code vs OpenCode

本文档深入分析 Claude Code（Flag-based）和 OpenCode（Subagent-based）两种 Plan Mode 实现方式的架构差异、上下文利用效率、安全性和用户体验。

---

## 1. 架构对比概览

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Claude Code: Flag-based Plan Mode                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Main Agent                                                                 │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                                                                      │   │
│   │   is_plan_mode = true/false  ◄── 单一会话，Flag 切换                │   │
│   │   ┌──────────────────────────────────────────────────────────────┐  │   │
│   │   │                                                               │  │   │
│   │   │  Normal Mode:           Plan Mode:                           │  │   │
│   │   │  - All tools enabled    - All tools STILL enabled            │  │   │
│   │   │  - Normal prompts       - PlanModeGenerator injects          │  │   │
│   │   │                           "MUST NOT make edits" prompt       │  │   │
│   │   │                         - Relies on LLM compliance           │  │   │
│   │   │                                                               │  │   │
│   │   └──────────────────────────────────────────────────────────────┘  │   │
│   │                                                                      │   │
│   │   Context: [User Messages] + [Assistant Messages] + [Tool Results]  │   │
│   │            └─────────────── 共享上下文 ────────────────────────────┘  │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                    OpenCode: Subagent-based Plan Mode                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   ┌─────────────────────────────┐    ┌─────────────────────────────┐       │
│   │      Build Agent (Primary)  │    │      Plan Agent (Primary)   │       │
│   │                             │    │                             │       │
│   │   Tools:                    │    │   Tools:                    │       │
│   │   - edit: ✓                 │    │   - edit: ✗ (deny)          │       │
│   │   - write: ✓                │    │   - write: ✗ (deny)         │       │
│   │   - bash: ✓ (all)           │    │   - bash: whitelist only    │       │
│   │   - read: ✓                 │    │   - read: ✓                 │       │
│   │   - glob: ✓                 │    │   - glob: ✓                 │       │
│   │   - grep: ✓                 │    │   - grep: ✓                 │       │
│   │                             │    │                             │       │
│   │   Session A                 │    │   Session A (same)          │       │
│   │   ├── Message History ◄─────┼────┼── 共享消息历史                │       │
│   │   └── Token Count           │    │                             │       │
│   │                             │    │                             │       │
│   └─────────────────────────────┘    └─────────────────────────────┘       │
│                │                                  │                         │
│                └──────────── Agent Switch ────────┘                         │
│                         (User Selection)                                    │
│                              │                                              │
│                              ▼                                              │
│                   Build-Switch Reminder                                     │
│                   注入切换提醒到上下文                                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. 核心实现差异

### 2.1 Claude Code (Codex) 实现

**核心机制：Prompt Injection (软限制)**

```rust
// core/src/system_reminder/attachments/plan_mode.rs

impl PlanModeGenerator {
    fn build_main_agent_content(&self, ctx: &GeneratorContext<'_>) -> String {
        format!(
            "Plan mode is active. The user indicated that they do not want you to execute yet -- \
             you MUST NOT make any edits (with the exception of the plan file mentioned below), \
             run any non-readonly tools (including changing configs or making commits), \
             or otherwise make any changes to the system. This supercedes any other instructions \
             you have received.\n\n\
             {plan_file_info}\n\
             You should build your plan incrementally by writing to or editing this file. \
             NOTE that this is the only file you are allowed to edit..."
        )
    }
}
```

**特点：**
1. **工具仍然可用** - 所有工具在技术上仍可调用
2. **依赖 LLM 遵守** - 通过提示词约束行为
3. **Plan 文件例外** - 允许编辑 plan 文件
4. **同一会话上下文** - Flag 切换，不切换会话

**状态管理：**

```rust
// GeneratorContext 中的 Plan Mode 相关字段
pub struct GeneratorContext<'a> {
    pub is_plan_mode: bool,           // 是否激活 Plan Mode
    pub plan_file_path: Option<&'a str>, // Plan 文件路径
    pub is_plan_reentry: bool,        // 是否重入 Plan Mode
    pub plan_state: &'a PlanState,    // Plan 状态跟踪
    // ...
}
```

### 2.2 OpenCode 实现

**核心机制：Permission Control (硬限制)**

```typescript
// packages/opencode/src/agent/agent.ts

plan: {
  name: "plan",
  mode: "primary",
  native: true,
  tools: { ...defaultTools },
  permission: {
    edit: "deny",                    // ⭐ 工具级别禁用
    bash: {
      "git diff*": "allow",
      "git log*": "allow",
      "find *": "allow",
      "grep*": "allow",
      "ls*": "allow",
      "*": "ask",                    // 其他命令需确认
    },
    webfetch: "allow",
  },
}
```

**工具过滤：**

```typescript
// packages/opencode/src/tool/registry.ts:139-160

export async function enabled(agent: Agent.Info): Promise<Record<string, boolean>> {
  const result: Record<string, boolean> = {}

  // Plan agent: edit="deny" → 禁用 edit/write 工具
  if (agent.permission.edit === "deny") {
    result["edit"] = false      // ⭐ 工具从列表中移除
    result["write"] = false
  }

  return result
}
```

---

## 3. 上下文 (Context) 利用效率分析

### 3.1 Token 消耗对比

| 场景 | Claude Code | OpenCode | 差异 |
|------|------------|----------|------|
| **Plan Mode 激活** | +200-400 tokens (PlanModeGenerator) | +0 tokens (权限配置) | Claude Code 额外消耗 |
| **重入 Plan Mode** | +300-500 tokens (Reentry + Main) | +0 tokens | Claude Code 额外消耗 |
| **模式切换提醒** | +0 tokens | +100-200 tokens (build-switch.txt) | OpenCode 额外消耗 |
| **工具列表** | 全量工具 (~500 tokens) | 精简工具 (~300 tokens) | OpenCode 节省 ~200 tokens |

**Token 效率公式：**

```
Claude Code:
  每轮 = Base + PlanModeReminder(~300) + AllTools(~500)
  总计 ≈ Base + 800 tokens overhead

OpenCode:
  每轮 = Base + ReducedTools(~300) + SwitchReminder(once, ~150)
  总计 ≈ Base + 300 tokens (切换时 +150)
```

**结论：OpenCode 在长会话中 token 效率更高，每轮节省 ~500 tokens。**

### 3.2 上下文连续性

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Claude Code: 完全连续的上下文                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Turn 1 (Normal) → Turn 2 (Plan) → Turn 3 (Plan) → Turn 4 (Normal)         │
│        │                │                │                │                  │
│        └────────────────┴────────────────┴────────────────┘                  │
│                         所有消息共享同一上下文                                  │
│                         LLM 完全记住之前的讨论                                  │
│                                                                              │
│   优势：                                                                      │
│   - 无需重复上下文                                                            │
│   - Plan 可以引用之前的讨论细节                                                │
│   - 切换无感知延迟                                                            │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                    OpenCode: 共享会话但不同 Agent                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Turn 1 (Build) → Turn 2 (Plan) → Turn 3 (Plan) → Turn 4 (Build)           │
│        │                │                │                │                  │
│        │           ┌────┴────┐      ┌────┴────┐           │                  │
│        │           │ Plan    │      │ Plan    │           │                  │
│        │           │ Agent   │      │ Agent   │           │                  │
│        │           └─────────┘      └─────────┘           │                  │
│        │                                                   │                  │
│        └───────────────────────────────────────────────────┘                  │
│                         消息历史共享，但权限配置不同                            │
│                         切换时注入 build-switch.txt                           │
│                                                                              │
│   优势：                                                                      │
│   - 消息历史仍然共享                                                          │
│   - 权限实际生效，不依赖 LLM 遵守                                              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 信息保留对比

| 信息类型 | Claude Code | OpenCode |
|---------|-------------|----------|
| 用户意图 | ✓ 完整保留 | ✓ 完整保留 |
| 代码分析结果 | ✓ 完整保留 | ✓ 完整保留 |
| Plan 内容 | ✓ 在 plan 文件中 | ✓ 在 plan 文件中 |
| 工具调用历史 | ✓ 完整保留 | ✓ 完整保留 |
| 切换上下文 | ✓ 隐式（Flag 变化） | ✓ 显式（Reminder 注入） |

**结论：两者在信息保留上基本等效，OpenCode 通过 build-switch.txt 显式提醒切换。**

---

## 4. 安全性分析

### 4.1 限制强度对比

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         限制强度层次                                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Level 1: Prompt Instruction (Claude Code)                                  │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  "You MUST NOT make any edits..."                                    │   │
│   │                                                                      │   │
│   │  ⚠️  可被绕过：                                                       │   │
│   │  - Jailbreak prompts                                                 │   │
│   │  - Model hallucination                                               │   │
│   │  - Ambiguous interpretation                                          │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   Level 2: Permission Check (OpenCode)                                       │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  permission.edit === "deny" → result["edit"] = false                 │   │
│   │                                                                      │   │
│   │  ✓ 工具从列表中移除                                                   │   │
│   │  ⚠️ 可被绕过：                                                        │   │
│   │  - 如果工具列表传递给 LLM 后仍可调用                                   │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   Level 3: Runtime Enforcement (理想状态)                                     │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  工具调用时强制检查权限                                                │   │
│   │                                                                      │   │
│   │  ✓ 即使 LLM 尝试调用，也会被拒绝                                       │   │
│   │  ✓ 最强安全保障                                                       │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 安全性评分

| 维度 | Claude Code | OpenCode | 说明 |
|------|-------------|----------|------|
| **防误操作** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | OpenCode 工具级禁用更可靠 |
| **防 Jailbreak** | ⭐⭐ | ⭐⭐⭐⭐ | Claude Code 依赖 LLM 遵守 |
| **Bash 限制** | ⭐⭐ (仅提示) | ⭐⭐⭐⭐⭐ (模式匹配) | OpenCode 有细粒度控制 |
| **审计追踪** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | 两者都可记录 |

### 4.3 Bash 命令控制对比

**Claude Code：提示级限制**

```rust
// 仅通过提示词约束
"run any non-readonly tools (including changing configs or making commits)"
// ⚠️ LLM 可能仍然执行 `rm -rf` 如果被误导
```

**OpenCode：模式匹配限制**

```typescript
// 使用 minimatch 精确控制
permission.bash = {
  "git diff*": "allow",
  "git log*": "allow",
  "find * -delete*": "ask",   // 删除需确认
  "rm*": "deny",              // 强制拒绝
  "*": "ask",                 // 其他需确认
}
```

---

## 5. 用户体验对比

### 5.1 切换流程

**Claude Code:**
```
User: "进入 plan mode"
→ System: 设置 is_plan_mode = true
→ System: PlanModeGenerator 注入提示词
→ LLM: 开始遵循只读约束

User: 调用 ExitPlanMode 工具
→ System: 设置 is_plan_mode = false
→ LLM: 恢复正常模式
```

**OpenCode:**
```
User: 点击 Agent 切换按钮，选择 "plan"
→ System: 切换到 Plan Agent
→ System: 应用 Plan Agent 权限配置
→ LLM: 使用受限工具集

User: 点击切换回 "build"
→ System: 切换到 Build Agent
→ System: 注入 build-switch.txt 提醒
→ LLM: 使用完整工具集，记住之前的 plan
```

### 5.2 体验评分

| 维度 | Claude Code | OpenCode | 说明 |
|------|-------------|----------|------|
| **切换速度** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | Claude Code 无需加载新 Agent |
| **视觉反馈** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | OpenCode 有明确的 Agent 标识/颜色 |
| **可预测性** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | OpenCode 工具列表直接变化 |
| **学习曲线** | ⭐⭐⭐⭐ | ⭐⭐⭐ | Claude Code 更简单（一个命令） |

---

## 6. 性能对比

### 6.1 延迟分析

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         响应延迟对比                                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Claude Code:                                                               │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  User Input                                                          │   │
│   │      │                                                               │   │
│   │      ▼                                                               │   │
│   │  Check is_plan_mode flag (< 1ms)                                     │   │
│   │      │                                                               │   │
│   │      ▼                                                               │   │
│   │  Generate PlanModeReminder (~5ms)                                    │   │
│   │      │                                                               │   │
│   │      ▼                                                               │   │
│   │  Send to LLM (with larger context)                                   │   │
│   │                                                                      │   │
│   │  总延迟: ~6ms + LLM(额外 tokens)                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   OpenCode:                                                                  │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  User Input                                                          │   │
│   │      │                                                               │   │
│   │      ▼                                                               │   │
│   │  Load Agent Config (~2ms)                                            │   │
│   │      │                                                               │   │
│   │      ▼                                                               │   │
│   │  Filter Tools by Permission (~3ms)                                   │   │
│   │      │                                                               │   │
│   │      ▼                                                               │   │
│   │  Send to LLM (with smaller tool list)                                │   │
│   │                                                                      │   │
│   │  总延迟: ~5ms + LLM(较少 tokens)                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 内存使用

| 维度 | Claude Code | OpenCode | 说明 |
|------|-------------|----------|------|
| **Agent 配置** | 1 份 | 多份 (build, plan, explore...) | OpenCode 多 Agent 占用更多 |
| **会话状态** | 1 个会话 | 1 个会话 (共享) | 相同 |
| **工具实例** | 全量缓存 | 按需过滤 | 相同 |

---

## 7. 优缺点总结

### 7.1 Claude Code (Flag-based)

**优点：**

| 优点 | 说明 |
|------|------|
| ✅ **架构简洁** | 单一 Agent，通过 Flag 切换模式 |
| ✅ **上下文完全连续** | 无需任何上下文传递，天然共享 |
| ✅ **切换零延迟** | 仅修改 Flag，无需加载新配置 |
| ✅ **实现成本低** | 只需添加一个 Generator |
| ✅ **灵活例外** | 可以允许编辑特定文件（plan 文件） |

**缺点：**

| 缺点 | 说明 |
|------|------|
| ❌ **软限制依赖 LLM** | 工具仍可被调用，依赖模型遵守指令 |
| ❌ **Token 开销大** | 每轮都注入 PlanMode 提示词（~300 tokens） |
| ❌ **Bash 无细粒度控制** | 只能通过提示词"建议"使用只读命令 |
| ❌ **可被 Jailbreak** | 恶意提示可能绕过限制 |
| ❌ **工具列表膨胀** | 即使禁用也传递给 LLM，消耗 tokens |

### 7.2 OpenCode (Subagent-based)

**优点：**

| 优点 | 说明 |
|------|------|
| ✅ **硬限制更安全** | 工具从列表中移除，无法调用 |
| ✅ **Bash 细粒度控制** | minimatch 模式匹配精确控制命令 |
| ✅ **Token 效率高** | 工具列表更小，无额外提示词开销 |
| ✅ **视觉明确** | Agent 切换有明确 UI 反馈 |
| ✅ **扩展性好** | 可轻松添加更多 Agent 类型 |

**缺点：**

| 缺点 | 说明 |
|------|------|
| ❌ **架构复杂** | 多 Agent 配置，权限系统，过滤逻辑 |
| ❌ **切换有感知** | 需要用户显式选择 Agent |
| ❌ **切换需 Reminder** | 需要 build-switch.txt 帮助 LLM 记住计划 |
| ❌ **配置维护成本** | Bash 白名单需要持续维护 |

---

## 8. 建议方案：混合模式

结合两者优点，设计 **Hybrid Plan Mode**：

```rust
// 混合方案设计

pub struct HybridPlanMode {
    // 1. Flag 控制（来自 Claude Code）
    is_plan_mode: bool,
    plan_file_path: Option<String>,

    // 2. 权限控制（来自 OpenCode）
    tool_permissions: HashMap<String, Permission>,
    bash_patterns: Vec<(String, Permission)>,
}

impl HybridPlanMode {
    /// 进入 Plan Mode
    pub fn enter(&mut self, plan_file: &str) {
        self.is_plan_mode = true;
        self.plan_file_path = Some(plan_file.to_string());

        // 硬限制：禁用危险工具
        self.tool_permissions.insert("edit".into(), Permission::DenyExcept(plan_file));
        self.tool_permissions.insert("write".into(), Permission::DenyExcept(plan_file));

        // Bash 白名单
        self.bash_patterns = vec![
            ("git diff*".into(), Permission::Allow),
            ("git log*".into(), Permission::Allow),
            ("ls*".into(), Permission::Allow),
            ("find * -delete*".into(), Permission::Ask),
            ("rm*".into(), Permission::Deny),
            ("*".into(), Permission::Ask),
        ];
    }

    /// 工具过滤（运行时检查）
    pub fn filter_tool_call(&self, tool: &str, args: &Value) -> ToolCallResult {
        match self.tool_permissions.get(tool) {
            Some(Permission::DenyExcept(allowed_path)) => {
                if self.is_targeting_path(args, allowed_path) {
                    ToolCallResult::Allow
                } else {
                    ToolCallResult::Deny("Plan mode: only plan file edits allowed")
                }
            }
            Some(Permission::Deny) => {
                ToolCallResult::Deny("Plan mode: tool disabled")
            }
            _ => ToolCallResult::Allow
        }
    }
}
```

**混合方案优势：**

| 特性 | 来源 | 效果 |
|------|------|------|
| 单一会话 | Claude Code | 上下文完全连续 |
| 运行时权限检查 | OpenCode | 硬限制不依赖 LLM |
| Bash 模式匹配 | OpenCode | 细粒度命令控制 |
| Plan 文件例外 | Claude Code | 灵活编辑 plan |
| 精简工具列表 | OpenCode | Token 效率 |
| Flag 切换 | Claude Code | 零延迟切换 |

---

## 9. 实现建议

### 9.1 对 Codex 的改进建议

```rust
// 1. 添加运行时工具调用检查
// core/src/tools/handlers/mod.rs

pub async fn execute_tool(
    tool_name: &str,
    args: Value,
    ctx: &ExecutionContext,
) -> Result<ToolResult> {
    // ⭐ 新增：Plan Mode 运行时检查
    if ctx.is_plan_mode {
        if let Err(reason) = ctx.plan_mode.filter_tool_call(tool_name, &args) {
            return Err(CodexErr::PlanModeViolation(reason));
        }
    }

    // 继续正常执行...
}

// 2. 添加 Bash 模式匹配
// core/src/command_safety/plan_mode_commands.rs

pub fn check_plan_mode_bash(command: &str) -> Permission {
    static PATTERNS: &[(&str, Permission)] = &[
        ("git diff*", Permission::Allow),
        ("git log*", Permission::Allow),
        ("find * -delete*", Permission::Ask),
        ("rm*", Permission::Deny),
        // ...
    ];

    for (pattern, permission) in PATTERNS {
        if glob_match(pattern, command) {
            return *permission;
        }
    }
    Permission::Ask  // 默认询问
}

// 3. 精简工具列表（可选）
// 在 plan mode 下不传递 edit/write 工具定义给 LLM
```

### 9.2 关键文件修改

| 文件 | 修改内容 |
|------|----------|
| `core/src/tools/handlers/mod.rs` | 添加运行时权限检查 |
| `core/src/command_safety/mod.rs` | 添加 plan_mode_commands 模块 |
| `core/src/system_reminder/attachments/plan_mode.rs` | 保持现有 prompt 逻辑 |
| `core/src/config/mod.rs` | 添加 PlanModeConfig |

---

## 10. 结论

| 维度 | 推荐方案 | 理由 |
|------|----------|------|
| **安全性** | OpenCode | 硬限制更可靠 |
| **Token 效率** | OpenCode | 长会话节省 ~500 tokens/轮 |
| **上下文连续性** | Claude Code | 天然共享，无需处理 |
| **实现简洁性** | Claude Code | 单 Agent，低维护成本 |
| **用户体验** | Claude Code | 切换更流畅 |
| **扩展性** | OpenCode | 多 Agent 架构更灵活 |

**最终建议：采用混合方案**

1. 保持 Claude Code 的单会话 Flag 架构（简洁、上下文连续）
2. 添加 OpenCode 的运行时权限检查（安全、可靠）
3. 添加 Bash 命令模式匹配（细粒度控制）
4. 可选：在 Plan Mode 下精简工具列表（Token 效率）

这样可以在保持用户体验的同时，显著提升安全性和效率。

---

*文档生成时间: 2025-12-28*
*基于 Claude Code 和 OpenCode 源码分析*
