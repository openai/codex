from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import AsyncIterator, Iterator

from .async_client import AsyncAppServerClient
from .client import AppServerClient, AppServerConfig
from .generated.v2_all import (
    AgentMessageDeltaNotification,
    RawResponseItemCompletedNotification,
    ThreadArchiveResponse,
    ThreadSetNameResponse,
    TurnError,
    TurnInterruptResponse,
)
from .generated.v2_types import (
    ModelListResponse,
    ThreadCompactStartResponse,
    ThreadItem,
    ThreadListResponse,
    ThreadReadResponse,
    ThreadTokenUsageUpdatedNotification,
    TurnCompletedNotificationPayload,
    TurnSteerResponse,
)
from .models import InitializeResponse, JsonObject, Notification
from .public_types import (
    AskForApproval,
    Personality,
    ReasoningEffort,
    ReasoningSummary,
    SandboxMode,
    SandboxPolicy,
    ServiceTier,
    ThreadForkParams,
    ThreadListParams,
    ThreadResumeParams,
    ThreadSortKey,
    ThreadSourceKind,
    ThreadStartParams,
    TurnStartParams,
    TurnStatus,
)


@dataclass(slots=True)
class TurnResult:
    thread_id: str
    turn_id: str
    status: TurnStatus
    error: TurnError | None
    text: str
    items: list[ThreadItem]
    usage: ThreadTokenUsageUpdatedNotification | None = None


@dataclass(slots=True)
class TextInput:
    text: str


@dataclass(slots=True)
class ImageInput:
    url: str


@dataclass(slots=True)
class LocalImageInput:
    path: str


@dataclass(slots=True)
class SkillInput:
    name: str
    path: str


@dataclass(slots=True)
class MentionInput:
    name: str
    path: str


InputItem = TextInput | ImageInput | LocalImageInput | SkillInput | MentionInput
Input = list[InputItem] | InputItem


@dataclass(slots=True)
class InitializeResult:
    server_name: str
    server_version: str
    user_agent: str


def _to_wire_item(item: InputItem) -> JsonObject:
    if isinstance(item, TextInput):
        return {"type": "text", "text": item.text}
    if isinstance(item, ImageInput):
        return {"type": "image", "url": item.url}
    if isinstance(item, LocalImageInput):
        return {"type": "localImage", "path": item.path}
    if isinstance(item, SkillInput):
        return {"type": "skill", "name": item.name, "path": item.path}
    if isinstance(item, MentionInput):
        return {"type": "mention", "name": item.name, "path": item.path}
    raise TypeError(f"unsupported input item: {type(item)!r}")


def _to_wire_input(input: Input) -> list[JsonObject]:
    if isinstance(input, list):
        return [_to_wire_item(i) for i in input]
    return [_to_wire_item(input)]


def _split_user_agent(user_agent: str) -> tuple[str | None, str | None]:
    raw = user_agent.strip()
    if not raw:
        return None, None
    if "/" in raw:
        name, version = raw.split("/", 1)
        return (name or None), (version or None)
    parts = raw.split(maxsplit=1)
    if len(parts) == 2:
        return parts[0], parts[1]
    return raw, None


def _enum_value(value: object) -> object:
    return getattr(value, "value", value)


def _assistant_output_text_chunks(
    notification: RawResponseItemCompletedNotification,
) -> list[str]:
    item = notification.item.root
    if _enum_value(getattr(item, "type", None)) != "message":
        return []
    if getattr(item, "role", None) != "assistant":
        return []

    chunks: list[str] = []
    for content in getattr(item, "content", []) or []:
        content_item = getattr(content, "root", content)
        if _enum_value(getattr(content_item, "type", None)) != "output_text":
            continue
        text = getattr(content_item, "text", None)
        if isinstance(text, str) and text:
            chunks.append(text)
    return chunks


