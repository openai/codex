# Codex App Server SDK — API Reference

Public surface of `codex_app_server` for app-server v2.

This SDK surface is experimental. The current implementation allows concurrent turn consumers on one client only when they belong to different thread IDs. Each client still supports only one active turn per thread ID at a time.

## Package Entry

```python
from codex_app_server import (
    Codex,
    AsyncCodex,
    RunResult,
    Thread,
    AsyncThread,
    TurnHandle,
    AsyncTurnHandle,
    InitializeResponse,
    Input,
    InputItem,
    TextInput,
    ImageInput,
    LocalImageInput,
    SkillInput,
    MentionInput,
    TurnStatus,
)
from codex_app_server.generated.v2_all import ThreadItem, ThreadTokenUsage
```

- Version: `codex_app_server.__version__`
- Requires Python >= 3.10
- Canonical generated app-server models live in `codex_app_server.generated.v2_all`

## Codex (sync)

```python
Codex(config: AppServerConfig | None = None)
```

Properties/methods:

- `metadata -> InitializeResponse`
- `close() -> None`
- `thread_start(*, approval_policy=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, model=None, model_provider=None, personality=None, sandbox=None) -> Thread`
- `thread_list(*, archived=None, cursor=None, cwd=None, limit=None, model_providers=None, sort_key=None, source_kinds=None) -> ThreadListResponse`
- `thread_resume(thread_id: str, *, approval_policy=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, model=None, model_provider=None, personality=None, sandbox=None) -> Thread`
- `thread_fork(thread_id: str, *, approval_policy=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, model=None, model_provider=None, sandbox=None) -> Thread`
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
- `thread_start(*, approval_policy=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, model=None, model_provider=None, personality=None, sandbox=None) -> Awaitable[AsyncThread]`
- `thread_list(*, archived=None, cursor=None, cwd=None, limit=None, model_providers=None, sort_key=None, source_kinds=None) -> Awaitable[ThreadListResponse]`
- `thread_resume(thread_id: str, *, approval_policy=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, model=None, model_provider=None, personality=None, sandbox=None) -> Awaitable[AsyncThread]`
- `thread_fork(thread_id: str, *, approval_policy=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, model=None, model_provider=None, sandbox=None) -> Awaitable[AsyncThread]`
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

- `run(input: str | Input, *, approval_policy=None, approvals_reviewer=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox_policy=None, service_tier=None, summary=None) -> RunResult`
- `turn(input: Input, *, approval_policy=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox_policy=None, summary=None) -> TurnHandle`
- `read(*, include_turns: bool = False) -> ThreadReadResponse`
- `set_name(name: str) -> ThreadSetNameResponse`
- `compact() -> ThreadCompactStartResponse`

### AsyncThread

- `run(input: str | Input, *, approval_policy=None, approvals_reviewer=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox_policy=None, service_tier=None, summary=None) -> Awaitable[RunResult]`
- `turn(input: Input, *, approval_policy=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox_policy=None, summary=None) -> Awaitable[AsyncTurnHandle]`
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
`interrupt()`) or the canonical generated `Turn` from `TurnHandle.run()`.

## TurnHandle / AsyncTurnHandle

### TurnHandle

- `steer(input: Input) -> TurnSteerResponse`
- `interrupt() -> TurnInterruptResponse`
- `stream() -> Iterator[Notification]`
- `run() -> codex_app_server.generated.v2_all.Turn`

Behavior notes:

- `stream()` and `run()` may run concurrently on one client when the turns belong to different thread IDs
- starting a second turn on the same thread raises `RuntimeError`; use `steer()` or `interrupt()` on the existing handle instead
- low-level global notification APIs such as `next_notification()` are incompatible with active turn streaming on the same client

### AsyncTurnHandle

- `steer(input: Input) -> Awaitable[TurnSteerResponse]`
- `interrupt() -> Awaitable[TurnInterruptResponse]`
- `stream() -> AsyncIterator[Notification]`
- `run() -> Awaitable[codex_app_server.generated.v2_all.Turn]`

Behavior notes:

- `stream()` and `run()` may run concurrently on one client when the turns belong to different thread IDs
- starting a second turn on the same thread raises `RuntimeError`; use `steer()` or `interrupt()` on the existing handle instead
- low-level global notification APIs such as `next_notification()` are incompatible with active turn streaming on the same client

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

## Generated Models

The SDK wrappers return and accept canonical generated app-server models wherever possible:

```python
from codex_app_server.generated.v2_all import (
    AskForApproval,
    ThreadReadResponse,
    Turn,
    TurnStartParams,
    TurnStatus,
)
```

## Retry + errors

```python
from codex_app_server import (
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
from codex_app_server import Codex

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    result = thread.run("Say hello in one sentence.")
    print(result.final_response)
```
