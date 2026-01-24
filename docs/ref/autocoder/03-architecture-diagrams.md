# Architecture Diagrams

> Visual representation of Auto-Coder's workflow and async execution systems.

## Table of Contents

1. [Workflow Execution Flow](#1-workflow-execution-flow)
2. [Async Time Execution Flow](#2-async-time-execution-flow)
3. [Multi-Agent Coordination](#3-multi-agent-coordination)
4. [Context Data Flow](#4-context-data-flow)
5. [Task Lifecycle](#5-task-lifecycle)
6. [Component Architecture](#6-component-architecture)

---

## 1. Workflow Execution Flow

```mermaid
flowchart TD
    A[Load YAML Config] --> B[Parse WorkflowSpec]
    B --> C[Validate Schema]
    C --> D{Valid?}
    D -->|No| E[Raise ValidationError]
    D -->|Yes| F[Build Agents]
    F --> G[Topological Sort Steps]
    G --> H{Cycle Detected?}
    H -->|Yes| I[Raise DependencyError]
    H -->|No| J[Execute Steps Loop]

    J --> K{More Steps?}
    K -->|No| L[Return WorkflowResult]
    K -->|Yes| M{Cancelled?}
    M -->|Yes| N[Mark CANCELLED]
    N --> K
    M -->|No| O[Evaluate Condition]
    O --> P{Condition Pass?}
    P -->|No| Q[Mark SKIPPED]
    Q --> K
    P -->|Yes| R{Replicas > 1?}
    R -->|No| S[Execute Single]
    R -->|Yes| T[Execute Parallel]
    S --> U[Extract Outputs]
    T --> V[Merge Results]
    V --> U
    U --> W[Update Context]
    W --> K
```

---

## 2. Async Time Execution Flow

```mermaid
flowchart TD
    A[Parse /async /time command] --> B{Has /time?}
    B -->|Yes| C[Parse time string]
    C --> D[Set loop_count = 100000]
    D --> E[Calculate max_duration]
    B -->|No| F{Has /loop?}
    F -->|Yes| G[Use specified count]
    F -->|No| H[Set loop_count = 1]
    G --> I[Create Stop Signal]
    H --> I
    E --> I

    I --> J[Start Daemon Thread]
    J --> K[Record start_time]
    K --> L[Iteration Loop]

    L --> M{Check Stop Signal}
    M -->|Set| N[Log & Exit]
    M -->|Clear| O{Time Exceeded?}
    O -->|Yes| P[Log & Exit]
    O -->|No| Q{First iteration?}
    Q -->|Yes| R[Use original query]
    Q -->|No| S[Use loop_query]
    R --> T[Build subprocess args]
    S --> T
    T --> U[Execute auto-coder.run]
    U --> V[Log result]
    V --> L

    N --> W[Cleanup Stop Signal]
    P --> W
    W --> X[Delete temp files]
```

---

## 3. Multi-Agent Coordination

```mermaid
flowchart LR
    subgraph Workflow["Workflow Execution"]
        direction TB
        S1["Step 1: Designer"] --> S2["Step 2: Implementer"]
        S2 --> S3["Step 3: Reviewer"]
    end

    subgraph Context["Shared Context"]
        direction TB
        V["vars"] --> S1
        S1 -->|"outputs"| O1["steps.step1.outputs"]
        O1 --> S2
        S2 -->|"outputs"| O2["steps.step2.outputs"]
        O2 --> S3
        S3 -->|"outputs"| O3["steps.step3.outputs"]
    end

    subgraph Agents["Agent Pool"]
        A1["Designer Agent<br/>model: gpt-4"]
        A2["Implementer Agent<br/>model: v3_chat"]
        A3["Reviewer Agent<br/>model: deepseek/v3"]
    end

    S1 -.->|"runs"| A1
    S2 -.->|"runs"| A2
    S3 -.->|"runs"| A3
```

---

## 4. Context Data Flow

```mermaid
flowchart TD
    subgraph Input["Input Sources"]
        V["vars:<br/>feature_request: 'Add auth'"]
        Q["User Query"]
    end

    subgraph Step1["Step 1: Design"]
        T1["Template:<br/>${vars.feature_request}"]
        A1["Designer Agent"]
        O1["outputs:<br/>design_doc, files"]
    end

    subgraph Step2["Step 2: Implement"]
        T2["Template:<br/>${steps.design.outputs.design_doc}"]
        A2["Implementer Agent"]
        O2["outputs:<br/>implementation"]
    end

    subgraph Step3["Step 3: Review"]
        T3["Template:<br/>${steps.implement.outputs.implementation}"]
        A3["Reviewer Agent"]
        O3["outputs:<br/>verdict"]
    end

    V --> T1
    T1 --> A1
    A1 -->|"attempt_result"| O1
    O1 --> T2
    T2 --> A2
    A2 -->|"attempt_result"| O2
    O2 --> T3
    T3 --> A3
    A3 -->|"attempt_result"| O3
```

---

## 5. Task Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Initializing: /async command parsed

    Initializing --> Running: Thread started
    Running --> Running: Iteration complete

    Running --> Completed: All iterations done
    Running --> Completed: Time limit reached
    Running --> Failed: Stop signal received
    Running --> Failed: Error occurred
    Running --> Failed: Manually killed

    Completed --> [*]
    Failed --> [*]

    note right of Running
        Checks on each iteration:
        1. Stop signal
        2. Time limit
        3. Loop count
    end note
```

---

## 6. Component Architecture

### 6.1 Workflow System Components

```mermaid
classDiagram
    class WorkflowSpec {
        +apiVersion: str
        +kind: str
        +metadata: MetadataConfig
        +spec: SpecConfig
    }

    class SpecConfig {
        +globals: GlobalsConfig
        +vars: Dict
        +conversation: ConversationConfig
        +attempt: AttemptConfig
        +agents: List~AgentSpec~
        +steps: List~StepSpec~
    }

    class AgentSpec {
        +id: str
        +path: str
        +runner: str
        +model: Optional~str~
    }

    class StepSpec {
        +id: str
        +agent: str
        +needs: List~str~
        +with_args: Dict
        +when: Optional~WhenConfig~
        +outputs: Dict
        +replicas: int
        +merge: Optional~MergeConfig~
    }

    class SubagentWorkflowExecutor {
        +workflow_spec: WorkflowSpec
        +args: AutoCoderArgs
        +llm: Any
        +context: Dict
        +agents: Dict
        +run() WorkflowResult
        -_build_agents()
        -_toposort()
        -_execute_step()
    }

    class WorkflowSubAgent {
        +agent_id: str
        +model: str
        +system_prompt: str
        +runner_type: str
        +run()
    }

    WorkflowSpec --> SpecConfig
    SpecConfig --> AgentSpec
    SpecConfig --> StepSpec
    SubagentWorkflowExecutor --> WorkflowSpec
    SubagentWorkflowExecutor --> WorkflowSubAgent
```

### 6.2 Async Command Components

```mermaid
classDiagram
    class AsyncCommandHandler {
        +async_agent_dir: Path
        +console: Console
        -_stop_signals: Dict
        -_stop_signals_lock: Lock
        +handle_async_command()
        +_parse_time_string()
        -_execute_async_task()
        -_handle_kill_command()
        -_handle_list_command()
    }

    class TaskMetadataManager {
        +meta_dir: Path
        +load_task_metadata()
        +save_task_metadata()
        +list_tasks()
        +get_task_summary()
    }

    class TaskMetadata {
        +task_id: str
        +pid: int
        +sub_pid: int
        +status: str
        +log_file: str
        +created_at: datetime
        +completed_at: datetime
        +user_query: str
        +model: str
        +update_status()
    }

    AsyncCommandHandler --> TaskMetadataManager
    TaskMetadataManager --> TaskMetadata
```

---

## 7. Parallel Replica Execution

```mermaid
flowchart TD
    subgraph Step["Step with replicas=3"]
        S["Step Config"]
    end

    subgraph ThreadPool["ThreadPoolExecutor"]
        T1["Thread 1<br/>Replica 0"]
        T2["Thread 2<br/>Replica 1"]
        T3["Thread 3<br/>Replica 2"]
    end

    subgraph Conversations["Conversation Isolation"]
        C1["Original Conv"]
        C2["Copy of Conv"]
        C3["Copy of Conv"]
    end

    subgraph Results["Results"]
        R1["Result 1"]
        R2["Result 2"]
        R3["Result 3"]
    end

    subgraph Merge["Merge Strategy"]
        M["Filter by condition<br/>+ Combine"]
    end

    S --> T1
    S --> T2
    S --> T3

    T1 --> C1
    T2 --> C2
    T3 --> C3

    C1 --> R1
    C2 --> R2
    C3 --> R3

    R1 --> M
    R2 --> M
    R3 --> M

    M --> F["Final StepResult"]
```

---

## 8. Stop Signal Flow

```mermaid
sequenceDiagram
    participant User
    participant CLI
    participant Handler as AsyncCommandHandler
    participant Signal as StopSignal
    participant Thread as DaemonThread
    participant Subprocess as auto-coder.run

    User->>CLI: /async /kill task123
    CLI->>Handler: _handle_kill_command("task123")
    Handler->>Signal: Set stop signal

    Note over Thread: Next iteration check
    Thread->>Signal: Check signal
    Signal-->>Thread: Signal is SET
    Thread->>Thread: Break loop

    Handler->>Subprocess: Terminate (psutil)
    Subprocess-->>Handler: Process killed

    Handler->>Handler: Update metadata to "failed"
    Handler-->>CLI: Kill success
    CLI-->>User: Task terminated
```

---

## 9. Template Resolution Flow

```mermaid
flowchart TD
    A["Template String:<br/>'Implement: \${steps.design.outputs.doc}'"]

    B["Pattern Match:<br/>(?<!\\\\)\\$\\{([^}]+)\\}"]

    C["Expression:<br/>steps.design.outputs.doc"]

    D["Parse Expression"]

    E{Expression Type}

    F["vars.key"]
    G["steps.id.outputs.key"]
    H["attempt_result"]

    I["Context Lookup"]

    J["Resolved Value:<br/>'Design document content...'"]

    K["Final String:<br/>'Implement: Design document content...'"]

    A --> B
    B --> C
    C --> D
    D --> E
    E -->|"starts with vars."| F
    E -->|"starts with steps."| G
    E -->|"is attempt_result"| H
    F --> I
    G --> I
    H --> I
    I --> J
    J --> K
```
