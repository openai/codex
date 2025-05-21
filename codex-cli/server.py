#!/usr/bin/env python3
import asyncio
import logging
import os
import sys
from dataclasses import dataclass, asdict
from enum import Enum
from typing import Dict, Optional

import httpx
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
import uvicorn
from rich.console import Console
from rich.table import Table
from rich.prompt import Prompt


# Shared stuff
class Decision(str, Enum):
    YES = "yes"
    NO_CONTINUE = "no-continue"
    NO_EXIT = "no-exit"
    ALWAYS = "always"


class PermissionRequest(BaseModel):
    request_id: str
    message: str


class PermissionResponse(BaseModel):
    decision: Decision
    customDenyMessage: str = ""


@dataclass
class RequestState:
    message: str
    event: asyncio.Event
    response: Optional[PermissionResponse] = None


pending_requests: Dict[str, RequestState] = {}

# Server app
fastapi_app = FastAPI()


@fastapi_app.get("/pending")
async def list_pending():
    return [{"request_id": rid, "message": st.message} for rid, st in pending_requests.items()]

@fastapi_app.post("/ask")
async def ask(req: PermissionRequest):
    if req.request_id in pending_requests:
        raise HTTPException(400, "Duplicate request_id")
        
    state = RequestState(req.message, asyncio.Event())
    pending_requests[req.request_id] = state

    print(f"🟡 New request: {req.request_id}", flush=True)
    print(f"🧠 {req.message}")
    print("⏳ Waiting for response... (run `python server.py respond`)")

    await state.event.wait()
    if state.response is None:
        raise HTTPException(500, "No response received")
    return state.response


@fastapi_app.post("/answer/{request_id}")
async def answer(request_id: str, resp: PermissionResponse):
    state = pending_requests.get(request_id)
    if not state:
        raise HTTPException(404, "No such request_id")

    state.response = resp
    state.event.set()
    pending_requests.pop(request_id)

    print(f"✅ Answer for {request_id}: {resp.decision}")
    if resp.customDenyMessage:
        print(f"🗒️  {resp.customDenyMessage}")
    return {"status": "ok"}


# CLI tools
console = Console()
async def respond_cli():
    async with httpx.AsyncClient() as client:
        try:
            resp = await client.get("http://localhost:8000/pending")
            resp.raise_for_status()
            pending = resp.json()
        except Exception as e:
            console.print(f"[red]❌ Failed to fetch pending requests: {e}")
            return

        if not pending:
            console.print("[green]✅ No pending requests.")
            return

        table = Table(title="Pending Requests")
        table.add_column("#", justify="right")
        table.add_column("Request ID")
        table.add_column("Message")
        for i, req in enumerate(pending, start=1):
            table.add_row(str(i), req["request_id"], req["message"])
        console.print(table)

        choice = (
            int(
                Prompt.ask(
                    "Pick a request",
                    choices=[str(i) for i in range(1, len(pending) + 1)],
                )
            )
            - 1
        )
        request_id = pending[choice]["request_id"]

        shortcuts = {
            "y": Decision.YES,
            "n": Decision.NO_CONTINUE,
            "x": Decision.NO_EXIT,
            "a": Decision.ALWAYS,
        }
        prompt = "Decision (y=yes, n=no-continue, x=no-exit, a=always)"
        while True:
            key = Prompt.ask(prompt, default="y").strip().lower()
            decision = shortcuts.get(key) or next((d for d in Decision if d.value.startswith(key)), None)
            if decision:
                break
            console.print(f"[red]Invalid choice: {key}")

        deny_msg = ""
        if decision == Decision.NO_CONTINUE:
            deny_msg = Prompt.ask("Custom deny message").strip()

        payload = {"decision": decision.value, "customDenyMessage": deny_msg}
        try:
            post = await client.post(
                f"http://localhost:8000/answer/{request_id}",
                json=payload,
                timeout=5,
            )
            post.raise_for_status()
            console.print("[green]✅ Response sent.")
        except Exception as e:
            console.print(f"[red]❌ Failed to send response: {e}")


# Entrypoint
if __name__ == "__main__":
    if len(sys.argv) == 1:
        # Default: run server
        uvicorn.run(fastapi_app, host="0.0.0.0", port=8000, reload=False)
    elif sys.argv[1] == "respond":
        asyncio.run(respond_cli())
    else:
        print("Usage:")
        print("  python server.py         # Start the FastAPI server")
        print("  python server.py respond # Respond to pending requests")
