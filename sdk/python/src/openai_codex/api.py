from __future__ import annotations

import asyncio
import re
from dataclasses import dataclass
from typing import AsyncIterator, Iterator

from ._approval_mode import (
    ApprovalMode as ApprovalMode,
    _approval_mode_override_settings,
    _approval_mode_settings,
)
from ._goal import (
    _AsyncGoalNotificationStream,
    _GoalNotificationStream,
    _GoalOperationState,
)
from ._initialize_metadata import validate_initialize_metadata
from ._inputs import (
    ImageInput as ImageInput,
    Input as Input,
    InputItem as InputItem,
    LocalImageInput as LocalImageInput,
    MentionInput as MentionInput,
    RunInput,
    SkillInput as SkillInput,
    TextInput as TextInput,
    _normalize_run_input,
    _to_wire_input,
)
from ._login import (
    AsyncChatgptLoginHandle,
    AsyncDeviceCodeLoginHandle,
    ChatgptLoginHandle,
    DeviceCodeLoginHandle,
    async_start_chatgpt_login,
    async_start_device_code_login,
    start_chatgpt_login,
    start_device_code_login,
)
from ._run import (
    TurnResult,
    _collect_async_turn_result,
    _collect_turn_result,
)
from ._sandbox import Sandbox as Sandbox, _sandbox_mode, _sandbox_policy
from .async_client import AsyncCodexClient
from .client import CodexClient, CodexConfig
from .errors import InvalidRequestError
from .generated.v2_all import (
    ApiKeyLoginAccountParams,
    GetAccountParams,
    GetAccountResponse,
    LoginAccountParams,
    ModelListResponse,
    Personality,
    ReasoningEffort,
    ReasoningSummary,
    SortDirection,
    ThreadArchiveResponse,
    ThreadCompactStartResponse,
    ThreadForkParams,
    ThreadListCwdFilter,
    ThreadListParams,
    ThreadListResponse,
    ThreadReadResponse,
    ThreadResumeParams,
    ThreadSetNameResponse,
    ThreadSortKey,
    ThreadSource,
    ThreadSourceKind,
    ThreadStartParams,
    ThreadStartSource,
    TurnCompletedNotification,
    TurnInterruptResponse,
    TurnStartParams,
    TurnSteerResponse,
)
from .models import InitializeResponse, JsonObject, Notification


def _normalize_goal_objective(objective: str) -> str:
    if not isinstance(objective, str):
        raise TypeError("goal objective must be a string")
    objective = objective.strip()
    if not objective:
        raise ValueError("goal objective must not be empty")
    return objective


def _active_turn_id_from_error(exc: InvalidRequestError) -> str | None:
    match = re.search(r" but found `?([^`]+)`?$", exc.message)
    return match.group(1) if match is not None else None


def _inactive_turn_error() -> InvalidRequestError:
    return InvalidRequestError(-32600, "no active turn to steer")


def _inactive_interrupt_error() -> InvalidRequestError:
    return InvalidRequestError(-32600, "no active turn to interrupt")


