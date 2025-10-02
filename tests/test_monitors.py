import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load_module(name: str, rel_path: str):
    path = ROOT / rel_path
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(mod)  # type: ignore[attr-defined]
    return mod


def test_fingerprint_raw_and_json_path():
    mon = load_module("monitors_poll", "scripts/monitors_poll.py")
    raw1 = json.dumps({"status": "ok", "n": 1})
    raw2 = json.dumps({"status": "ok", "n": 2})
    # Without json_path, different bodies produce different fingerprints
    fp1 = mon.fingerprint({}, raw1)
    fp2 = mon.fingerprint({}, raw2)
    assert fp1 != fp2
    # With json_path, only the selected value matters
    cfg = {"json_path": "status"}
    fp3 = mon.fingerprint(cfg, raw1)
    fp4 = mon.fingerprint(cfg, raw2.replace("\"status\":\"ok\"", "\"status\":\"ok\""))
    assert fp3 == fp4

