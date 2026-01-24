# Async Time Command Analysis

> Deep analysis of Auto-Coder's `/async /time` command for continuous agent execution.

## Table of Contents

1. [Command Syntax](#1-command-syntax)
2. [Time String Parsing](#2-time-string-parsing)
3. [Execution Mechanism](#3-execution-mechanism)
4. [Driving Prompts](#4-driving-prompts)
5. [Background Task Management](#5-background-task-management)
6. [Task Kill Mechanism](#6-task-kill-mechanism)
7. [Task Metadata Storage](#7-task-metadata-storage)
8. [Subprocess Execution](#8-subprocess-execution)
9. [Workflow Mode](#9-workflow-mode)

---

## 1. Command Syntax

**Source**: `src/autocoder/inner/async_command_handler.py` (lines 126-202)

### 1.1 Regular Mode Syntax

```bash
/auto /async /name <task_name> /time <duration> "query"
/auto /async /name <task_name> /loop <count> "query"
/auto /async /name <task_name> /model <model> /time 1h "query"
```

### 1.2 Workflow Mode Syntax

```bash
/auto /async /workflow <workflow_name> /name <task_name> "query"
```

### 1.3 Management Commands

```bash
/auto /async /list                    # List all tasks
/auto /async /task <task_id>          # View task details
/auto /async /kill <task_id>          # Terminate task
/auto /async /drop <task_id>          # Delete task metadata
/auto /async /help                    # Show help
```

### 1.4 Parameter Reference

| Parameter | Required | Description |
|-----------|----------|-------------|
| `/async` | Yes | Enable async execution mode |
| `/name <name>` | Yes | Unique task identifier |
| `/time <duration>` | No | Time-based execution (5s, 10m, 2h, 1d) |
| `/loop <count>` | No | Fixed iteration count |
| `/model <model>` | No | Specify LLM model |
| `/workflow <name>` | No | Use workflow mode instead |
| `/libs <value>` | No | Include libraries |
| `/prefix <prefix>` | No | Task prefix |
| `/effect <count>` | No | Alternative to loop |

### 1.5 Command Configuration

```python
def _create_regular_config(self):
    """Create typed config for regular async commands"""
    return (
        create_config()
        .collect_remainder("query")
        .command("async").max_args(0)
        .command("model").positional("value", required=True).max_args(1)
        .command("loop").positional("value", type=int).max_args(1)
        .command("time").positional("value", required=True).max_args(1)
        .command("name").positional("value", required=True).max_args(1)
        .command("prefix").positional("value", required=True).max_args(1)
        .command("libs").positional("value", required=True).max_args(1)
        # Management commands
        .command("list").max_args(0)
        .command("kill").positional("task_id", required=True).max_args(1)
        .command("task").positional("task_id", required=False).max_args(1)
        .command("drop").positional("task_id", required=True).max_args(1)
        .command("help").max_args(0)
        .build()
    )
```

---

## 2. Time String Parsing

**Source**: `src/autocoder/inner/async_command_handler.py` (lines 88-123)

### 2.1 Parsing Implementation

```python
def _parse_time_string(self, time_str: str) -> int:
    """
    Parse time string to seconds

    Args:
        time_str: Time string (e.g., "5s", "10m", "2h", "1d")

    Returns:
        Time in seconds

    Raises:
        ValueError: Invalid format
    """
    time_str = time_str.strip()
    pattern = r"^(\d+)([smhd])$"
    match = re.match(pattern, time_str)

    if not match:
        raise ValueError(
            f"Invalid time format: {time_str}. "
            "Expected: <number><unit> where unit is s/m/h/d. "
            "Example: 5s, 10m, 2h, 1d"
        )

    value = int(match.group(1))
    unit = match.group(2)

    # Convert to seconds
    multipliers = {
        "s": 1,       # seconds
        "m": 60,      # minutes
        "h": 3600,    # hours
        "d": 86400    # days
    }

    return value * multipliers[unit]
```

### 2.2 Conversion Examples

| Input | Seconds |
|-------|---------|
| `5s` | 5 |
| `10m` | 600 |
| `2h` | 7,200 |
| `1d` | 86,400 |
| `30m` | 1,800 |
| `12h` | 43,200 |

---

## 3. Execution Mechanism

**Source**: `src/autocoder/inner/async_command_handler.py` (lines 1183-1367)

### 3.1 Time-Based vs Loop-Based

```python
def _handle_async_execution(self, result, args):
    loop_count = 1
    max_duration_seconds = None

    if result.has_command("time"):
        # Time-based: Parse duration, set very large loop count
        time_value = result.time
        max_duration_seconds = self._parse_time_string(time_value)
        loop_count = 100000  # Effectively infinite

        global_logger.info(
            f"Time-based execution: {max_duration_seconds}s "
            f"(max {loop_count} iterations)"
        )

    elif result.has_command("loop"):
        # Loop-based: Use specified count
        loop_count = result.loop

    elif result.has_command("effect"):
        # Effect is alias for loop
        loop_count = result.effect
```

### 3.2 Main Execution Loop

```python
def run_async_command():
    """Background thread execution"""

    # Record start time for time-based mode
    start_time = time.time() if max_duration_seconds is not None else None

    for i in range(loop_count):
        # 1. Check stop signal BEFORE each iteration
        with self._stop_signals_lock:
            stop_event = self._stop_signals.get(task_id)

        if stop_event and stop_event.is_set():
            global_logger.info(
                f"Task {task_id} received stop signal. "
                f"Completed {i} iterations."
            )
            break

        # 2. Execute iteration
        execute(i)

        # 3. Check time limit AFTER each iteration
        if start_time is not None:
            elapsed_time = time.time() - start_time
            if elapsed_time >= max_duration_seconds:
                global_logger.info(
                    f"Time limit reached: {elapsed_time:.2f}s >= "
                    f"{max_duration_seconds}s. Completed {i + 1} iterations."
                )
                break
            else:
                remaining = max_duration_seconds - elapsed_time
                global_logger.info(
                    f"Iteration {i + 1} complete. "
                    f"Elapsed: {elapsed_time:.2f}s, "
                    f"Remaining: {remaining:.2f}s"
                )
```

### 3.3 Iteration Execution

```python
def execute(index: int):
    """Execute single iteration"""

    # Select query based on iteration index
    if index == 0:
        target_file = tmp_file_path        # Original query
    else:
        target_file = tmp_file_loop_path   # Enhanced loop query

    # Build command
    cmd_args = [
        _get_command_path("auto-coder.run"),
        "--async",
        "--include-rules",
        "--model", model,
        "--verbose",
        "--is-sub-agent",
        "--worktree-name", worktree_name,
    ]

    if task_prefix:
        cmd_args.extend(["--task-prefix", task_prefix])
    if include_libs:
        cmd_args.extend(["--include-libs", include_libs])

    # Read input content
    with open(target_file, "r", encoding="utf-8") as f:
        input_content = f.read()

    # Execute subprocess
    result = subprocess.run(
        cmd_args,
        input=input_content,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=_build_env()
    )

    global_logger.info(f"Async command result: {result.stdout}")
```

---

## 4. Driving Prompts

**Source**: `src/autocoder/inner/async_command_handler.py` (lines 1262-1268)

### 4.1 First Iteration Prompt

For the first iteration (`index == 0`), the **original user query** is used directly:

```
<original user query>
```

### 4.2 Subsequent Iterations Prompt

For all subsequent iterations (`index > 0`), an **enhanced loop query** is used:

```python
loop_query = f"""{async_query}

Additional instruction: use git log to get the code changes generated by previous
tasks and try to focus on iterative improvements and refinements and make sure to
use git commit command to make a commit after every single file edit."""
```

### 4.3 Prompt Design Analysis

The loop prompt instructs the agent to:

| Instruction | Purpose |
|-------------|---------|
| `use git log to get the code changes` | Review what was done in previous iterations |
| `focus on iterative improvements` | Build upon previous work, not start fresh |
| `refinements` | Polish and enhance existing code |
| `git commit after every single file edit` | Maintain atomic, trackable changes |

### 4.4 Why This Works

1. **Context Continuity**: By checking `git log`, the agent understands what it already accomplished
2. **Incremental Progress**: Focus on "improvements and refinements" prevents repetition
3. **Audit Trail**: Commits after each edit create clear history for debugging
4. **Self-Driving Loop**: The prompt naturally leads to finding more work to do

### 4.5 Alternative Loop Instructions

In `sdk/core/bridge.py` (line 159), there's an extended version:

```python
"""use git log to get the code changes generated by previous tasks
and try to focus on iterative improvements and refinements.
Ensure the implementation is complete, functional, and fully usable
without any missing features or incomplete functionality.
Make sure to use git commit command to make a commit after every single file edit."""
```

Additional instructions:
- `Ensure the implementation is complete` - Push toward completion
- `fully usable without any missing features` - Quality check
- `incomplete functionality` - Self-review mechanism

### 4.6 Custom Loop Prompts

**Source**: `src/autocoder/sdk/models/options.py` (line 63)

Users can provide custom loop prompts via the `--loop-additional-prompt` CLI option:

```python
@dataclass
class AutoCoderRunOptions:
    loop: int = 1                           # Loop count
    loop_keep_conversation: bool = False    # Keep conversation across loops
    loop_additional_prompt: Optional[str] = None  # Custom loop prompt
```

**Usage**:
```bash
auto-coder.run --loop 5 --loop-additional-prompt "Focus on test coverage" "query"
```

**SDK Usage**:
```python
from autocoder.sdk import AutoCoderRunOptions, AutoCoderBridge

options = AutoCoderRunOptions(
    loop=5,
    loop_keep_conversation=True,
    loop_additional_prompt="Focus on improving test coverage and edge cases"
)
bridge = AutoCoderBridge(options)
bridge.run("Implement feature X")
```

### 4.7 Loop Conversation Continuity

When `loop_keep_conversation=True`:
- Subsequent loops use `CONTINUE` action instead of `NEW`
- Agent retains full conversation history across iterations
- Enables more coherent multi-iteration improvements

---

## 5. Background Task Management

**Source**: `src/autocoder/inner/async_command_handler.py` (lines 79-87, 1368-1370)

### 5.1 Stop Signal Management

```python
class AsyncCommandHandler:
    def __init__(self):
        # Thread-safe stop signal registry
        self._stop_signals = {}  # task_id -> threading.Event
        self._stop_signals_lock = threading.Lock()
```

### 5.2 Signal Creation

```python
def _execute_async_task(self, ...):
    task_id = worktree_name

    with self._stop_signals_lock:
        if task_id not in self._stop_signals:
            self._stop_signals[task_id] = threading.Event()
        # Clear any residual signal
        self._stop_signals[task_id].clear()

    global_logger.info(f"Created stop signal for task {task_id}")
```

### 5.3 Thread Execution

```python
# Start as daemon thread (won't block program exit)
thread = threading.Thread(target=run_async_command, daemon=True)
thread.start()
```

### 5.4 Signal Checking in Loop

```python
for i in range(loop_count):
    # Check signal before each iteration
    with self._stop_signals_lock:
        stop_event = self._stop_signals.get(task_id)

    if stop_event and stop_event.is_set():
        global_logger.info(f"Task {task_id} stopping at iteration {i}")
        break

    execute(i)
```

### 5.5 Signal Cleanup

```python
finally:
    # Clean up stop signal after completion
    with self._stop_signals_lock:
        if task_id in self._stop_signals:
            del self._stop_signals[task_id]
            global_logger.info(f"Cleaned up stop signal for {task_id}")
```

---

## 6. Task Kill Mechanism

**Source**: `src/autocoder/inner/async_command_handler.py` (lines 365-515)

### 6.1 Kill Process Overview

```python
def _handle_kill_command(self, result) -> None:
    task_id = result.get_command("kill").args[0]

    # 1. Load task metadata
    task = metadata_manager.load_task_metadata(task_id)

    # 2. Verify task is running
    if task.status != "running":
        # Cannot kill non-running task
        return

    # 3. Set stop signal (blocks new iterations)
    with self._stop_signals_lock:
        if task_id not in self._stop_signals:
            self._stop_signals[task_id] = threading.Event()
        self._stop_signals[task_id].set()

    # 4. Kill child process first (auto-coder.run)
    if task.sub_pid > 0 and psutil.pid_exists(task.sub_pid):
        sub_process = psutil.Process(task.sub_pid)
        self._terminate_process_tree(sub_process)

    # 5. Kill main process
    if task.pid > 0 and psutil.pid_exists(task.pid):
        main_process = psutil.Process(task.pid)
        self._terminate_process_tree(main_process)

    # 6. Update metadata
    task.update_status("failed", "Task manually terminated by user")
    metadata_manager.save_task_metadata(task)
```

### 6.2 Process Tree Termination

```python
def _terminate_process_tree(self, process: psutil.Process):
    """Terminate process and all children"""
    try:
        # Get all child processes
        children = process.children(recursive=True)

        # Terminate children first
        for child in children:
            try:
                child.terminate()
            except psutil.NoSuchProcess:
                pass

        # Wait for children
        gone, alive = psutil.wait_procs(children, timeout=3)

        # Kill any survivors
        for p in alive:
            try:
                p.kill()
            except psutil.NoSuchProcess:
                pass

        # Finally terminate parent
        process.terminate()
        process.wait(timeout=3)

    except psutil.NoSuchProcess:
        pass
```

### 6.3 Kill Order Rationale

1. **Set stop signal first** - Prevents new subprocess spawns
2. **Kill sub_pid (auto-coder.run)** - Stop active agent execution
3. **Kill main pid** - Clean up orchestration process
4. **Update metadata** - Mark task as failed with reason

---

## 7. Task Metadata Storage

### 7.1 Storage Location

```
~/.auto-coder/async_agent/
├── meta/
│   ├── task1.json
│   ├── task2.json
│   └── ...
├── tasks/
│   ├── task1/
│   │   └── ... (working directory)
│   └── task2/
│       └── ...
└── logs/
    └── ...
```

### 7.2 Metadata Structure

```python
@dataclass
class TaskMetadata:
    task_id: str                    # Unique identifier (= worktree_name)
    pid: int                        # Main process ID
    sub_pid: int                    # Child process ID (auto-coder.run)
    status: str                     # "running" | "completed" | "failed"
    log_file: str                   # Path to execution log
    created_at: datetime            # Task creation time
    completed_at: Optional[datetime] # Completion time
    user_query: str                 # Original query
    model: str                      # LLM model used
    worktree_path: str              # Git worktree directory
    error_message: Optional[str]    # Error details if failed
```

### 7.3 Metadata JSON Example

```json
{
    "task_id": "feature-auth",
    "pid": 12345,
    "sub_pid": 12346,
    "status": "running",
    "log_file": "/Users/xxx/.auto-coder/async_agent/tasks/feature-auth/log.txt",
    "created_at": "2024-01-15T10:30:00",
    "completed_at": null,
    "user_query": "Implement user authentication with JWT",
    "model": "deepseek/v3",
    "worktree_path": "/project/.git/worktrees/feature-auth"
}
```

---

## 8. Subprocess Execution

**Source**: `src/autocoder/inner/async_command_handler.py` (lines 1279-1319)

### 8.1 Command Construction

```python
cmd_args = [
    _get_command_path("auto-coder.run"),
    "--async",
    "--include-rules",
    "--model", model,
    "--verbose",
    "--is-sub-agent",
    "--worktree-name", worktree_name,
]

if task_prefix:
    cmd_args.extend(["--task-prefix", task_prefix])
if include_libs:
    cmd_args.extend(["--include-libs", include_libs])
```

### 8.2 Cross-Platform Execution

```python
def _build_env() -> Dict[str, str]:
    """Build subprocess environment (UTF-8 support)"""
    env = os.environ.copy()

    # Windows UTF-8 configuration
    if platform.system() == "Windows":
        env.update({
            "PYTHONIOENCODING": "utf-8",
            "LANG": "zh_CN.UTF-8",
            "LC_ALL": "zh_CN.UTF-8",
        })

    return env

# Execute with input via stdin (cross-platform, no shell pipe)
result = subprocess.run(
    cmd_args,
    input=input_content,      # Content via stdin
    capture_output=True,
    text=True,
    encoding="utf-8",
    errors="replace",         # Handle encoding errors
    env=_build_env()
)
```

### 8.3 Command Path Resolution

```python
def _get_command_path(command: str) -> str:
    """Get full path to command (Windows compatibility)"""
    if os.path.isabs(command):
        return command

    full_path = shutil.which(command)
    if full_path:
        return full_path

    # Windows: try with .exe
    if platform.system() == "Windows":
        full_path = shutil.which(f"{command}.exe")
        if full_path:
            return full_path

    return command  # Let subprocess handle error
```

---

## 9. Workflow Mode

**Source**: `src/autocoder/inner/async_command_handler.py` (lines 1404-1511)

### 9.1 Workflow vs Regular Mode

| Feature | Regular Mode | Workflow Mode |
|---------|--------------|---------------|
| Loop support | Yes (`/loop`, `/time`) | No (single execution) |
| Model override | Yes (`/model`) | No (uses workflow's model) |
| Libraries | Yes (`/libs`) | No |
| Execution | Iterative agent calls | Single workflow run |

### 9.2 Workflow Execution

```python
def _execute_async_workflow_task(
    self,
    async_query: str,
    workflow: str,
    worktree_name: str,
):
    """Execute async workflow (single execution, no loop)"""

    # Create stop signal
    task_id = worktree_name
    with self._stop_signals_lock:
        self._stop_signals[task_id] = threading.Event()
        self._stop_signals[task_id].clear()

    def run_async_workflow_command():
        try:
            # Check stop signal
            with self._stop_signals_lock:
                stop_event = self._stop_signals.get(task_id)
            if stop_event and stop_event.is_set():
                return

            # Build workflow command
            cmd_args = [
                _get_command_path("auto-coder.run"),
                "--async",
                "--workflow", workflow,
                "--include-rules",
                "--worktree-name", worktree_name,
            ]

            # Execute
            result = subprocess.run(
                cmd_args,
                input=input_content,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                env=_build_env()
            )

        finally:
            # Cleanup
            with self._stop_signals_lock:
                if task_id in self._stop_signals:
                    del self._stop_signals[task_id]

    # Start in background thread
    thread = threading.Thread(target=run_async_workflow_command, daemon=True)
    thread.start()
```

### 9.3 Workflow Command Parsing

```python
def _create_workflow_config(self):
    """Workflow mode config (strict parsing)"""
    return (
        create_config()
        .strict(True)  # Unknown commands raise errors
        .collect_remainder("query")
        .command("async").max_args(0)
        .command("workflow").positional("value", required=True).max_args(1)
        .command("name").positional("value", required=True).max_args(1)
        # Only management commands allowed
        .command("list").max_args(0)
        .command("kill").positional("task_id", required=True).max_args(1)
        .command("task").positional("task_id", required=False).max_args(1)
        .command("drop").positional("task_id", required=True).max_args(1)
        .command("help").max_args(0)
        .build()
    )
```