class Codex:
    """Synchronous client for creating threads and running Codex turns.

    The client starts its runtime connection during construction. Use it as a
    context manager so resources are closed promptly.
    """

    def __init__(self, config: CodexConfig | None = None) -> None:
        self._client = CodexClient(config=config)
        try:
            self._client.start()
            self._init = validate_initialize_metadata(self._client.initialize())
        except Exception:
            self._client.close()
            raise

    def __enter__(self) -> "Codex":
        return self

    def __exit__(self, _exc_type, _exc, _tb) -> None:
        self.close()

    @property
    def metadata(self) -> InitializeResponse:
        return self._init

    def close(self) -> None:
        self._client.close()

    def login_api_key(self, api_key: str) -> None:
        """Authenticate Codex with an API key."""
        self._client.account_login_start(
            LoginAccountParams(
                root=ApiKeyLoginAccountParams(
                    api_key=api_key,
                    type="apiKey",
                )
            )
        )

    def login_chatgpt(self) -> ChatgptLoginHandle:
        """Start browser-based ChatGPT login and return its live handle."""
        return start_chatgpt_login(self._client)

    def login_chatgpt_device_code(self) -> DeviceCodeLoginHandle:
        """Start device-code ChatGPT login and return its live handle."""
        return start_device_code_login(self._client)

    def account(self, *, refresh_token: bool = False) -> GetAccountResponse:
        """Read the current Codex account state."""
        return self._client.account_read(GetAccountParams(refresh_token=refresh_token))

    def logout(self) -> None:
        """Clear the current Codex account session."""
        self._client.account_logout()

    # BEGIN GENERATED: Codex.flat_methods
    def thread_start(
        self,
        *,
        approval_mode: ApprovalMode = ApprovalMode.auto_review,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: Sandbox | None = None,
        service_name: str | None = None,
        service_tier: str | None = None,
        session_start_source: ThreadStartSource | None = None,
        thread_source: ThreadSource | None = None,
    ) -> Thread:
        """Create a new Codex conversation thread."""
        approval_policy, approvals_reviewer = _approval_mode_settings(approval_mode)
        params = ThreadStartParams(
            approval_policy=approval_policy,
            approvals_reviewer=approvals_reviewer,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            model_provider=model_provider,
            personality=personality,
            sandbox=_sandbox_mode(sandbox),
            service_name=service_name,
            service_tier=service_tier,
            session_start_source=session_start_source,
            thread_source=thread_source,
        )
        started = self._client.thread_start(params)
        return Thread(self._client, started.thread.id)

    def thread_list(
        self,
        *,
        archived: bool | None = None,
        cursor: str | None = None,
        cwd: ThreadListCwdFilter | None = None,
        limit: int | None = None,
        model_providers: list[str] | None = None,
        search_term: str | None = None,
        sort_direction: SortDirection | None = None,
        sort_key: ThreadSortKey | None = None,
        source_kinds: list[ThreadSourceKind] | None = None,
        use_state_db_only: bool | None = None,
    ) -> ThreadListResponse:
        """List saved conversation threads."""
        params = ThreadListParams(
            archived=archived,
            cursor=cursor,
            cwd=cwd,
            limit=limit,
            model_providers=model_providers,
            search_term=search_term,
            sort_direction=sort_direction,
            sort_key=sort_key,
            source_kinds=source_kinds,
            use_state_db_only=use_state_db_only,
        )
        return self._client.thread_list(params)

    def thread_resume(
        self,
        thread_id: str,
        *,
        approval_mode: ApprovalMode | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: Sandbox | None = None,
        service_tier: str | None = None,
    ) -> Thread:
        """Resume an existing conversation thread by ID."""
        approval_policy, approvals_reviewer = _approval_mode_override_settings(approval_mode)
        params = ThreadResumeParams(
            thread_id=thread_id,
            approval_policy=approval_policy,
            approvals_reviewer=approvals_reviewer,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            model=model,
            model_provider=model_provider,
            personality=personality,
            sandbox=_sandbox_mode(sandbox),
            service_tier=service_tier,
        )
        resumed = self._client.thread_resume(thread_id, params)
        return Thread(self._client, resumed.thread.id)

    def thread_fork(
        self,
        thread_id: str,
        *,
        approval_mode: ApprovalMode | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        sandbox: Sandbox | None = None,
        service_tier: str | None = None,
        thread_source: ThreadSource | None = None,
    ) -> Thread:
        """Create a new thread from an existing thread."""
        approval_policy, approvals_reviewer = _approval_mode_override_settings(approval_mode)
        params = ThreadForkParams(
            thread_id=thread_id,
            approval_policy=approval_policy,
            approvals_reviewer=approvals_reviewer,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            model_provider=model_provider,
            sandbox=_sandbox_mode(sandbox),
            service_tier=service_tier,
            thread_source=thread_source,
        )
        forked = self._client.thread_fork(thread_id, params)
        return Thread(self._client, forked.thread.id)

    def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        """Archive a stored conversation thread."""
        return self._client.thread_archive(thread_id)

    def thread_unarchive(self, thread_id: str) -> Thread:
        """Restore an archived conversation thread."""
        unarchived = self._client.thread_unarchive(thread_id)
        return Thread(self._client, unarchived.thread.id)

    # END GENERATED: Codex.flat_methods

    def models(self, *, include_hidden: bool = False) -> ModelListResponse:
        """List available models reported by Codex."""
        return self._client.model_list(include_hidden=include_hidden)


