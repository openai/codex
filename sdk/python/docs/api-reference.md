# OpenAI Codex Python SDK - API Reference

Public surface of `openai_codex` for Codex workflows.

Turn streams are routed by turn ID so one client can consume multiple active turns concurrently.
Thread starts default to `ApprovalMode.auto_review`; turn starts accept an optional `approval_mode` override.

## Package Entry

```python
from openai_codex import (
    Codex,
    AsyncCodex,
    CodexConfig,
    ApprovalMode,
    Sandbox,
    ChatgptLoginHandle,
    DeviceCodeLoginHandle,
    AsyncChatgptLoginHandle,
    AsyncDeviceCodeLoginHandle,
    Thread,
    AsyncThread,
    TurnHandle,
    AsyncTurnHandle,
    TurnResult,
    Input,
    InputItem,
    RunInput,
    TextInput,
    ImageInput,
    LocalImageInput,
    SkillInput,
    MentionInput,
)
from openai_codex.types import (
    Account,
    AccountLoginCompletedNotification,
    CancelLoginAccountResponse,
    CancelLoginAccountStatus,
    GetAccountResponse,
    InitializeResponse,
    ThreadItem,
    ThreadTokenUsage,
    TurnError,
    TurnStatus,
)
```

- Version: `openai_codex.__version__`
- Requires Python >= 3.10
- Public Codex protocol value and event types live in `openai_codex.types`

## Codex (sync)

```python
Codex(config: CodexConfig | None = None)
```

Properties/methods:

- `metadata -> InitializeResponse`
- `close() -> None`
- `login_api_key(api_key: str) -> None`
- `login_chatgpt() -> ChatgptLoginHandle`
- `login_chatgpt_device_code() -> DeviceCodeLoginHandle`
- `account(*, refresh_token: bool = False) -> GetAccountResponse`
- `logout() -> None`
- `thread_start(*, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, model=None, model_provider=None, personality=None, sandbox: Sandbox | None = None) -> Thread`
- `thread_list(*, archived=None, cursor=None, cwd=None, limit=None, model_providers=None, sort_key=None, source_kinds=None) -> ThreadListResponse`
- `thread_resume(thread_id: str, *, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, model=None, model_provider=None, personality=None, sandbox: Sandbox | None = None) -> Thread`
- `thread_fork(thread_id: str, *, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, model=None, model_provider=None, sandbox: Sandbox | None = None) -> Thread`
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
AsyncCodex(config: CodexConfig | None = None)
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
- `login_api_key(api_key: str) -> Awaitable[None]`
- `login_chatgpt() -> Awaitable[AsyncChatgptLoginHandle]`
- `login_chatgpt_device_code() -> Awaitable[AsyncDeviceCodeLoginHandle]`
- `account(*, refresh_token: bool = False) -> Awaitable[GetAccountResponse]`
- `logout() -> Awaitable[None]`
- `thread_start(*, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, model=None, model_provider=None, personality=None, sandbox: Sandbox | None = None) -> Awaitable[AsyncThread]`
- `thread_list(*, archived=None, cursor=None, cwd=None, limit=None, model_providers=None, sort_key=None, source_kinds=None) -> Awaitable[ThreadListResponse]`
- `thread_resume(thread_id: str, *, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, model=None, model_provider=None, personality=None, sandbox: Sandbox | None = None) -> Awaitable[AsyncThread]`
- `thread_fork(thread_id: str, *, approval_mode=ApprovalMode.auto_review, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, model=None, model_provider=None, sandbox: Sandbox | None = None) -> Awaitable[AsyncThread]`
- `thread_archive(thread_id: str) -> Awaitable[ThreadArchiveResponse]`
- `thread_unarchive(thread_id: str) -> Awaitable[AsyncThread]`
- `models(*, include_hidden: bool = False) -> Awaitable[ModelListResponse]`

Async context manager:

```python
async with AsyncCodex() as codex:
    ...
```

## Login handles

### ChatgptLoginHandle / AsyncChatgptLoginHandle

- `login_id: str`
- `auth_url: str`
- `wait() -> AccountLoginCompletedNotification`
- `cancel() -> CancelLoginAccountResponse`

Async handle methods return awaitables.

### DeviceCodeLoginHandle / AsyncDeviceCodeLoginHandle