def _build_turn_result(
    completed: TurnCompletedNotificationPayload | None,
    usage: ThreadTokenUsageUpdatedNotification | None,
    delta_chunks: list[str],
    raw_text_chunks: list[str],
) -> TurnResult:
    if completed is None:
        raise RuntimeError("turn completed event not received")
    if completed.turn.status == TurnStatus.completed and usage is None:
        raise RuntimeError(
            "thread/tokenUsage/updated notification not received for completed turn"
        )

    text = "".join(delta_chunks) if delta_chunks else "".join(raw_text_chunks)
    return TurnResult(
        thread_id=completed.thread_id,
        turn_id=completed.turn.id,
        status=completed.turn.status,
        error=completed.turn.error,
        text=text,
        items=list(completed.turn.items or []),
        usage=usage,
    )


class Codex:
    """Minimal typed SDK surface for app-server v2."""

    def __init__(self, config: AppServerConfig | None = None) -> None:
        self._client = AppServerClient(config=config)
        try:
            self._client.start()
            self._init = self._parse_initialize(self._client.initialize())
        except Exception:
            self._client.close()
            raise

    def __enter__(self) -> "Codex":
        return self

    def __exit__(self, _exc_type, _exc, _tb) -> None:
        self.close()

    @staticmethod
    def _parse_initialize(payload: InitializeResponse) -> InitializeResult:
        user_agent = (payload.userAgent or "").strip()
        server = payload.serverInfo

        server_name: str | None = None
        server_version: str | None = None

        if server is not None:
            server_name = (server.name or "").strip() or None
            server_version = (server.version or "").strip() or None

        if (server_name is None or server_version is None) and user_agent:
            parsed_name, parsed_version = _split_user_agent(user_agent)
            if server_name is None:
                server_name = parsed_name
            if server_version is None:
                server_version = parsed_version

        normalized_server_name = (server_name or "").strip()
        normalized_server_version = (server_version or "").strip()
        if not user_agent or not normalized_server_name or not normalized_server_version:
            raise RuntimeError(
                "initialize response missing required metadata "
                f"(user_agent={user_agent!r}, server_name={normalized_server_name!r}, server_version={normalized_server_version!r})"
            )

        return InitializeResult(
            server_name=normalized_server_name,
            server_version=normalized_server_version,
            user_agent=user_agent,
        )

    @property
    def metadata(self) -> InitializeResult:
        return self._init

    def close(self) -> None:
        self._client.close()

    # BEGIN GENERATED: Codex.flat_methods
    def thread_start(
        self,
        *,
        approval_policy: AskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: SandboxMode | None = None,
        service_name: str | None = None,
        service_tier: ServiceTier | None = None,
    ) -> Thread:
        params = ThreadStartParams(
            approval_policy=approval_policy,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            model_provider=model_provider,
            personality=personality,
            sandbox=sandbox,
            service_name=service_name,
            service_tier=service_tier,
        )
        started = self._client.thread_start(params)
        return Thread(self._client, started.thread.id)

    def thread_list(
        self,
        *,
        archived: bool | None = None,
        cursor: str | None = None,
        cwd: str | None = None,
        limit: int | None = None,
        model_providers: list[str] | None = None,
        search_term: str | None = None,
        sort_key: ThreadSortKey | None = None,
        source_kinds: list[ThreadSourceKind] | None = None,
    ) -> ThreadListResponse:
        params = ThreadListParams(
            archived=archived,
            cursor=cursor,
            cwd=cwd,
            limit=limit,
            model_providers=model_providers,
            search_term=search_term,
            sort_key=sort_key,
            source_kinds=source_kinds,
        )
        return self._client.thread_list(params)

    def thread_resume(
        self,
        thread_id: str,
        *,
        approval_policy: AskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: SandboxMode | None = None,
        service_tier: ServiceTier | None = None,
    ) -> Thread:
        params = ThreadResumeParams(
            thread_id=thread_id,
            approval_policy=approval_policy,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            model=model,
            model_provider=model_provider,
            personality=personality,
            sandbox=sandbox,
            service_tier=service_tier,
        )
        resumed = self._client.thread_resume(thread_id, params)
        return Thread(self._client, resumed.thread.id)

    def thread_fork(
        self,
        thread_id: str,
        *,
        approval_policy: AskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        sandbox: SandboxMode | None = None,
        service_tier: ServiceTier | None = None,
    ) -> Thread:
        params = ThreadForkParams(
            thread_id=thread_id,
            approval_policy=approval_policy,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            model_provider=model_provider,
            sandbox=sandbox,
            service_tier=service_tier,
        )
        forked = self._client.thread_fork(thread_id, params)
        return Thread(self._client, forked.thread.id)

    def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        return self._client.thread_archive(thread_id)

    def thread_unarchive(self, thread_id: str) -> Thread:
        unarchived = self._client.thread_unarchive(thread_id)
        return Thread(self._client, unarchived.thread.id)
    # END GENERATED: Codex.flat_methods

    def models(self, *, include_hidden: bool = False) -> ModelListResponse:
        return self._client.model_list(include_hidden=include_hidden)