class AsyncCodex:
    """Async mirror of :class:`Codex`.

    Prefer ``async with AsyncCodex()`` so initialization and shutdown are
    explicit and paired. The async client initializes lazily on context entry
    or first awaited API use.
    """

    def __init__(self, config: CodexConfig | None = None) -> None:
        self._client = AsyncCodexClient(config=config)
        self._init: InitializeResponse | None = None
        self._initialized = False
        self._init_lock = asyncio.Lock()

    async def __aenter__(self) -> "AsyncCodex":
        await self._ensure_initialized()
        return self

    async def __aexit__(self, _exc_type, _exc, _tb) -> None:
        await self.close()

    async def _ensure_initialized(self) -> None:
        if self._initialized:
            return
        async with self._init_lock:
            if self._initialized:
                return
            try:
                await self._client.start()
                payload = await self._client.initialize()
                self._init = validate_initialize_metadata(payload)
                self._initialized = True
            except Exception:
                await self._client.close()
                self._init = None
                self._initialized = False
                raise

    @property
    def metadata(self) -> InitializeResponse:
        if self._init is None:
            raise RuntimeError(
                "AsyncCodex is not initialized yet. Prefer `async with AsyncCodex()`; "
                "initialization also happens on first awaited API use."
            )
        return self._init

    async def close(self) -> None:
        await self._client.close()
        self._init = None
        self._initialized = False

    async def login_api_key(self, api_key: str) -> None:
        """Authenticate Codex with an API key."""
        await self._ensure_initialized()
        await self._client.account_login_start(
            LoginAccountParams(
                root=ApiKeyLoginAccountParams(
                    api_key=api_key,
                    type="apiKey",
                )
            )
        )

    async def login_chatgpt(self) -> AsyncChatgptLoginHandle:
        """Start browser-based ChatGPT login and return its live handle."""
        await self._ensure_initialized()
        return await async_start_chatgpt_login(self)

    async def login_chatgpt_device_code(self) -> AsyncDeviceCodeLoginHandle:
        """Start device-code ChatGPT login and return its live handle."""
        await self._ensure_initialized()
        return await async_start_device_code_login(self)

    async def account(self, *, refresh_token: bool = False) -> GetAccountResponse:
        """Read the current Codex account state."""
        await self._ensure_initialized()
        return await self._client.account_read(GetAccountParams(refresh_token=refresh_token))

    async def logout(self) -> None:
        """Clear the current Codex account session."""
        await self._ensure_initialized()
        await self._client.account_logout()

    # BEGIN GENERATED: AsyncCodex.flat_methods
    async def thread_start(
        self,
        *,
        approval_mode: ApprovalMode = ApprovalMode.auto_review,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: Sandbox | None = None,
        service_name: str | None = None,
        service_tier: str | None = None,
        session_start_source: ThreadStartSource | None = None,
        thread_source: ThreadSource | None = None,
    ) -> AsyncThread:
        """Create a new Codex conversation thread."""
        await self._ensure_initialized()
        approval_policy, approvals_reviewer = _approval_mode_settings(approval_mode)
        params = ThreadStartParams(
            approval_policy=approval_policy,
            approvals_reviewer=approvals_reviewer,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            model_provider=model_provider,
            personality=personality,
            sandbox=_sandbox_mode(sandbox),
            service_name=service_name,
            service_tier=service_tier,
            session_start_source=session_start_source,
            thread_source=thread_source,
        )
        started = await self._client.thread_start(params)
        return AsyncThread(self, started.thread.id)

    async def thread_list(
        self,
        *,
        archived: bool | None = None,
        cursor: str | None = None,
        cwd: ThreadListCwdFilter | None = None,
        limit: int | None = None,
        model_providers: list[str] | None = None,
        search_term: str | None = None,
        sort_direction: SortDirection | None = None,
        sort_key: ThreadSortKey | None = None,
        source_kinds: list[ThreadSourceKind] | None = None,
        use_state_db_only: bool | None = None,
    ) -> ThreadListResponse:
        """List saved conversation threads."""
        await self._ensure_initialized()
        params = ThreadListParams(
            archived=archived,
            cursor=cursor,
            cwd=cwd,
            limit=limit,
            model_providers=model_providers,
            search_term=search_term,
            sort_direction=sort_direction,
            sort_key=sort_key,
            source_kinds=source_kinds,
            use_state_db_only=use_state_db_only,
        )
        return await self._client.thread_list(params)

    async def thread_resume(
        self,
        thread_id: str,
        *,
        approval_mode: ApprovalMode | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: Sandbox | None = None,
        service_tier: str | None = None,
    ) -> AsyncThread:
        """Resume an existing conversation thread by ID."""
        await self._ensure_initialized()
        approval_policy, approvals_reviewer = _approval_mode_override_settings(approval_mode)
        params = ThreadResumeParams(
            thread_id=thread_id,
            approval_policy=approval_policy,
            approvals_reviewer=approvals_reviewer,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            model=model,
            model_provider=model_provider,
            personality=personality,
            sandbox=_sandbox_mode(sandbox),
            service_tier=service_tier,
        )
        resumed = await self._client.thread_resume(thread_id, params)
        return AsyncThread(self, resumed.thread.id)

    async def thread_fork(
        self,
        thread_id: str,
        *,
        approval_mode: ApprovalMode | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        sandbox: Sandbox | None = None,
        service_tier: str | None = None,
        thread_source: ThreadSource | None = None,
    ) -> AsyncThread:
        """Create a new thread from an existing thread."""
        await self._ensure_initialized()
        approval_policy, approvals_reviewer = _approval_mode_override_settings(approval_mode)
        params = ThreadForkParams(
            thread_id=thread_id,
            approval_policy=approval_policy,
            approvals_reviewer=approvals_reviewer,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            model_provider=model_provider,
            sandbox=_sandbox_mode(sandbox),
            service_tier=service_tier,
            thread_source=thread_source,
        )
        forked = await self._client.thread_fork(thread_id, params)
        return AsyncThread(self, forked.thread.id)

    async def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        """Archive a stored conversation thread."""
        await self._ensure_initialized()
        return await self._client.thread_archive(thread_id)

    async def thread_unarchive(self, thread_id: str) -> AsyncThread:
        """Restore an archived conversation thread."""
        await self._ensure_initialized()
        unarchived = await self._client.thread_unarchive(thread_id)
        return AsyncThread(self, unarchived.thread.id)

    # END GENERATED: AsyncCodex.flat_methods

    async def models(self, *, include_hidden: bool = False) -> ModelListResponse:
        await self._ensure_initialized()
        return await self._client.model_list(include_hidden=include_hidden)


