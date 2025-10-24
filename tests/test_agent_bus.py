import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load_module(name: str, rel_path: str):
    path = ROOT / rel_path
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(mod)  # type: ignore[attr-defined]
    return mod


def test_idempotency_key_includes_path_and_bucket():
    agent_bus = load_module("agent_bus", "scripts/agent_bus.py")
    b = agent_bus.ts_bucket(1234, 300)
    assert b == 1200
    a = agent_bus.make_idempotency_key("o/r", 1, "/notify", 1200, "/ci/notify")
    bkey = agent_bus.make_idempotency_key("o/r", 1, "/notify", 1200, "/audit/event")
    assert a != bkey


def test_http_path_allowed():
    agent_bus = load_module("agent_bus", "scripts/agent_bus.py")
    cfg = {"agents": {"http_ops": {"allow_paths": ["/ci/notify"]}}}
    assert agent_bus.http_path_allowed(cfg, "/ci/notify") is True
    assert agent_bus.http_path_allowed(cfg, "/audit/event") is False