class AsyncCodex:
    """Async mirror of :class:`Codex` with matching method shapes."""

    def __init__(self, config: AppServerConfig | None = None) -> None:
        self._client = AsyncAppServerClient(config=config)
        self._init: InitializeResult | None = None
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
                self._init = Codex._parse_initialize(payload)
                self._initialized = True
            except Exception:
                await self._client.close()
                self._init = None
                self._initialized = False
                raise

    @property
    def metadata(self) -> InitializeResult:
        if self._init is None:
            raise RuntimeError(
                "AsyncCodex is not initialized yet. Use `async with AsyncCodex()` or call an async API first."
            )
        return self._init

    async def close(self) -> None:
        await self._client.close()
        self._init = None
        self._initialized = False

    # BEGIN GENERATED: AsyncCodex.flat_methods
    async def thread_start(
        self,
        *,
        approval_policy: AskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: SandboxMode | None = None,
        service_name: str | None = None,
        service_tier: ServiceTier | None = None,
    ) -> AsyncThread:
        await self._ensure_initialized()
        params = ThreadStartParams(
            approval_policy=approval_policy,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            model_provider=model_provider,
            personality=personality,
            sandbox=sandbox,
            service_name=service_name,
            service_tier=service_tier,
        )
        started = await self._client.thread_start(params)
        return AsyncThread(self, started.thread.id)

    async def thread_list(
        self,
        *,
        archived: bool | None = None,
        cursor: str | None = None,
        cwd: str | None = None,
        limit: int | None = None,
        model_providers: list[str] | None = None,
        search_term: str | None = None,
        sort_key: ThreadSortKey | None = None,
        source_kinds: list[ThreadSourceKind] | None = None,
    ) -> ThreadListResponse:
        await self._ensure_initialized()
        params = ThreadListParams(
            archived=archived,
            cursor=cursor,
            cwd=cwd,
            limit=limit,
            model_providers=model_providers,
            search_term=search_term,
            sort_key=sort_key,
            source_kinds=source_kinds,
        )
        return await self._client.thread_list(params)

    async def thread_resume(
        self,
        thread_id: str,
        *,
        approval_policy: AskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: SandboxMode | None = None,
        service_tier: ServiceTier | None = None,
    ) -> AsyncThread:
        await self._ensure_initialized()
        params = ThreadResumeParams(
            thread_id=thread_id,
            approval_policy=approval_policy,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            model=model,
            model_provider=model_provider,
            personality=personality,
            sandbox=sandbox,
            service_tier=service_tier,
        )
        resumed = await self._client.thread_resume(thread_id, params)
        return AsyncThread(self, resumed.thread.id)

    async def thread_fork(
        self,
        thread_id: str,
        *,
        approval_policy: AskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        sandbox: SandboxMode | None = None,
        service_tier: ServiceTier | None = None,
    ) -> AsyncThread:
        await self._ensure_initialized()
        params = ThreadForkParams(
            thread_id=thread_id,
            approval_policy=approval_policy,
            base_instructions=base_instructions,
            config=config,
            cwd=cwd,
            developer_instructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            model_provider=model_provider,
            sandbox=sandbox,
            service_tier=service_tier,
        )
        forked = await self._client.thread_fork(thread_id, params)
        return AsyncThread(self, forked.thread.id)

    async def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        await self._ensure_initialized()
        return await self._client.thread_archive(thread_id)

    async def thread_unarchive(self, thread_id: str) -> AsyncThread:
        await self._ensure_initialized()
        unarchived = await self._client.thread_unarchive(thread_id)
        return AsyncThread(self, unarchived.thread.id)
    # END GENERATED: AsyncCodex.flat_methods

    async def models(self, *, include_hidden: bool = False) -> ModelListResponse:
        await self._ensure_initialized()
        return await self._client.model_list(include_hidden=include_hidden)