@dataclass(slots=True)
class Thread:
    """Synchronous conversation thread used to run one or more turns."""

    _client: CodexClient
    id: str

    def run(
        self,
        input: RunInput,
        *,
        approval_mode: ApprovalMode | None = None,
        cwd: str | None = None,
        effort: ReasoningEffort | None = None,
        model: str | None = None,
        output_schema: JsonObject | None = None,
        personality: Personality | None = None,
        sandbox: Sandbox | None = None,
        service_tier: str | None = None,
        summary: ReasoningSummary | None = None,
    ) -> TurnResult:
        """Run a complete turn and collect its final result."""
        turn = self.turn(
            input,
            approval_mode=approval_mode,
            cwd=cwd,
            effort=effort,
            model=model,
            output_schema=output_schema,
            personality=personality,
            sandbox=sandbox,
            service_tier=service_tier,
            summary=summary,
        )
        stream = turn.stream()
        try:
            return _collect_turn_result(stream, turn_id=turn.id)
        finally:
            stream.close()

    def run_goal(self, objective: str) -> TurnResult:
        """Run a persisted goal to completion as one logical turn."""
        return self.start_goal(objective).run()

    # BEGIN GENERATED: Thread.flat_methods
    def turn(
        self,
        input: RunInput,
        *,
        approval_mode: ApprovalMode | None = None,
        cwd: str | None = None,
        effort: ReasoningEffort | None = None,
        model: str | None = None,
        output_schema: JsonObject | None = None,
        personality: Personality | None = None,
        sandbox: Sandbox | None = None,
        service_tier: str | None = None,
        summary: ReasoningSummary | None = None,
    ) -> TurnHandle:
        """Start a turn and return a handle for streaming or control."""
        wire_input = _to_wire_input(_normalize_run_input(input))
        approval_policy, approvals_reviewer = _approval_mode_override_settings(approval_mode)
        params = TurnStartParams(
            thread_id=self.id,
            input=wire_input,
            approval_policy=approval_policy,
            approvals_reviewer=approvals_reviewer,
            cwd=cwd,
            effort=effort,
            model=model,
            output_schema=output_schema,
            personality=personality,
            sandbox_policy=_sandbox_policy(sandbox),
            service_tier=service_tier,
            summary=summary,
        )
        turn = self._client.turn_start(self.id, wire_input, params=params)
        return TurnHandle(self._client, self.id, turn.turn.id)

    # END GENERATED: Thread.flat_methods

    def start_goal(self, objective: str) -> TurnHandle:
        """Activate a persisted goal and return its logical turn handle."""
        objective = _normalize_goal_objective(objective)
        state, turn_id = self._client.start_goal_operation(self.id, objective)
        return TurnHandle(self._client, self.id, turn_id, _goal=state)

    def read(self, *, include_turns: bool = False) -> ThreadReadResponse:
        """Read this thread, optionally including its turn history."""
        return self._client.thread_read(self.id, include_turns=include_turns)

    def set_name(self, name: str) -> ThreadSetNameResponse:
        return self._client.thread_set_name(self.id, name)

    def compact(self) -> ThreadCompactStartResponse:
        return self._client.thread_compact(self.id)


