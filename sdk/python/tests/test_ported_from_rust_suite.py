"""Ported core scenarios from codex-rs/app-server/tests/suite/*

These tests mirror the intent of Rust suite cases in a transport-focused way:
- create_thread
- list_resume
- interrupt
"""

from codex_app_server import AppServerClient, AppServerConfig
from pathlib import Path

FAKE = Path(__file__).parent / "fake_app_server.py"


def client() -> AppServerClient:
    return AppServerClient(AppServerConfig(launch_args_override=("python3", str(FAKE))))


def test_create_thread_port():
    with client() as c:
        c.initialize()
        res = c.thread_start(model="gpt-5")
        assert "thread" in res and res["thread"]["id"].startswith("thr_")


def test_list_and_resume_port():
    with client() as c:
        c.initialize()
        out = c.thread_list(limit=2)
        assert isinstance(out["data"], list)
        resumed = c.thread_resume("thr_existing")
        assert resumed["thread"]["id"] == "thr_existing"


def test_interrupt_port():
    with client() as c:
        c.initialize()
        tid = c.thread_start()["thread"]["id"]
        turn = c.turn_start(tid, input_items=[{"type": "text", "text": "run"}])
        assert c.turn_interrupt(tid, turn["turn"]["id"]) == {}