@dataclass(slots=True)
class Thread:
    _client: AppServerClient
    id: str

    # BEGIN GENERATED: Thread.flat_methods
    def turn(
        self,
        input: Input,
        *,
        approval_policy: AskForApproval | None = None,
        cwd: str | None = None,
        effort: ReasoningEffort | None = None,
        model: str | None = None,
        output_schema: JsonObject | None = None,
        personality: Personality | None = None,
        sandbox_policy: SandboxPolicy | None = None,
        service_tier: ServiceTier | None = None,
        summary: ReasoningSummary | None = None,
    ) -> Turn:
        wire_input = _to_wire_input(input)
        params = TurnStartParams(
            thread_id=self.id,
            input=wire_input,
            approval_policy=approval_policy,
            cwd=cwd,
            effort=effort,
            model=model,
            output_schema=output_schema,
            personality=personality,
            sandbox_policy=sandbox_policy,
            service_tier=service_tier,
            summary=summary,
        )
        turn = self._client.turn_start(self.id, wire_input, params=params)
        return Turn(self._client, self.id, turn.turn.id)
    # END GENERATED: Thread.flat_methods

    def read(self, *, include_turns: bool = False) -> ThreadReadResponse:
        return self._client.thread_read(self.id, include_turns=include_turns)

    def set_name(self, name: str) -> ThreadSetNameResponse:
        return self._client.thread_set_name(self.id, name)

    def compact(self) -> ThreadCompactStartResponse:
        return self._client.thread_compact(self.id)


@dataclass(slots=True)
class AsyncThread:
    _codex: AsyncCodex
    id: str

    # BEGIN GENERATED: AsyncThread.flat_methods
    async def turn(
        self,
        input: Input,
        *,
        approval_policy: AskForApproval | None = None,
        cwd: str | None = None,
        effort: ReasoningEffort | None = None,
        model: str | None = None,
        output_schema: JsonObject | None = None,
        personality: Personality | None = None,
        sandbox_policy: SandboxPolicy | None = None,
        service_tier: ServiceTier | None = None,
        summary: ReasoningSummary | None = None,
    ) -> AsyncTurn:
        await self._codex._ensure_initialized()
        wire_input = _to_wire_input(input)
        params = TurnStartParams(
            thread_id=self.id,
            input=wire_input,
            approval_policy=approval_policy,
            cwd=cwd,
            effort=effort,
            model=model,
            output_schema=output_schema,
            personality=personality,
            sandbox_policy=sandbox_policy,
            service_tier=service_tier,
            summary=summary,
        )
        turn = await self._codex._client.turn_start(
            self.id,
            wire_input,
            params=params,
        )
        return AsyncTurn(self._codex, self.id, turn.turn.id)
    # END GENERATED: AsyncThread.flat_methods

    async def read(self, *, include_turns: bool = False) -> ThreadReadResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.thread_read(self.id, include_turns=include_turns)

    async def set_name(self, name: str) -> ThreadSetNameResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.thread_set_name(self.id, name)

    async def compact(self) -> ThreadCompactStartResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.thread_compact(self.id)


