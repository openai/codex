# Key Source Files Index

> Reference index of critical source files with line number references.

## Table of Contents

1. [Workflow System Files](#1-workflow-system-files)
2. [Async Command Files](#2-async-command-files)
3. [Supporting Components](#3-supporting-components)
4. [Key Line References](#4-key-line-references)
5. [Code Patterns](#5-code-patterns)

---

## 1. Workflow System Files

### 1.1 Core Workflow Module

| File | Lines | Purpose |
|------|-------|---------|
| `workflow_agents/types.py` | 213 | Data class definitions (WorkflowSpec, StepSpec, etc.) |
| `workflow_agents/executor.py` | 1017 | Main execution orchestrator (SubagentWorkflowExecutor) |
| `workflow_agents/loader.py` | 1046 | YAML parsing and validation |
| `workflow_agents/agent.py` | 153 | WorkflowSubAgent wrapper class |
| `workflow_agents/runner.py` | 269 | High-level API (run_workflow_from_yaml) |
| `workflow_agents/workflow_manager.py` | 212 | Workflow file discovery |
| `workflow_agents/utils.py` | 528 | Template rendering, condition evaluation |
| `workflow_agents/exceptions.py` | 745 | 13 custom exception types with i18n |
| `workflow_agents/__init__.py` | ~50 | Public exports |

### 1.2 File Paths

```
src/autocoder/workflow_agents/
├── __init__.py
├── types.py            # WorkflowSpec, StepSpec, WhenConfig, etc.
├── executor.py         # SubagentWorkflowExecutor class
├── loader.py           # load_workflow_from_yaml(), validation
├── agent.py            # WorkflowSubAgent class
├── runner.py           # run_workflow_from_yaml() public API
├── workflow_manager.py # WorkflowManager, discovery
├── utils.py            # render_template(), evaluate_condition()
└── exceptions.py       # WorkflowError hierarchy
```

---

## 2. Async Command Files

### 2.1 Core Async Module

| File | Lines | Purpose |
|------|-------|---------|
| `inner/async_command_handler.py` | 1512+ | Main /async command handler |
| `inner/agentic.py` | ~500 | Command chain integration |
| `sdk/async_runner/async_executor.py` | ~300 | Background execution engine |
| `sdk/async_runner/async_handler.py` | ~200 | SDK async handler |
| `sdk/async_runner/task_metadata.py` | ~200 | Task metadata management |
| `sdk/core/bridge.py` | ~400 | Loop execution with custom prompts |
| `sdk/models/options.py` | ~180 | AutoCoderRunOptions (loop settings) |

### 2.2 Supporting Files

| File | Purpose |
|------|---------|
| `common/international/messages/async_command_messages.py` | i18n messages |
| `common/international/messages/command_help_messages.py` | Help text |
| `terminal/command_processor.py` | CLI command routing |

### 2.3 File Paths

```
src/autocoder/
├── inner/
│   ├── async_command_handler.py  # AsyncCommandHandler class
│   └── agentic.py                # Integration with command chain
└── sdk/
    └── async_runner/
        ├── async_executor.py     # Background execution
        ├── async_handler.py      # SDK handler
        └── task_metadata.py      # TaskMetadata, TaskMetadataManager
```

---

## 3. Supporting Components

### 3.1 Agent Management

| File | Purpose |
|------|---------|
| `common/agents/agent_manager.py` | AgentManager, agent discovery |
| `common/agents/agent_parser.py` | Parse agent definition files |
| `agent/base_agentic/base_agent.py` | BaseAgent abstract class |
| `agent/base_agentic/agent_hub.py` | Global agent registry |

### 3.2 LLM Management

| File | Purpose |
|------|---------|
| `common/llms/manager.py` | LLMManager class |
| `common/llms/factory.py` | LLMFactory for instance creation |
| `common/llms/schema.py` | LLMModel data class |
| `common/llms/registry.py` | Model registry from models.json |

### 3.3 Conversation Management

| File | Purpose |
|------|---------|
| `common/conversations/manager.py` | PersistConversationManager |
| `common/conversations/get_conversation_manager.py` | Factory functions |
| `common/conversations/models.py` | Conversation, ConversationMessage |

### 3.4 Configuration

| File | Purpose |
|------|---------|
| `common/core_config/config_manager.py` | ConfigManagerMixin |
| `common/core_config/models.py` | CoreMemory dataclass |
| `common/ac_style_command_parser.py` | Command parser (create_config) |

---

## 4. Key Line References

### 4.1 Workflow System

#### types.py
| Lines | Content |
|-------|---------|
| 12-18 | `GlobalsConfig` dataclass |
| 21-24 | `ConversationConfig` dataclass |
| 27-32 | `AttemptConfig` dataclass |
| 35-42 | `AgentSpec` dataclass |
| 45-52 | `RegexCondition` dataclass |
| 54-63 | `JsonPathCondition` dataclass |
| 65-92 | `TextCondition` dataclass (all operators) |
| 94-101 | `WhenConfig` dataclass |
| 103-111 | `OutputConfig` dataclass |
| 137-149 | `StepSpec` dataclass |
| 161-168 | `WorkflowSpec` dataclass |
| 185-192 | `StepStatus` enum |
| 194-203 | `StepResult` dataclass |
| 205-213 | `WorkflowResult` dataclass |

#### executor.py
| Lines | Content |
|-------|---------|
| 53-58 | `SubagentWorkflowExecutor` class docstring |
| 60-106 | `__init__()` - Initialization |
| 87-91 | Context structure definition |
| 99-102 | LLM cache initialization |
| 107-123 | `_build_agents()` method |
| 200-264 | `_get_llm_for_model()` - LLM caching |
| 265-300+ | `_toposort()` - Topological sort |
| 283-295 | Cycle detection logic |

#### utils.py
| Lines | Content |
|-------|---------|
| 23-28 | Constants (TEMPLATE_PREFIX, etc.) |
| 31-84 | `render_template()` function |
| 87-128 | `_resolve_expression()` function |
| 131-165 | `evaluate_condition()` function |
| 167-183 | `_get_input_string()` helper |
| 186-200+ | `evaluate_regex_condition()` |

### 4.2 Async Command

#### async_command_handler.py
| Lines | Content |
|-------|---------|
| 76-77 | `AsyncCommandHandler` class docstring |
| 79-86 | `__init__()` - Stop signals initialization |
| 88-123 | `_parse_time_string()` method |
| 102-103 | Time regex pattern |
| 116-122 | Multipliers dictionary |
| 125-169 | `_create_regular_config()` |
| 171-202 | `_create_workflow_config()` |
| 204-256 | `handle_async_command()` main entry |
| 365-515 | `_handle_kill_command()` |
| 417-428 | Stop signal setting |
| 431-454 | Process termination logic |
| 1150-1159 | Name parameter validation |
| 1183-1232 | Time parameter handling |
| 1196-1202 | Time-based loop count setup |
| 1235-1244 | `_execute_async_task()` signature |
| 1246-1253 | Stop signal creation |
| 1262-1268 | Loop query prompt definition |
| 1270-1367 | `run_async_command()` inner function |
| 1279-1289 | Subprocess command construction |
| 1310-1319 | `subprocess.run()` call |
| 1325-1351 | Main execution loop |
| 1327-1334 | Stop signal check |
| 1339-1346 | Time limit check |
| 1368-1370 | Thread creation and start |
| 1404-1511 | `_execute_async_workflow_task()` |

#### sdk/core/bridge.py
| Lines | Content |
|-------|---------|
| 153-160 | Loop query construction with custom prompt support |
| 156-157 | Custom `loop_additional_prompt` check |
| 159 | Default extended loop prompt |

#### sdk/models/options.py
| Lines | Content |
|-------|---------|
| 61 | `loop: int = 1` - Loop count |
| 62 | `loop_keep_conversation: bool` - Conversation continuity |
| 63 | `loop_additional_prompt: Optional[str]` - Custom prompt |

---

## 5. Code Patterns

### 5.1 Workflow Step Execution Pattern

```python
# executor.py:~350-400
def _execute_step(self, step: StepSpec) -> StepResult:
    # 1. Check condition
    if step.when:
        if not evaluate_condition(step.when, ...):
            return StepResult(status=StepStatus.SKIPPED)

    # 2. Execute based on replicas
    if step.replicas > 1:
        return self._execute_step_parallel(step)
    else:
        return self._execute_step_single(step)
```

### 5.2 Template Variable Pattern

```python
# utils.py:31-84
pattern = r"(?<!\\)\$\{([^}]+)\}"

# Expression types:
# - ${vars.key}
# - ${steps.stepId.outputs.key}
# - ${attempt_result}
```

### 5.3 Stop Signal Pattern

```python
# async_command_handler.py:79-87
self._stop_signals = {}  # task_id -> threading.Event
self._stop_signals_lock = threading.Lock()

# Check pattern (inside loop):
with self._stop_signals_lock:
    stop_event = self._stop_signals.get(task_id)
if stop_event and stop_event.is_set():
    break
```

### 5.4 LLM Cache Pattern

```python
# executor.py:200-264
self._llm_cache: Dict[str, Any] = {args.model: llm}
self._llm_cache_lock = threading.RLock()

def _get_llm_for_model(self, model_name):
    with self._llm_cache_lock:
        if model_name in self._llm_cache:
            return self._llm_cache[model_name]
        new_llm = get_single_llm(model_name, ...)
        self._llm_cache[model_name] = new_llm
        return new_llm
```

### 5.5 Process Termination Pattern

```python
# async_command_handler.py:~420-460
# Order: Signal -> Child -> Parent -> Metadata

# 1. Set stop signal first
with self._stop_signals_lock:
    self._stop_signals[task_id].set()

# 2. Kill child process (auto-coder.run)
if psutil.pid_exists(task.sub_pid):
    self._terminate_process_tree(psutil.Process(task.sub_pid))

# 3. Kill main process
if psutil.pid_exists(task.pid):
    self._terminate_process_tree(psutil.Process(task.pid))

# 4. Update metadata
task.update_status("failed", "Manually terminated")
```

---

## 6. Quick Reference Card

### Workflow
```
types.py          → Data classes (WorkflowSpec, StepSpec)
executor.py       → SubagentWorkflowExecutor.run()
loader.py         → load_workflow_from_yaml()
utils.py          → render_template(), evaluate_condition()
workflow_manager  → WorkflowManager.find_workflow()
```

### Async Command
```
async_command_handler.py:88-123   → Time parsing
async_command_handler.py:1183-1232 → Time parameter handling
async_command_handler.py:1262-1268 → Loop driving prompt
async_command_handler.py:1325-1351 → Main execution loop
async_command_handler.py:365-515  → Kill mechanism
```

### Search Queries
```bash
# Find workflow executor
grep -r "SubagentWorkflowExecutor" src/

# Find async command handling
grep -r "_parse_time_string" src/

# Find loop query prompt
grep -r "git log.*iterative improvements" src/

# Find stop signal usage
grep -r "_stop_signals" src/
```