- `login_id: str`
- `verification_url: str`
- `user_code: str`
- `wait() -> AccountLoginCompletedNotification`
- `cancel() -> CancelLoginAccountResponse`

Async handle methods return awaitables.

`wait()` consumes only the completion notification for its matching login
attempt. API-key login completes synchronously and does not return a handle.

## Thread / AsyncThread

`Thread` and `AsyncThread` share the same shape and intent.

### Thread

- `run(input: str | Input, *, approval_mode=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox: Sandbox | None = None, service_tier=None, summary=None) -> TurnResult`
- `turn(input: str | Input, *, approval_mode=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox: Sandbox | None = None, service_tier=None, summary=None) -> TurnHandle`
- `read(*, include_turns: bool = False) -> ThreadReadResponse`
- `set_name(name: str) -> ThreadSetNameResponse`
- `compact() -> ThreadCompactStartResponse`

### AsyncThread

- `run(input: str | Input, *, approval_mode=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox: Sandbox | None = None, service_tier=None, summary=None) -> Awaitable[TurnResult]`
- `turn(input: str | Input, *, approval_mode=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox: Sandbox | None = None, service_tier=None, summary=None) -> Awaitable[AsyncTurnHandle]`
- `read(*, include_turns: bool = False) -> Awaitable[ThreadReadResponse]`
- `set_name(name: str) -> Awaitable[ThreadSetNameResponse]`
- `compact() -> Awaitable[ThreadCompactStartResponse]`

`run(...)` is the common-case convenience path. It accepts plain strings, starts
the turn, consumes notifications until completion, and returns a small result
object with:

- `id: str`
- `status: TurnStatus`
- `error: TurnError | None`
- `started_at: int | None`
- `completed_at: int | None`
- `duration_ms: int | None`
- `final_response: str | None`
- `items: list[ThreadItem]`
- `usage: ThreadTokenUsage | None`

`final_response` is `None` when the turn finishes without a final-answer or
phase-less assistant message item.

Use `turn(...)` when you need low-level turn control (`stream()`, `steer()`,
`interrupt()`) before collecting the turn result.

## Sandbox

Use `sandbox=` consistently on thread lifecycle methods and turns:

```python
from openai_codex import Codex, Sandbox

with Codex() as codex:
    thread = codex.thread_start(sandbox=Sandbox.workspace_write)
    result = thread.run("Review the diff only.", sandbox=Sandbox.read_only)
```

Presets:

- `Sandbox.read_only`: read files without allowing writes.
- `Sandbox.workspace_write`: the normal default for projects with a recorded trust decision; read files and write inside the workspace and configured writable roots.
- `Sandbox.full_access`: run without filesystem access restrictions.

When `sandbox=` is omitted, Codex uses its configured default. A sandbox
passed to `run(...)` or `turn(...)` applies to that turn and subsequent turns.

## TurnHandle / AsyncTurnHandle

### TurnHandle

- `steer(input: str | Input) -> TurnSteerResponse`
- `interrupt() -> TurnInterruptResponse`
- `stream() -> Iterator[Notification]`
- `run() -> TurnResult`

Behavior notes:

- `stream()` and `run()` consume only notifications for their own turn ID
- one `Codex` instance can stream multiple active turns concurrently

### AsyncTurnHandle

- `steer(input: str | Input) -> Awaitable[TurnSteerResponse]`
- `interrupt() -> Awaitable[TurnInterruptResponse]`
- `stream() -> AsyncIterator[Notification]`
- `run() -> Awaitable[TurnResult]`

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
RunInput = Input | str
```

Use `ImageInput` with a base64-encoded `data:image/...` URL. HTTP and HTTPS image URLs are
deprecated; download remote images and pass their local paths with `LocalImageInput` instead.

Use a plain `str` as shorthand for `TextInput(...)` anywhere a turn input is accepted:
`thread.run("...")`, `thread.turn("...")`, and `turn.steer("...")`.

## Public Types

The SDK wrappers return and accept public Codex protocol models wherever possible:

```python
from openai_codex.types import (
    Account,
    AccountLoginCompletedNotification,
    CancelLoginAccountResponse,
    CancelLoginAccountStatus,
    GetAccountResponse,
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
