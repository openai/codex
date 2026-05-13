# OpenAI Codex SDK — API Reference

Public surface of `openai_codex` for app-server v2.

This SDK surface is experimental. Turn streams are routed by turn ID so one client can consume multiple active turns concurrently.
Thread and turn starts expose `approval_mode`. `ApprovalMode.auto_review` is the default; use `ApprovalMode.deny_all` to deny escalated permissions.

## Package Entry

```python
from openai_codex import (
    Codex,
    AsyncCodex,
    ApprovalMode,
    RunResult,
    Thread,
    AsyncThread,
    TurnHandle,
    AsyncTurnHandle,
    Input,
    InputItem,
    TextInput,
    ImageInput,
    LocalImageInput,
    SkillInput,
    MentionInput,
)
from openai_codex.types import (
    InitializeResponse,
    ThreadItem,
    ThreadTokenUsage,
    TurnStatus,
)
```

- Version: `openai_codex.__version__`
- Requires Python >= 3.10
- Public app-server value and event types live in `openai_codex.types`

## Codex (sync)

```python
Codex(config: AppServerConfig | None = None)
```

Properties/methods:

- `metadata -> InitializeResponse`
- `close() -> None`
- `thread_start(*, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, model=None, model_provider=None, personality=None, sandbox=None) -> Thread`
- `thread_list(*, archived=None, cursor=None, cwd=None, limit=None, model_providers=None, sort_key=None, source_kinds=None) -> ThreadListResponse`
- `thread_resume(thread_id: str, *, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, model=None, model_provider=None, personality=None, sandbox=None) -> Thread`
- `thread_fork(thread_id: str, *, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, model=None, model_provider=None, sandbox=None) -> Thread`
- `thread_archive(thread_id: str) -> ThreadArchiveResponse`
- `thread_unarchive(thread_id: str) -> Thread`
- `models(*, include_hidden: bool = False) -> ModelListResponse`

Context manager:

```python
with Codex() as codex:
    ...
```

## AsyncCodex (async parity)

```python
AsyncCodex(config: AppServerConfig | None = None)
```

Preferred usage:

```python
async with AsyncCodex() as codex:
    ...
```

`AsyncCodex` initializes lazily. Context entry is the standard path because it
ensures startup and shutdown are paired explicitly.

Properties/methods:

- `metadata -> InitializeResponse`
- `close() -> Awaitable[None]`
- `thread_start(*, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, model=None, model_provider=None, personality=None, sandbox=None) -> Awaitable[AsyncThread]`
- `thread_list(*, archived=None, cursor=None, cwd=None, limit=None, model_providers=None, sort_key=None, source_kinds=None) -> Awaitable[ThreadListResponse]`
- `thread_resume(thread_id: str, *, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, model=None, model_provider=None, personality=None, sandbox=None) -> Awaitable[AsyncThread]`
- `thread_fork(thread_id: str, *, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, model=None, model_provider=None, sandbox=None) -> Awaitable[AsyncThread]`
- `thread_archive(thread_id: str) -> Awaitable[ThreadArchiveResponse]`
- `thread_unarchive(thread_id: str) -> Awaitable[AsyncThread]`
- `models(*, include_hidden: bool = False) -> Awaitable[ModelListResponse]`

Async context manager:

```python
async with AsyncCodex() as codex:
    ...
```

## Thread / AsyncThread

`Thread` and `AsyncThread` share the same shape and intent.

### Thread

- `run(input: str | Input, *, approval_mode=ApprovalMode.auto_review, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox_policy=None, service_tier=None, summary=None) -> RunResult`
- `turn(input: Input, *, approval_mode=ApprovalMode.auto_review, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox_policy=None, summary=None) -> TurnHandle`
- `read(*, include_turns: bool = False) -> ThreadReadResponse`
- `set_name(name: str) -> ThreadSetNameResponse`
- `compact() -> ThreadCompactStartResponse`

### AsyncThread

- `run(input: str | Input, *, approval_mode=ApprovalMode.auto_review, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox_policy=None, service_tier=None, summary=None) -> Awaitable[RunResult]`
- `turn(input: Input, *, approval_mode=ApprovalMode.auto_review, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox_policy=None, summary=None) -> Awaitable[AsyncTurnHandle]`
- `read(*, include_turns: bool = False) -> Awaitable[ThreadReadResponse]`
- `set_name(name: str) -> Awaitable[ThreadSetNameResponse]`
- `compact() -> Awaitable[ThreadCompactStartResponse]`

`run(...)` is the common-case convenience path. It accepts plain strings, starts
the turn, consumes notifications until completion, and returns a small result
object with:

- `final_response: str | None`
- `items: list[ThreadItem]`
- `usage: ThreadTokenUsage | None`

`final_response` is `None` when the turn finishes without a final-answer or
phase-less assistant message item.

Use `turn(...)` when you need low-level turn control (`stream()`, `steer()`,
`interrupt()`) or the public `Turn` model from `TurnHandle.run()`.

## TurnHandle / AsyncTurnHandle

### TurnHandle

- `steer(input: Input) -> TurnSteerResponse`
- `interrupt() -> TurnInterruptResponse`
- `stream() -> Iterator[Notification]`
- `run() -> openai_codex.types.Turn`

Behavior notes:

- `stream()` and `run()` consume only notifications for their own turn ID
- one `Codex` instance can stream multiple active turns concurrently

### AsyncTurnHandle

- `steer(input: Input) -> Awaitable[TurnSteerResponse]`
- `interrupt() -> Awaitable[TurnInterruptResponse]`
- `stream() -> AsyncIterator[Notification]`
- `run() -> Awaitable[openai_codex.types.Turn]`

Behavior notes:

- `stream()` and `run()` consume only notifications for their own turn ID
- one `AsyncCodex` instance can stream multiple active turns concurrently

## Inputs

```python
@dataclass class TextInput: text: str
@dataclass class ImageInput: url: str
@dataclass class LocalImageInput: path: str
@dataclass class SkillInput: name: str; path: str
@dataclass class MentionInput: name: str; path: str

InputItem = TextInput | ImageInput | LocalImageInput | SkillInput | MentionInput
Input = list[InputItem] | InputItem
```

## Public Types

The SDK wrappers return and accept public app-server models wherever possible:

```python
from openai_codex.types import (
    ThreadReadResponse,
    Turn,
    TurnStatus,
)
```

## Retry + errors

```python
from openai_codex import (
    retry_on_overload,
    JsonRpcError,
    MethodNotFoundError,
    InvalidParamsError,
    ServerBusyError,
    is_retryable_error,
)
```

- `retry_on_overload(...)` retries transient overload errors with exponential backoff + jitter.
- `is_retryable_error(exc)` checks if an exception is transient/overload-like.

## Example

```python
from openai_codex import Codex

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    result = thread.run("Say hello in one sentence.")
    print(result.final_response)
```
