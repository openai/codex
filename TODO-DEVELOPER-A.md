# 🔵 Developer A: Core Execution Engine (Backend Focus)

## Your Mission

Implement the core hook execution engine, integrate with Codex's event system, and build comprehensive testing infrastructure.

## 🎯 Your Responsibilities

- Hook execution coordination and management
- Event system integration with existing Codex architecture
- Core testing infrastructure and validation
- Performance optimization and error handling

## 📁 Your Primary Files

- `codex-rs/core/src/hooks/manager.rs` ⭐ **Your main file**
- `codex-rs/core/src/hooks/executor.rs` ⭐ **Your main file**
- `codex-rs/core/src/hooks/executors/` (new directory) ⭐ **Create this**
- `codex-rs/core/src/protocol.rs`
- `codex-rs/core/src/codex.rs`
- `codex-rs/core/src/agent.rs`
- `codex-rs/exec/src/event_processor.rs`

## 🚀 Start Here: Phase 2.1 - Core Hook Manager

### 🔴 HIGH PRIORITY: Complete Hook Manager Implementation

**File**: `codex-rs/core/src/hooks/manager.rs`

#### Current Status

The file exists with basic structure but needs complete implementation.

#### Your Tasks

- [ ] **Hook Execution Coordination**

  - Implement the `trigger_event` method to actually execute hooks
  - Add hook filtering based on event type and conditions
  - Coordinate execution of multiple hooks for the same event

- [ ] **Event Subscription and Routing**

  - Create event subscription mechanism
  - Route events to appropriate hooks based on registry
  - Handle event filtering and matching

- [ ] **Error Handling and Logging**

  - Implement comprehensive error handling for hook failures
  - Add structured logging for hook execution
  - Handle partial failures gracefully

- [ ] **Performance Monitoring**
  - Add execution time tracking
  - Implement hook execution metrics
  - Monitor resource usage

#### Implementation Guide

```rust
// In manager.rs - implement this method
impl HookManager {
    pub async fn trigger_event(&self, event: LifecycleEvent) -> Result<(), HookError> {
        if !self.config.hooks.enabled {
            return Ok(());
        }

        // 1. Get matching hooks from registry
        let context = HookContext::new(event.clone(), /* working_dir */);
        let hooks = self.registry.get_matching_hooks(&event, &context)?;

        // 2. Execute hooks based on priority and mode
        // 3. Handle errors and collect results
        // 4. Log execution metrics

        // TODO: Your implementation here
    }
}
```

---

## 📋 Your Complete Task List

### 🔄 Phase 2: Hook Execution Engine

#### 2.1 Core Hook Manager ⭐ **START HERE**

- [ ] Complete hook execution coordination in `manager.rs`
- [ ] Implement event subscription and routing
- [ ] Add error handling and logging
- [ ] Performance monitoring and metrics

#### 2.2 Hook Executor Framework

- [ ] Complete timeout management and cancellation in `executor.rs`
- [ ] Implement error isolation and recovery
- [ ] Add execution mode support (blocking/non-blocking, parallel/sequential)
- [ ] Hook execution result aggregation

#### 2.3 Hook Executor Implementations

- [ ] Create `codex-rs/core/src/hooks/executors/mod.rs`
- [ ] Implement `ScriptExecutor` in `executors/script.rs`
- [ ] Implement `WebhookExecutor` in `executors/webhook.rs`
- [ ] Implement `McpToolExecutor` in `executors/mcp.rs`

### 🔄 Phase 3: Event System Integration

#### 3.1 Protocol Extensions

- [ ] Add lifecycle event types to `protocol.rs`
- [ ] Add hook execution events for monitoring
- [ ] Update event serialization/deserialization

#### 3.2 Core Integration Points

- [ ] Integrate hook manager in `codex.rs`
- [ ] Add hook trigger points in `agent.rs`
- [ ] Session and task lifecycle hooks

#### 3.3 Execution Integration

- [ ] Add hook execution to `event_processor.rs`
- [ ] Command execution hooks (before/after)
- [ ] Patch application hooks (before/after)
- [ ] MCP tool execution hooks

### 🔄 Phase 6: Testing and Validation

#### 6.1 Unit Tests

- [ ] Test hook execution coordination
- [ ] Test timeout and error handling
- [ ] Test individual hook executors

#### 6.2 Integration Tests

- [ ] Test hook execution with real events
- [ ] Test hook error handling and recovery
- [ ] Test performance impact

#### 6.3 End-to-End Tests

- [ ] Test complete hook workflows
- [ ] Test integration with existing Codex functionality

---

## 🎯 Success Criteria

By the end of your work, you should achieve:

- [ ] **Hooks execute successfully** with proper error handling
- [ ] **Performance impact < 5%** on normal Codex operations
- [ ] **All hook types working** (script, webhook, MCP)
- [ ] **Integration tests passing** with good coverage

---

## 🤝 Coordination with Developer B

### What Developer B is Working On

- Client-side integration (CLI, TypeScript)
- Documentation and examples
- Advanced features and monitoring

### Shared Dependencies (Already Complete ✅)

- Hook Types (`types.rs`)
- Hook Context (`context.rs`)
- Hook Configuration (`config.rs`)
- Hook Registry (`registry.rs`)

### Communication

- **Daily sync**: Share progress and blockers
- **Branch naming**: Use `feat/hook-execution-*` pattern
- **File ownership**: You own backend Rust files
- **Testing**: Run full test suite before merging

---

## 🚀 Getting Started Commands

```bash
# Create your feature branch
git checkout -b feat/hook-execution-engine

# Start with the manager implementation
code codex-rs/core/src/hooks/manager.rs

# Test your changes
cd codex-rs && cargo test hooks

# Commit your progress
git add .
git commit -m "feat: implement hook execution coordination"
git push origin feat/hook-execution-engine
```

---

## 📊 Your Progress Tracking

### Phase 2: Hook Execution Engine

- [ ] **2.1 Complete**: Core Hook Manager (0/4 tasks)
- [ ] **2.2 Complete**: Hook Executor Framework (0/4 tasks)
- [ ] **2.3 Complete**: Hook Executor Implementations (0/4 tasks)

### Phase 3: Event System Integration

- [ ] **3.1 Complete**: Protocol Extensions (0/3 tasks)
- [ ] **3.2 Complete**: Core Integration Points (0/3 tasks)
- [ ] **3.3 Complete**: Execution Integration (0/4 tasks)

### Phase 6: Testing and Validation

- [ ] **6.1 Complete**: Unit Tests (0/3 tasks)
- [ ] **6.2 Complete**: Integration Tests (0/3 tasks)
- [ ] **6.3 Complete**: End-to-End Tests (0/2 tasks)

**Your Total Progress: 0/30 tasks complete**

---

## 💡 Tips for Success

1. **Start Small**: Begin with basic hook execution in `manager.rs`
2. **Test Early**: Write tests as you implement features
3. **Use Existing Patterns**: Follow Codex's existing async patterns
4. **Performance First**: Keep the async, non-blocking design
5. **Error Handling**: Hooks should never crash the main process
6. **Logging**: Add comprehensive tracing for debugging

**You've got this! 🚀**
