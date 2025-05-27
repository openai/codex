from enum import Enum
from typing import Optional
import socketio
from fastapi import FastAPI
from pydantic import BaseModel, ValidationError
from rich.console import Console
from rich.prompt import Prompt
import asyncio

console = Console()


class PermissionRequest(BaseModel):
    agentId: str
    message: str


class Decision(str, Enum):
    YES = "yes"
    NO_CONTINUE = "no-continue"
    NO_EXIT = "no-exit"
    EXPLAIN = "explain"


class PermissionResponse(BaseModel):
    agentId: str
    decision: Decision
    customDenyMessage: Optional[str] = None

    class Config:
        use_enum_values = True


# socket.io + FastAPI setup
sio = socketio.AsyncServer(
    cors_allowed_origins="*",
    async_mode="asgi",
    # logger=True,
    # engineio_logger=True,
    ping_interval=25,
    ping_timeout=60,
)
app = FastAPI()
app.mount("/", socketio.ASGIApp(sio, socketio_path="socket.io"))


@sio.event
async def connect(sid, environ):
    console.log(f"[green]Client connected:[/green] {sid}")


@sio.on("permission_request")
async def on_permission_request(sid, data):
    try:
        req = PermissionRequest.parse_obj(data)
    except ValidationError as exc:
        console.log(f"[red]Invalid request from {sid}[/red]: {exc}")
        return

    console.print(f"🟡 New request from [bold]{req.agentId}[/bold]: {req.message}")

    # Run Prompt.ask in a thread
    choice = await asyncio.to_thread(Prompt.ask, "Reply", choices=[d.value for d in Decision], show_choices=True)

    custom_msg = None
    if choice == Decision.NO_CONTINUE.value:
        custom_msg = (await asyncio.to_thread(Prompt.ask, "Custom deny message", default="")).strip() or None

    resp = PermissionResponse(agentId=req.agentId, decision=Decision(choice), customDenyMessage=custom_msg)
    await sio.emit("permission_response", resp.dict(exclude_none=True), room=sid)