@dataclass(slots=True)
class Turn:
    _client: AppServerClient
    thread_id: str
    id: str

    def steer(self, input: Input) -> TurnSteerResponse:
        return self._client.turn_steer(self.thread_id, self.id, _to_wire_input(input))

    def interrupt(self) -> TurnInterruptResponse:
        return self._client.turn_interrupt(self.thread_id, self.id)

    def stream(self) -> Iterator[Notification]:
        # TODO: replace this client-wide experimental guard with per-turn event demux.
        self._client.acquire_turn_consumer(self.id)
        try:
            while True:
                event = self._client.next_notification()
                yield event
                if (
                    event.method == "turn/completed"
                    and isinstance(event.payload, TurnCompletedNotificationPayload)
                    and event.payload.turn.id == self.id
                ):
                    break
        finally:
            self._client.release_turn_consumer(self.id)

    def run(self) -> TurnResult:
        completed: TurnCompletedNotificationPayload | None = None
        usage: ThreadTokenUsageUpdatedNotification | None = None
        delta_chunks: list[str] = []
        raw_text_chunks: list[str] = []

        stream = self.stream()
        try:
            for event in stream:
                payload = event.payload
                if (
                    isinstance(payload, AgentMessageDeltaNotification)
                    and payload.turn_id == self.id
                ):
                    delta_chunks.append(payload.delta)
                    continue
                if (
                    isinstance(payload, RawResponseItemCompletedNotification)
                    and payload.turn_id == self.id
                ):
                    raw_text_chunks.extend(_assistant_output_text_chunks(payload))
                    continue
                if (
                    isinstance(payload, ThreadTokenUsageUpdatedNotification)
                    and payload.turn_id == self.id
                ):
                    usage = payload
                    continue
                if (
                    isinstance(payload, TurnCompletedNotificationPayload)
                    and payload.turn.id == self.id
                ):
                    completed = payload
        finally:
            stream.close()

        return _build_turn_result(completed, usage, delta_chunks, raw_text_chunks)


@dataclass(slots=True)
class AsyncTurn:
    _codex: AsyncCodex
    thread_id: str
    id: str

    async def steer(self, input: Input) -> TurnSteerResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.turn_steer(
            self.thread_id,
            self.id,
            _to_wire_input(input),
        )

    async def interrupt(self) -> TurnInterruptResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.turn_interrupt(self.thread_id, self.id)

    async def stream(self) -> AsyncIterator[Notification]:
        await self._codex._ensure_initialized()
        # TODO: replace this client-wide experimental guard with per-turn event demux.
        self._codex._client.acquire_turn_consumer(self.id)
        try:
            while True:
                event = await self._codex._client.next_notification()
                yield event
                if (
                    event.method == "turn/completed"
                    and isinstance(event.payload, TurnCompletedNotificationPayload)
                    and event.payload.turn.id == self.id
                ):
                    break
        finally:
            self._codex._client.release_turn_consumer(self.id)

    async def run(self) -> TurnResult:
        completed: TurnCompletedNotificationPayload | None = None
        usage: ThreadTokenUsageUpdatedNotification | None = None
        delta_chunks: list[str] = []
        raw_text_chunks: list[str] = []

        stream = self.stream()
        try:
            async for event in stream:
                payload = event.payload
                if (
                    isinstance(payload, AgentMessageDeltaNotification)
                    and payload.turn_id == self.id
                ):
                    delta_chunks.append(payload.delta)
                    continue
                if (
                    isinstance(payload, RawResponseItemCompletedNotification)
                    and payload.turn_id == self.id
                ):
                    raw_text_chunks.extend(_assistant_output_text_chunks(payload))
                    continue
                if (
                    isinstance(payload, ThreadTokenUsageUpdatedNotification)
                    and payload.turn_id == self.id
                ):
                    usage = payload
                    continue
                if (
                    isinstance(payload, TurnCompletedNotificationPayload)
                    and payload.turn.id == self.id
                ):
                    completed = payload
        finally:
            await stream.aclose()

        return _build_turn_result(completed, usage, delta_chunks, raw_text_chunks)
