import asyncio
from dataclasses import dataclass

from ._goal import (
    _AsyncGoalNotificationStream,
    _GoalNotificationStream,
    _GoalOperationState,
)
from ._inputs import RunInput, _normalize_run_input, _to_wire_input
from .async_client import AsyncCodexClient
from .client import CodexClient, _active_turn_id_from_error
from .errors import InvalidRequestError
from .generated.v2_all import TurnInterruptResponse, TurnSteerResponse


def _inactive_turn_error() -> InvalidRequestError:
    return InvalidRequestError(-32600, "no active turn to steer")


def _inactive_interrupt_error() -> InvalidRequestError:
    return InvalidRequestError(-32600, "no active turn to interrupt")


@dataclass(slots=True)
class _GoalTurnHandleAdapter:
    """Implement synchronous turn controls for one logical goal operation."""

    client: CodexClient
    state: _GoalOperationState
    thread_id: str
    logical_turn_id: str

    def steer(self, input: RunInput) -> TurnSteerResponse:
        wire_input = _to_wire_input(_normalize_run_input(input))
        turn_id = self.state.active_turn()
        if turn_id is None:
            raise _inactive_turn_error()
        try:
            response = self.client.turn_steer(self.thread_id, turn_id, wire_input)
        except InvalidRequestError as exc:
            if not (
                exc.message == "no active turn to steer"
                or exc.message.startswith("expected active turn id")
            ):
                raise
            next_turn_id = _active_turn_id_from_error(exc)
            if next_turn_id is None:
                next_turn_id = self.state.active_turn(after=turn_id)
            if next_turn_id is None:
                raise _inactive_turn_error() from exc
            response = self.client.turn_steer(self.thread_id, next_turn_id, wire_input)
            self.state.resolve_active_turn(turn_id, next_turn_id)
        return response.model_copy(update={"turn_id": self.logical_turn_id})

    def interrupt(self) -> TurnInterruptResponse:
        if not self.state.begin_interrupt():
            raise _inactive_interrupt_error()
        try:
            self.client.pause_goal(self.thread_id)
            turn_id = self.state.turn_for_interrupt()
            if turn_id is None:
                response = TurnInterruptResponse()
            else:
                try:
                    response = self.client.turn_interrupt(self.thread_id, turn_id)
                except InvalidRequestError as exc:
                    if exc.message == "no active turn to interrupt":
                        response = TurnInterruptResponse()
                    elif exc.message.startswith("expected active turn id"):
                        next_turn_id = _active_turn_id_from_error(exc) or self.state.current_turn()
                        if next_turn_id is None or next_turn_id == turn_id:
                            response = TurnInterruptResponse()
                        else:
                            try:
                                response = self.client.turn_interrupt(
                                    self.thread_id,
                                    next_turn_id,
                                )
                            except InvalidRequestError as retry_exc:
                                if retry_exc.message != "no active turn to interrupt":
                                    raise
                                response = TurnInterruptResponse()
                            self.state.resolve_active_turn(turn_id, next_turn_id)
                    else:
                        raise
            self.state.confirm_interrupt()
            return response
        except BaseException:
            self.state.cancel_interrupt()
            raise

    def stream(self) -> _GoalNotificationStream:
        return _GoalNotificationStream(
            self.state,
            lambda: self.client.next_goal_notification(self.state),
            lambda: self.client.unregister_goal_operation(self.state),
            lambda: self.client.cancel_goal_operation(self.state),
        )


@dataclass(slots=True)
class _AsyncGoalTurnHandleAdapter:
    """Implement asynchronous turn controls for one logical goal operation."""

    client: AsyncCodexClient
    state: _GoalOperationState
    thread_id: str
    logical_turn_id: str

    async def steer(self, input: RunInput) -> TurnSteerResponse:
        wire_input = _to_wire_input(_normalize_run_input(input))
        turn_id = await asyncio.to_thread(self.state.active_turn)
        if turn_id is None:
            raise _inactive_turn_error()
        try:
            response = await self.client.turn_steer(self.thread_id, turn_id, wire_input)
        except InvalidRequestError as exc:
            if not (
                exc.message == "no active turn to steer"
                or exc.message.startswith("expected active turn id")
            ):
                raise
            next_turn_id = _active_turn_id_from_error(exc)
            if next_turn_id is None:
                next_turn_id = await asyncio.to_thread(
                    self.state.active_turn,
                    after=turn_id,
                )
            if next_turn_id is None:
                raise _inactive_turn_error() from exc
            response = await self.client.turn_steer(
                self.thread_id,
                next_turn_id,
                wire_input,
            )
            self.state.resolve_active_turn(turn_id, next_turn_id)
        return response.model_copy(update={"turn_id": self.logical_turn_id})

    async def interrupt(self) -> TurnInterruptResponse:
        if not self.state.begin_interrupt():
            raise _inactive_interrupt_error()
        try:
            await self.client.pause_goal(self.thread_id)
            turn_id = self.state.turn_for_interrupt()
            if turn_id is None:
                response = TurnInterruptResponse()
            else:
                try:
                    response = await self.client.turn_interrupt(self.thread_id, turn_id)
                except InvalidRequestError as exc:
                    if exc.message == "no active turn to interrupt":
                        response = TurnInterruptResponse()
                    elif exc.message.startswith("expected active turn id"):
                        next_turn_id = _active_turn_id_from_error(exc) or self.state.current_turn()
                        if next_turn_id is None or next_turn_id == turn_id:
                            response = TurnInterruptResponse()
                        else:
                            try:
                                response = await self.client.turn_interrupt(
                                    self.thread_id,
                                    next_turn_id,
                                )
                            except InvalidRequestError as retry_exc:
                                if retry_exc.message != "no active turn to interrupt":
                                    raise
                                response = TurnInterruptResponse()
                            self.state.resolve_active_turn(turn_id, next_turn_id)
                    else:
                        raise
            self.state.confirm_interrupt()
            return response
        except BaseException:
            self.state.cancel_interrupt()
            raise

    def stream(self) -> _AsyncGoalNotificationStream:
        return _AsyncGoalNotificationStream(
            self.state,
            lambda: self.client.next_goal_notification(self.state),
            lambda: self.client.unregister_goal_operation(self.state),
            lambda: self.client.cancel_goal_operation(self.state),
        )
