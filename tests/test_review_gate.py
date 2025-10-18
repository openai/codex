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


def test_actor_in_owners_required():
    gate = load_module("review_gate", "scripts/review_gate.py")
    cfg = {"review": {"owners": ["@alice", "bob"]}}
    assert gate.actor_in_owners(cfg, "alice") is True
    assert gate.actor_in_owners(cfg, "carol") is False
    # Owners required
    assert gate.actor_in_owners({"review": {}}, "alice") is False