@dataclass(slots=True)
class AsyncThread:
    """Asynchronous conversation thread used to run one or more turns."""

    _codex: AsyncCodex
    id: str

    async def run(
        self,
        input: RunInput,
        *,
        approval_mode: ApprovalMode | None = None,
        cwd: str | None = None,
        effort: ReasoningEffort | None = None,
        model: str | None = None,
        output_schema: JsonObject | None = None,
        personality: Personality | None = None,
        sandbox: Sandbox | None = None,
        service_tier: str | None = None,
        summary: ReasoningSummary | None = None,
    ) -> TurnResult:
        """Run a complete turn asynchronously and collect its final result."""
        turn = await self.turn(
            input,
            approval_mode=approval_mode,
            cwd=cwd,
            effort=effort,
            model=model,
            output_schema=output_schema,
            personality=personality,
            sandbox=sandbox,
            service_tier=service_tier,
            summary=summary,
        )
        stream = turn.stream()
        try:
            return await _collect_async_turn_result(stream, turn_id=turn.id)
        finally:
            await stream.aclose()

    async def run_goal(self, objective: str) -> TurnResult:
        """Run a persisted goal asynchronously as one logical turn."""
        goal = await self.start_goal(objective)
        return await goal.run()

    # BEGIN GENERATED: AsyncThread.flat_methods
    async def turn(
        self,
        input: RunInput,
        *,
        approval_mode: ApprovalMode | None = None,
        cwd: str | None = None,
        effort: ReasoningEffort | None = None,
        model: str | None = None,
        output_schema: JsonObject | None = None,
        personality: Personality | None = None,
        sandbox: Sandbox | None = None,
        service_tier: str | None = None,
        summary: ReasoningSummary | None = None,
    ) -> AsyncTurnHandle:
        """Start a turn and return a handle for streaming or control."""
        await self._codex._ensure_initialized()
        wire_input = _to_wire_input(_normalize_run_input(input))
        approval_policy, approvals_reviewer = _approval_mode_override_settings(approval_mode)
        params = TurnStartParams(
            thread_id=self.id,
            input=wire_input,
            approval_policy=approval_policy,
            approvals_reviewer=approvals_reviewer,
            cwd=cwd,
            effort=effort,
            model=model,
            output_schema=output_schema,
            personality=personality,
            sandbox_policy=_sandbox_policy(sandbox),
            service_tier=service_tier,
            summary=summary,
        )
        turn = await self._codex._client.turn_start(
            self.id,
            wire_input,
            params=params,
        )
        return AsyncTurnHandle(self._codex, self.id, turn.turn.id)

    # END GENERATED: AsyncThread.flat_methods

    async def start_goal(self, objective: str) -> AsyncTurnHandle:
        """Activate a persisted goal and return its async logical turn handle."""
        await self._codex._ensure_initialized()
        objective = _normalize_goal_objective(objective)
        state, turn_id = await self._codex._client.start_goal_operation(self.id, objective)
        return AsyncTurnHandle(self._codex, self.id, turn_id, _goal=state)

    async def read(self, *, include_turns: bool = False) -> ThreadReadResponse:
        """Read this thread, optionally including its turn history."""
        await self._codex._ensure_initialized()
        return await self._codex._client.thread_read(self.id, include_turns=include_turns)

    async def set_name(self, name: str) -> ThreadSetNameResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.thread_set_name(self.id, name)

    async def compact(self) -> ThreadCompactStartResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.thread_compact(self.id)


