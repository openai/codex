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
    ExternalMessage,
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
- `thread_resume(thread_id: str, *, approval_mode=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, include_turns: bool | None = None, model=None, model_provider=None, personality=None, sandbox: Sandbox | None = None, service_tier=None) -> Thread`
- `thread_fork(thread_id: str, *, approval_mode=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, include_turns: bool | None = None, model=None, model_provider=None, sandbox: Sandbox | None = None, service_tier=None) -> Thread`
- `thread_archive(thread_id: str) -> ThreadArchiveResponse`
- `thread_unarchive(thread_id: str) -> Thread`
- `models(*, include_hidden: bool = False) -> ModelListResponse`

Context manager:

```python
with Codex() as codex:
    ...
```

`thread_resume(...)` and `thread_fork(...)` accept `include_turns` to control
whether the server loads turn history into its response. `False` skips that
work; `True` requests it. Omitting the option, or passing `None`, preserves the
server's default behavior. This does not remove history from the model's
context. Both methods return a thread handle; use `thread.read(include_turns=True)`
to retrieve its history.

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
- `thread_resume(thread_id: str, *, approval_mode=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, include_turns: bool | None = None, model=None, model_provider=None, personality=None, sandbox: Sandbox | None = None, service_tier=None) -> Awaitable[AsyncThread]`
- `thread_fork(thread_id: str, *, approval_mode=None, base_instructions=None, config=None, cwd=None, developer_instructions=None, ephemeral=None, include_turns: bool | None = None, model=None, model_provider=None, sandbox: Sandbox | None = None, service_tier=None) -> Awaitable[AsyncThread]`
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

- `run(input: RunInput, *, approval_mode=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox: Sandbox | None = None, service_tier=None, source=None, summary=None, turn_service_tier=None) -> TurnResult`
- `turn(input: RunInput, *, approval_mode=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox: Sandbox | None = None, service_tier=None, source=None, summary=None, turn_service_tier=None) -> TurnHandle`
- `read(*, include_turns: bool = False) -> ThreadReadResponse`
- `set_name(name: str) -> ThreadSetNameResponse`
- `compact() -> ThreadCompactStartResponse`

### AsyncThread

- `run(input: RunInput, *, approval_mode=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox: Sandbox | None = None, service_tier=None, source=None, summary=None, turn_service_tier=None) -> Awaitable[TurnResult]`
- `turn(input: RunInput, *, approval_mode=None, cwd=None, effort=None, model=None, output_schema=None, personality=None, sandbox: Sandbox | None = None, service_tier=None, source=None, summary=None, turn_service_tier=None) -> Awaitable[AsyncTurnHandle]`
- `read(*, include_turns: bool = False) -> Awaitable[ThreadReadResponse]`
- `set_name(name: str) -> Awaitable[ThreadSetNameResponse]`
- `compact() -> Awaitable[ThreadCompactStartResponse]`

`run(...)` is the common-case convenience path. It accepts the same input and
options as `turn(...)`, consumes notifications until completion, and returns a
small result object with:

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

### Turn options

These options have the same behavior on sync and async `run(...)` and `turn(...)`:

| Option | Behavior |
| --- | --- |
| `service_tier: str | None = None` | Sets the thread's service tier for this and subsequent turns. |
| `turn_service_tier: str | None = None` | Overrides the tier for a newly started turn only. `None` inherits the thread setting; `"default"` selects standard speed. Does not change the thread default and is ignored when input joins an active turn. |
| `source: str | None = None` | Labels the caller that initiated a new turn, such as `"review_ui"`. This is metadata; it does not schedule work or grant authority. Ignored when input joins an active turn. |

`ExternalMessage`, `turn_service_tier`, `source`, and explicit `include_turns`
on resume/fork require Codex CLI 0.151.0 or newer. The SDK raises `CodexError`
before sending these options to an older runtime, which would otherwise ignore
them. Published SDK releases install a matching runtime automatically; when
using `CodexConfig.codex_bin`, choose a compatible executable. Unversioned local
builds are checked lazily against their experimental schema before these options
are sent. A custom `launch_args_override` must report a supported version.

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

A `thread.turn(...)` handle receives events from when the call sends its request.
Other handles start when they join; use `thread.read(include_turns=True)` for earlier history.

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
RunInput = Input | str | ExternalMessage
```

Use `ImageInput` with a base64-encoded `data:image/...` URL. HTTP and HTTPS image URLs are
deprecated; download remote images and pass their local paths with `LocalImageInput` instead.

Use a plain `str` as shorthand for `TextInput(...)` anywhere a turn input is accepted:
`thread.run("...")`, `thread.turn("...")`, and `turn.steer("...")`.

### ExternalMessage

`ExternalMessage` supplies **untrusted content** from another agent, tool, or
application. Content reaches the model with tool-level authority, below user
and developer instructions. It does not establish user authorization or
approval. Keep the thread's sandbox and approval policies appropriate for the
work the user has authorized.

```python
from openai_codex import ExternalMessage

message = ExternalMessage(
    tool_name="notifications",
    namespace="slack",
    content="Deployment notification: the staging checks failed.",
)
result = thread.run(message)
```

| Field | Meaning |
| --- | --- |
| `tool_name: str` | Required, nonempty name of the tool or application delivering the message. |
| `content` | Required text, or a sequence of structured content dictionaries or generated `FunctionCallOutputContentItem` models. Structured image content requires inline data URLs. |
| `namespace: str | None = None` | Optional namespace for the tool name. |

Pass one `ExternalMessage` as the complete input to `run(...)` or `turn(...)`.
It starts a turn when the thread is idle or joins an active regular turn. It
appears in saved history and item notifications as a `functionCallOutput`
item, retaining tool authority. No preceding tool call or call ID is required.
Tool names and namespaces identify the source; they are not proof of its
identity or permission to act.

When a message joins an active turn, both handles can stream or collect the
result independently. A joining handle receives previously completed items and
the latest usage, followed by live notifications. Consumed transient events such
as token deltas are discarded. Both handles collect the complete result, and
closing one stream leaves the other active.

The async calls use the same object:

```python
result = await async_thread.run(message)
```

Use `await async_thread.turn(message)` to collect a handle for streaming and
interruption. An `ExternalMessage` cannot be mixed into a user-input list.
`TurnHandle.steer(...)` accepts user input; deliver an external message to an
active turn through `thread.turn(message)`.

See the [external message examples](../examples/16_external_message) for a user
request followed by an external notification.

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

### Notifications and generated models

Known notifications have typed `Notification.payload` values, including
authentication recovery, thread queue/project changes, thread reversion, and
realtime item updates. The `Notification.payload` type covers every registered
event. Unknown methods and payloads that fail validation still produce
`UnknownNotification`, with the raw data in
`.params`. When an event gains a typed payload, read its named fields instead
of `.params`.

Returned models include the current CLI's thread metadata, richer turn errors,
and `functionCallOutput` history items. Code that imports generated
`HookMetadata` directly must access the handler through `.root`, inspect its
`handler_type`, and then read the fields for that handler. For example, only a
`"command"` handler has a `command` field. This reflects the app-server's
separate command, MCP tool, prompt, and agent hook variants.

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
