# Workflow System Analysis

> Comprehensive analysis of Auto-Coder's multi-agent workflow orchestration system.

## Table of Contents

1. [Workflow Definition Schema](#1-workflow-definition-schema)
2. [Configuration Example](#2-configuration-example)
3. [Workflow Discovery & Loading](#3-workflow-discovery--loading)
4. [Execution Engine](#4-execution-engine)
5. [Multi-Agent Coordination](#5-multi-agent-coordination)
6. [Condition System](#6-condition-system)
7. [Output Extraction](#7-output-extraction)
8. [Error Handling](#8-error-handling)

---

## 1. Workflow Definition Schema

**Source**: `src/autocoder/workflow_agents/types.py` (213 lines)

### 1.1 Top-Level Structure

```python
@dataclass
class WorkflowSpec:
    """Workflow 总规格"""
    apiVersion: str          # e.g., "autocoder/v1"
    kind: str                # e.g., "SubagentWorkflow"
    metadata: MetadataConfig
    spec: SpecConfig
```

### 1.2 Spec Configuration

```python
@dataclass
class SpecConfig:
    """Spec 配置"""
    globals: GlobalsConfig              # Global settings
    vars: Dict[str, Any]                # Workflow variables
    conversation: ConversationConfig    # Conversation strategy
    attempt: AttemptConfig              # AttemptCompletion config
    agents: List[AgentSpec]             # Agent definitions
    steps: List[StepSpec]               # Execution steps
```

### 1.3 Global Configuration

```python
@dataclass
class GlobalsConfig:
    """全局配置"""
    model: str = "v3_chat"        # Default LLM model
    product_mode: str = "lite"    # "lite" or "pro"
```

### 1.4 Agent Specification

```python
@dataclass
class AgentSpec:
    """代理规格配置"""
    id: str                      # Unique identifier
    path: str                    # Path to agent definition file
    runner: str = "sdk"          # "sdk" or "terminal"
    model: Optional[str] = None  # Override global model
```

### 1.5 Step Specification

```python
@dataclass
class StepSpec:
    """步骤规格配置"""
    id: str                                          # Unique step ID
    agent: str                                       # Reference to agent.id
    needs: List[str] = []                           # Dependencies
    with_args: Dict[str, Any] = {}                  # Input arguments
    when: Optional[WhenConfig] = None               # Condition
    outputs: Dict[str, OutputConfig] = {}           # Output mapping
    conversation: Optional[StepConversationConfig]  # Conversation config
    replicas: int = 1                               # Parallel copies
    merge: Optional[MergeConfig] = None             # Merge strategy
```

### 1.6 Condition Types

```python
@dataclass
class WhenConfig:
    """条件判断配置"""
    regex: Optional[RegexCondition] = None
    jsonpath: Optional[JsonPathCondition] = None
    text: Optional[TextCondition] = None

@dataclass
class RegexCondition:
    input: str           # Input template
    pattern: str         # Regex pattern
    flags: Optional[str] = None

@dataclass
class JsonPathCondition:
    input: str
    path: str                     # JSONPath expression
    exists: Optional[bool] = None
    equals: Optional[Any] = None
    contains: Optional[str] = None

@dataclass
class TextCondition:
    input: str
    contains: Optional[str] = None
    not_contains: Optional[str] = None
    starts_with: Optional[str] = None
    ends_with: Optional[str] = None
    equals: Optional[str] = None
    not_equals: Optional[str] = None
    is_empty: Optional[bool] = None
    matches: Optional[str] = None    # Regex
    ignore_case: bool = False
```

### 1.7 Output Configuration

```python
@dataclass
class OutputConfig:
    """输出映射配置"""
    jsonpath: Optional[str] = None       # e.g., "$.result"
    regex: Optional[str] = None          # Regex pattern
    regex_group: Optional[int] = None    # Group number
    template: Optional[str] = None       # e.g., "${attempt_result}"
```

### 1.8 Execution Results

```python
class StepStatus(str, Enum):
    SUCCESS = "success"
    FAILED = "failed"
    SKIPPED = "skipped"
    CANCELLED = "cancelled"

@dataclass
class StepResult:
    step_id: str
    status: StepStatus
    attempt_result: Optional[str] = None
    error: Optional[str] = None
    outputs: Dict[str, Any] = {}

@dataclass
class WorkflowResult:
    success: bool
    context: Dict[str, Any]
    step_results: List[StepResult]
    error: Optional[str] = None
```

---

## 2. Configuration Example

### 2.1 Complete Workflow YAML

```yaml
apiVersion: autocoder/v1
kind: SubagentWorkflow
metadata:
  name: feature-implementation
  description: Multi-agent feature implementation workflow

spec:
  globals:
    model: v3_chat
    product_mode: lite

  vars:
    feature_request: "Add user authentication"
    file_pattern: "*.py"

  conversation:
    default_action: resume  # resume | new | continue

  attempt:
    format: json
    jsonpaths:
      result: "$.result"
      files: "$.affected_files"

  agents:
    - id: designer
      path: designer.md
      runner: sdk
      # model: gpt-4  # Optional override

    - id: implementer
      path: implementer.md
      runner: terminal

    - id: reviewer
      path: reviewer.md
      runner: sdk
      model: deepseek/v3

  steps:
    - id: design
      agent: designer
      with_args:
        user_input: "${vars.feature_request}"
      outputs:
        design_doc:
          jsonpath: "$.design"
        affected_files:
          jsonpath: "$.files"

    - id: implement
      agent: implementer
      needs: [design]
      when:
        text:
          input: "${steps.design.outputs.design_doc}"
          not_equals: ""
      with_args:
        user_input: |
          Implement the following design:
          ${steps.design.outputs.design_doc}

          Affected files: ${steps.design.outputs.affected_files}
      outputs:
        implementation:
          template: "${attempt_result}"

    - id: review
      agent: reviewer
      needs: [implement]
      replicas: 3    # Run 3 parallel reviews
      merge:
        when:
          text:
            contains: "APPROVED"
      with_args:
        user_input: "Review: ${steps.implement.outputs.implementation}"
      outputs:
        review_result:
          jsonpath: "$.verdict"
```

### 2.2 Agent Definition File (designer.md)

**Source**: `src/autocoder/workflow_agents/agent.py` (153 lines)

Agent definition files use YAML frontmatter format:

```markdown
---
model: v3_chat
include_rules: true
---

You are a software design expert. Given a feature request, create a detailed
technical design document.

Output in JSON format:
{
  "design": "detailed design description",
  "files": ["list", "of", "files"]
}
```

**Frontmatter Fields**:

| Field | Type | Description |
|-------|------|-------------|
| `model` | string | Override global model for this agent |
| `include_rules` | bool | Include project rules/context in prompt |

### 2.3 WorkflowSubAgent Class

**Source**: `src/autocoder/workflow_agents/agent.py`

```python
class WorkflowSubAgent:
    """Workflow sub-agent wrapper"""

    def __init__(
        self,
        agent_id: str,
        model: Optional[str],          # Override global model
        system_prompt: Optional[str],  # From agent definition file
        runner_type: str = "sdk",      # "sdk" or "terminal"
        include_rules: bool = False,   # Include project rules
    )

    def run(
        self,
        user_input: str,
        conversation_config: AgenticEditConversationConfig,
        args: AutoCoderArgs,
        llm: Any,
        cancel_token: Optional[str] = None,
    ) -> Optional[AttemptCompletionTool]:
        """Execute agent and return result"""
```

**Runner Types**:

| Runner | Description |
|--------|-------------|
| `sdk` (default) | Uses `SdkRunner`, returns events via generator |
| `terminal` | Uses `TerminalRunner`, returns direct string result |

**Include Rules Feature**:

When `include_rules=True`:
- Agent receives project context (CLAUDE.md, AGENTS.md, etc.)
- `skip_build_index=False` and `skip_filter_index=False` are set
- Enables codebase-aware agent execution

---

## 3. Workflow Discovery & Loading

### 3.1 Discovery Priority

**Source**: `src/autocoder/workflow_agents/workflow_manager.py` (212 lines)

```python
class WorkflowManager:
    def get_workflow_directories(self) -> List[Path]:
        """Workflow search paths in priority order"""
        return [
            Path(".autocoderworkflow"),                    # Highest priority
            Path(".auto-coder/.autocoderworkflow"),        # Project config
            Path.home() / ".auto-coder/.autocoderworkflow" # Global
        ]
```

### 3.2 Loading Process

**Source**: `src/autocoder/workflow_agents/loader.py` (1046 lines)

```python
def load_workflow_from_yaml(yaml_path: Path) -> WorkflowSpec:
    """
    Load and validate workflow YAML

    Validation steps:
    1. Check required fields (apiVersion, kind, spec)
    2. Validate apiVersion == "autocoder/v1"
    3. Validate kind == "SubagentWorkflow"
    4. Parse and validate agents list
    5. Validate agent uniqueness
    6. Validate agent files exist (via AgentManager)
    7. Parse and validate steps
    8. Validate step IDs unique
    9. Validate agent references exist
    10. Validate dependency chains (no cycles)
    """
```

### 3.3 Agent Discovery

**Source**: `src/autocoder/common/agents/agent_manager.py`

Agent search paths (priority order):
1. `.autocoderagents/` (project root, highest)
2. `.auto-coder/.autocoderagents/` (project config)
3. `~/.auto-coder/.autocoderagents/` (global)

---

## 4. Execution Engine

**Source**: `src/autocoder/workflow_agents/executor.py` (1017 lines)

### 4.1 Executor Initialization

```python
class SubagentWorkflowExecutor:
    def __init__(
        self,
        workflow_spec: WorkflowSpec,
        args: AutoCoderArgs,
        llm: Any,
        cancel_token: Optional[str] = None,
    ) -> None:
        self.workflow_spec = workflow_spec
        self.args = args
        self.llm = llm
        self.cancel_token = cancel_token

        # Parse configuration
        self.spec = workflow_spec.spec
        self.agents: Dict[str, WorkflowSubAgent] = self._build_agents()
        self.steps: List[StepSpec] = self.spec.steps

        # Execution context
        self.context: Dict[str, Any] = {
            "vars": self.spec.vars,
            "steps": {},
            "_last_attempt_result": None,
        }

        # Conversation management
        self._conversation_id: Optional[str] = None

        # LLM cache (multi-model support)
        self._llm_cache: Dict[str, Any] = {args.model: llm}
        self._llm_cache_lock = threading.RLock()

        # LLM manager for validation
        self._llm_manager = LLMManager()
```

### 4.2 Topological Sorting

```python
def _toposort(self) -> List[StepSpec]:
    """
    Topological sort with cycle detection

    Returns:
        Sorted step list in execution order

    Raises:
        WorkflowDependencyError: If circular dependency detected
    """
    result: List[StepSpec] = []
    visited: Set[str] = set()
    visiting: Set[str] = set()  # For cycle detection
    id2step = {step.id: step for step in self.steps}
    dependency_chain: List[str] = []  # Track for error reporting

    def dfs(step_id: str) -> None:
        if step_id in visited:
            return
        if step_id in visiting:
            # Circular dependency detected
            cycle_start = dependency_chain.index(step_id)
            cycle = dependency_chain[cycle_start:] + [step_id]
            raise WorkflowDependencyError(
                message="检测到循环依赖",
                step_id=step_id,
                dependency_chain=cycle
            )

        visiting.add(step_id)
        dependency_chain.append(step_id)

        step = id2step[step_id]
        for need in step.needs:
            if need not in id2step:
                raise WorkflowDependencyError(
                    message=f"依赖步骤不存在: {need}",
                    step_id=step_id
                )
            dfs(need)

        visiting.remove(step_id)
        dependency_chain.pop()
        visited.add(step_id)
        result.append(step)

    for step in self.steps:
        dfs(step.id)

    return result
```

### 4.3 Main Execution Loop

```python
def run(self) -> WorkflowResult:
    """
    Execute workflow steps in topological order

    Features:
    - Check cancel token before each step
    - Failure-tolerant (continues after step failures)
    - Records StepResult for each step
    """
    sorted_steps = self._toposort()

    for step in sorted_steps:
        # Check cancellation
        if global_cancel.is_cancelled(self.cancel_token):
            self.step_results.append(StepResult(
                step_id=step.id,
                status=StepStatus.CANCELLED
            ))
            continue

        try:
            result = self._execute_step(step)
            self.step_results.append(result)
        except Exception as e:
            self.step_results.append(StepResult(
                step_id=step.id,
                status=StepStatus.FAILED,
                error=str(e)
            ))

    # Determine overall success
    success = all(
        r.status in [StepStatus.SUCCESS, StepStatus.SKIPPED]
        for r in self.step_results
    )

    return WorkflowResult(
        success=success,
        context=self.context,
        step_results=self.step_results
    )
```

### 4.4 Single Step Execution

```python
def _execute_step_single(self, step: StepSpec) -> StepResult:
    """Execute a single step instance"""

    # 1. Render input template
    user_input = render_template(
        step.with_args.get("user_input", ""),
        self.context
    )

    # 2. Get/create conversation
    conv_config = self._get_conversation_config(step)

    # 3. Get LLM for agent's model
    agent = self.agents[step.agent]
    llm = self._get_llm_for_model(agent.model, step.id, step.agent)

    # 4. Run agent
    attempt_result = agent.run(
        user_input=user_input,
        args=self.args,
        llm=llm,
        conversation_config=conv_config,
        cancel_token=self.cancel_token
    )

    # 5. Update context
    self.context["_last_attempt_result"] = attempt_result

    # 6. Extract outputs
    outputs = extract_outputs(step.outputs, attempt_result, self.context)
    self.context["steps"][step.id] = {"outputs": outputs}

    return StepResult(
        step_id=step.id,
        status=StepStatus.SUCCESS,
        attempt_result=attempt_result,
        outputs=outputs
    )
```

### 4.5 Parallel Step Execution

```python
def _execute_step_parallel(self, step: StepSpec) -> StepResult:
    """Execute step with multiple replicas"""

    results = []
    with ThreadPoolExecutor(max_workers=step.replicas) as executor:
        futures = []
        for i in range(step.replicas):
            # First replica uses current conversation
            # Others copy conversation for isolation
            if i == 0:
                conv_config = self._get_conversation_config(step)
            else:
                conv_config = copy_conversation(
                    self._get_conversation_config(step)
                )

            future = executor.submit(
                self._execute_replica,
                step, i, conv_config
            )
            futures.append(future)

        for future in as_completed(futures):
            results.append(future.result())

    # Merge results based on strategy
    merged_result = self._merge_replica_results(step, results)
    return merged_result

def _merge_replica_results(
    self, step: StepSpec, results: List[StepResult]
) -> StepResult:
    """Merge results from multiple replicas"""

    # Filter by merge condition
    if step.merge and step.merge.when:
        results = [
            r for r in results
            if evaluate_condition(
                step.merge.when,
                r.attempt_result,
                self.context
            )
        ]

    # Any success = step success
    if any(r.status == StepStatus.SUCCESS for r in results):
        status = StepStatus.SUCCESS
    else:
        status = StepStatus.FAILED

    # Merge attempt_results
    if self.spec.attempt.format == "json":
        # JSON array
        merged = json.dumps([
            json.loads(r.attempt_result)
            for r in results
            if r.attempt_result
        ])
    else:
        # Newline-separated text
        merged = "\n".join(
            r.attempt_result for r in results
            if r.attempt_result
        )

    return StepResult(
        step_id=step.id,
        status=status,
        attempt_result=merged
    )
```

---

## 5. Multi-Agent Coordination

### 5.1 Context Sharing Structure

```python
context = {
    "vars": {
        # Global workflow variables
        "feature_request": "Add authentication",
        "file_pattern": "*.py"
    },
    "steps": {
        "design": {
            "outputs": {
                "design_doc": "...",
                "affected_files": ["auth.py", "user.py"]
            }
        },
        "implement": {
            "outputs": {
                "implementation": "..."
            }
        }
    },
    "_last_attempt_result": "..."  # Previous step's raw result
}
```

### 5.2 Template System

**Source**: `src/autocoder/workflow_agents/utils.py` (lines 31-84)

```python
def render_template(template: Any, context: Dict[str, Any]) -> Any:
    """
    Render template string with context

    Syntax:
    - ${vars.key}                    - Access workflow variable
    - ${steps.stepId.outputs.key}    - Access step output
    - ${attempt_result}              - Previous step's result
    - \$                             - Escape literal $
    """
    pattern = r"(?<!\\)\$\{([^}]+)\}"

    def replace_var(match):
        expr = match.group(1).strip()
        value = _resolve_expression(expr, context)
        return str(value) if value is not None else ""

    result = re.sub(pattern, replace_var, template)
    result = result.replace(r"\$", "$")  # Handle escapes
    return result

def _resolve_expression(expr: str, context: Dict[str, Any]) -> Any:
    """Resolve template expression"""
    parts = expr.split(".")

    # ${vars.key}
    if parts[0] == "vars" and len(parts) >= 2:
        return context.get("vars", {}).get(parts[1])

    # ${steps.stepId.outputs.key}
    if parts[0] == "steps" and len(parts) >= 4 and parts[2] == "outputs":
        step_id = parts[1]
        key = parts[3]
        return context.get("steps", {}).get(step_id, {}).get("outputs", {}).get(key)

    # ${attempt_result}
    if expr == "attempt_result":
        return context.get("_last_attempt_result")

    raise KeyError(f"Unknown expression: {expr}")
```

### 5.3 Conversation Sharing Strategies

```python
class ConversationAction(Enum):
    NEW = "new"        # Always create new conversation
    RESUME = "resume"  # Reuse existing (or create if missing)
    CONTINUE = "continue"  # Continue in conversation chain

# Per-step conversation config
@dataclass
class StepConversationConfig:
    action: str = "resume"
    conversation_id: Optional[str] = None  # Explicit ID override
```

**Strategy Details**:

| Strategy | Behavior |
|----------|----------|
| `new` | Creates fresh conversation for each step |
| `resume` | Reuses existing conversation by ID, creates if missing |
| `continue` | Continues conversation chain, preserving history |

**Parallel Replica Isolation**:
```python
# First replica uses current conversation
# Other replicas copy to avoid race conditions
if replica_index == 0:
    conv_config = self._get_conversation_config(step)
else:
    conv_config = copy_conversation(
        self._get_conversation_config(step)
    )
```

### 5.4 Execution Coordination Patterns

| Pattern | Description |
|---------|-------------|
| Sequential | Steps execute in topological order (DAG) |
| Parallel Replicas | Same step runs N times with ThreadPoolExecutor |
| Conditional | `when` conditions can skip steps |
| Dependency | `needs` ensures prerequisites complete first |

---

## 6. Condition System

**Source**: `src/autocoder/workflow_agents/utils.py` (lines 131-200+)

### 6.1 Regex Condition

```yaml
when:
  regex:
    input: "${steps.step1.outputs.result}"
    pattern: "success|approved"
    flags: "i"  # Case insensitive
```

```python
def evaluate_regex_condition(
    regex_config: RegexCondition,
    attempt_result: Optional[str],
    context: Dict[str, Any]
) -> bool:
    input_str = _get_input_string(regex_config.input, context, attempt_result)
    flags = 0
    if regex_config.flags and "i" in regex_config.flags:
        flags |= re.IGNORECASE
    return bool(re.search(regex_config.pattern, input_str, flags))
```

### 6.2 JSONPath Condition

```yaml
when:
  jsonpath:
    input: "${attempt_result}"
    path: "$.status"
    equals: "completed"
```

```python
def evaluate_jsonpath_condition(
    jsonpath_config: JsonPathCondition,
    attempt_result: Optional[str],
    context: Dict[str, Any]
) -> bool:
    input_str = _get_input_string(jsonpath_config.input, context, attempt_result)

    try:
        data = json.loads(input_str)
        matches = jsonpath_ng.parse(jsonpath_config.path).find(data)

        if jsonpath_config.exists is not None:
            return bool(matches) == jsonpath_config.exists

        if not matches:
            return False

        value = matches[0].value

        if jsonpath_config.equals is not None:
            return value == jsonpath_config.equals

        if jsonpath_config.contains is not None:
            return jsonpath_config.contains in str(value)

        return True
    except:
        return False
```

### 6.3 Text Condition

```yaml
when:
  text:
    input: "${steps.review.outputs.verdict}"
    contains: "APPROVED"
    ignore_case: true
```

**Supported Operators**:

| Operator | Description |
|----------|-------------|
| `contains` | String contains substring |
| `not_contains` | String does not contain substring |
| `starts_with` | String starts with prefix |
| `ends_with` | String ends with suffix |
| `equals` | Exact match |
| `not_equals` | Not equal |
| `is_empty` | Check if empty (True) or not empty (False) |
| `matches` | Regex pattern match |
| `ignore_case` | Case-insensitive comparison |

---

## 7. Output Extraction

### 7.1 JSONPath Extraction

```yaml
outputs:
  result:
    jsonpath: "$.data.result"
```

### 7.2 Regex Extraction

```yaml
outputs:
  version:
    regex: "version: (\\d+\\.\\d+\\.\\d+)"
    regex_group: 1
```

### 7.3 Template Extraction

```yaml
outputs:
  raw_output:
    template: "${attempt_result}"
```

### 7.4 Extraction Implementation

```python
def extract_outputs(
    outputs_config: Dict[str, OutputConfig],
    attempt_result: str,
    context: Dict[str, Any]
) -> Dict[str, Any]:
    """Extract outputs from attempt result"""

    extracted = {}
    for key, config in outputs_config.items():
        if config.jsonpath:
            data = json.loads(attempt_result)
            matches = jsonpath_ng.parse(config.jsonpath).find(data)
            extracted[key] = matches[0].value if matches else None

        elif config.regex:
            match = re.search(config.regex, attempt_result)
            if match:
                group = config.regex_group or 0
                extracted[key] = match.group(group)
            else:
                extracted[key] = None

        elif config.template:
            extracted[key] = render_template(config.template, context)

    return extracted
```

---

## 8. Error Handling

**Source**: `src/autocoder/workflow_agents/exceptions.py` (745 lines)

### 8.1 Exception Hierarchy

```python
class WorkflowError(Exception):
    """Base workflow exception"""
    pass

class WorkflowValidationError(WorkflowError):
    """YAML format/validation errors"""
    pass

class WorkflowFileNotFoundError(WorkflowError):
    """Workflow file not found"""
    pass

class WorkflowParseError(WorkflowError):
    """Invalid YAML syntax"""
    pass

class WorkflowStepError(WorkflowError):
    """Step execution failure"""
    pass

class WorkflowDependencyError(WorkflowError):
    """Circular deps, missing deps"""
    def __init__(self, message, step_id, dependency_chain=None):
        self.step_id = step_id
        self.dependency_chain = dependency_chain
        super().__init__(message)

class WorkflowAgentNotFoundError(WorkflowError):
    """Agent not defined"""
    pass

class WorkflowTemplateError(WorkflowError):
    """Template rendering failure"""
    def __init__(self, template, expression, context_keys):
        self.template = template
        self.expression = expression
        self.context_keys = context_keys

class WorkflowAgentDefinitionError(WorkflowError):
    """Agent file issues"""
    pass

class WorkflowConversationError(WorkflowError):
    """Conversation management errors"""
    pass

class WorkflowConditionError(WorkflowError):
    """Condition evaluation errors"""
    pass

class WorkflowOutputExtractionError(WorkflowError):
    """Output parsing errors"""
    pass

class WorkflowAgentResolutionError(WorkflowError):
    """Agent resolution errors"""
    pass

class WorkflowModelValidationError(WorkflowError):
    """Model not found or key missing"""
    @classmethod
    def for_model_not_found(cls, step_id, agent_id, model):
        return cls(f"Model {model} not found for agent {agent_id} in step {step_id}")

    @classmethod
    def for_key_missing(cls, step_id, agent_id, model):
        return cls(f"API key not configured for model {model}")
```

### 8.2 Error Recovery

The workflow executor is **failure-tolerant**:
- Failed steps are recorded with `StepStatus.FAILED`
- Execution continues to remaining independent steps
- Overall success requires all non-skipped steps to succeed

```python
for step in sorted_steps:
    try:
        result = self._execute_step(step)
        self.step_results.append(result)
    except Exception as e:
        # Record failure but continue
        self.step_results.append(StepResult(
            step_id=step.id,
            status=StepStatus.FAILED,
            error=str(e)
        ))
        # Continue to next step
```