@dataclass(slots=True)
class TurnHandle:
    """Control and consume a synchronous turn after it has started."""

    _client: CodexClient
    thread_id: str
    id: str
    _goal: _GoalOperationState | None = None

    def steer(self, input: RunInput) -> TurnSteerResponse:
        """Send additional input to this active turn."""
        if self._goal is not None:
            wire_input = _to_wire_input(_normalize_run_input(input))
            turn_id = self._goal.active_turn()
            if turn_id is None:
                raise _inactive_turn_error()
            try:
                response = self._client.turn_steer(self.thread_id, turn_id, wire_input)
            except InvalidRequestError as exc:
                if not (
                    exc.message == "no active turn to steer"
                    or exc.message.startswith("expected active turn id")
                ):
                    raise
                next_turn_id = _active_turn_id_from_error(exc)
                if next_turn_id is None:
                    next_turn_id = self._goal.active_turn(after=turn_id)
                if next_turn_id is None:
                    raise _inactive_turn_error() from exc
                response = self._client.turn_steer(self.thread_id, next_turn_id, wire_input)
            return response.model_copy(update={"turn_id": self.id})
        return self._client.turn_steer(
            self.thread_id,
            self.id,
            _to_wire_input(_normalize_run_input(input)),
        )

    def interrupt(self) -> TurnInterruptResponse:
        """Request interruption of this active turn."""
        if self._goal is not None:
            if not self._goal.begin_interrupt():
                raise _inactive_interrupt_error()
            try:
                self._client.pause_goal(self.thread_id)
            except BaseException:
                self._goal.cancel_interrupt()
                raise
            self._goal.confirm_interrupt()
            turn_id = self._goal.current_turn()
            if turn_id is None:
                return TurnInterruptResponse()
            try:
                return self._client.turn_interrupt(self.thread_id, turn_id)
            except InvalidRequestError as exc:
                if exc.message == "no active turn to interrupt":
                    return TurnInterruptResponse()
                if exc.message.startswith("expected active turn id"):
                    next_turn_id = _active_turn_id_from_error(exc) or self._goal.current_turn()
                    if next_turn_id is None or next_turn_id == turn_id:
                        return TurnInterruptResponse()
                    try:
                        return self._client.turn_interrupt(self.thread_id, next_turn_id)
                    except InvalidRequestError as retry_exc:
                        if retry_exc.message == "no active turn to interrupt":
                            return TurnInterruptResponse()
                        raise
                raise
        return self._client.turn_interrupt(self.thread_id, self.id)

    def stream(self) -> Iterator[Notification]:
        """Yield only notifications routed to this turn handle."""
        if self._goal is not None:
            return _GoalNotificationStream(
                self._goal,
                lambda: self._client.next_goal_notification(self._goal),
                lambda: self._client.unregister_goal_operation(self._goal),
            )

        def ordinary_stream() -> Iterator[Notification]:
            self._client.register_turn_notifications(self.id)
            try:
                while True:
                    event = self._client.next_turn_notification(self.id)
                    yield event
                    if (
                        event.method == "turn/completed"
                        and isinstance(event.payload, TurnCompletedNotification)
                        and event.payload.turn.id == self.id
                    ):
                        break
            finally:
                self._client.unregister_turn_notifications(self.id)

        return ordinary_stream()

    def run(self) -> TurnResult:
        """Consume the turn stream and return its completed result."""
        stream = self.stream()
        try:
            return _collect_turn_result(stream, turn_id=self.id)
        finally:
            stream.close()


@dataclass(slots=True)
class AsyncTurnHandle:
    """Control and consume an asynchronous turn after it has started."""

    _codex: AsyncCodex
    thread_id: str
    id: str
    _goal: _GoalOperationState | None = None

    async def steer(self, input: RunInput) -> TurnSteerResponse:
        """Send additional input to this active turn."""
        await self._codex._ensure_initialized()
        if self._goal is not None:
            wire_input = _to_wire_input(_normalize_run_input(input))
            turn_id = await asyncio.to_thread(self._goal.active_turn)
            if turn_id is None:
                raise _inactive_turn_error()
            try:
                response = await self._codex._client.turn_steer(
                    self.thread_id,
                    turn_id,
                    wire_input,
                )
            except InvalidRequestError as exc:
                if not (
                    exc.message == "no active turn to steer"
                    or exc.message.startswith("expected active turn id")
                ):
                    raise
                next_turn_id = _active_turn_id_from_error(exc)
                if next_turn_id is None:
                    next_turn_id = await asyncio.to_thread(self._goal.active_turn, after=turn_id)
                if next_turn_id is None:
                    raise _inactive_turn_error() from exc
                response = await self._codex._client.turn_steer(
                    self.thread_id,
                    next_turn_id,
                    wire_input,
                )
            return response.model_copy(update={"turn_id": self.id})
        return await self._codex._client.turn_steer(
            self.thread_id,
            self.id,
            _to_wire_input(_normalize_run_input(input)),
        )

    async def interrupt(self) -> TurnInterruptResponse:
        """Request interruption of this active turn."""
        await self._codex._ensure_initialized()
        if self._goal is not None:
            if not self._goal.begin_interrupt():
                raise _inactive_interrupt_error()
            try:
                await self._codex._client.pause_goal(self.thread_id)
            except BaseException:
                self._goal.cancel_interrupt()
                raise
            self._goal.confirm_interrupt()
            turn_id = self._goal.current_turn()
            if turn_id is None:
                return TurnInterruptResponse()
            try:
                return await self._codex._client.turn_interrupt(self.thread_id, turn_id)
            except InvalidRequestError as exc:
                if exc.message == "no active turn to interrupt":
                    return TurnInterruptResponse()
                if exc.message.startswith("expected active turn id"):
                    next_turn_id = _active_turn_id_from_error(exc) or self._goal.current_turn()
                    if next_turn_id is None or next_turn_id == turn_id:
                        return TurnInterruptResponse()
                    try:
                        return await self._codex._client.turn_interrupt(
                            self.thread_id,
                            next_turn_id,
                        )
                    except InvalidRequestError as retry_exc:
                        if retry_exc.message == "no active turn to interrupt":
                            return TurnInterruptResponse()
                        raise
                raise
        return await self._codex._client.turn_interrupt(self.thread_id, self.id)

    def stream(self) -> AsyncIterator[Notification]:
        """Yield only notifications routed to this async turn handle."""
        if self._goal is not None:

            async def next_goal_notification() -> Notification:
                await self._codex._ensure_initialized()
                return await self._codex._client.next_goal_notification(self._goal)

            return _AsyncGoalNotificationStream(
                self._goal,
                next_goal_notification,
                lambda: self._codex._client.unregister_goal_operation(self._goal),
            )

        async def ordinary_stream() -> AsyncIterator[Notification]:
            await self._codex._ensure_initialized()
            self._codex._client.register_turn_notifications(self.id)
            try:
                while True:
                    event = await self._codex._client.next_turn_notification(self.id)
                    yield event
                    if (
                        event.method == "turn/completed"
                        and isinstance(event.payload, TurnCompletedNotification)
                        and event.payload.turn.id == self.id
                    ):
                        break
            finally:
                self._codex._client.unregister_turn_notifications(self.id)

        return ordinary_stream()

    async def run(self) -> TurnResult:
        """Consume the turn stream and return its completed result."""
        stream = self.stream()
        try:
            return await _collect_async_turn_result(stream, turn_id=self.id)
        finally:
            await stream.aclose()
