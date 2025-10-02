#!/usr/bin/env python3
import json
import os
import re
import subprocess
import time
from pathlib import Path

try:
    import tomllib
except Exception as e:
    print(f"Python 3.11+ required (tomllib): {e}")
    raise SystemExit(0)

ROOT = Path(__file__).resolve().parent.parent
CFG = ROOT / 'local/automation/monitors.toml'
STATE = ROOT / '.git/.monitors_state.json'


def load_cfg():
    if not CFG.exists():
        print(f"no monitors config: {CFG}")
        return {}
    with CFG.open('rb') as f:
        return tomllib.load(f)


def load_state():
    if not STATE.exists():
        return {}
    try:
        return json.loads(STATE.read_text())
    except Exception:
        return {}


def save_state(state: dict):
    STATE.parent.mkdir(parents=True, exist_ok=True)
    STATE.write_text(json.dumps(state))


def resolve_headers(hdrs: dict[str, str]) -> dict[str, str]:
    out = {}
    for k, v in (hdrs or {}).items():
        # Expand ${ENV:VAR}
        m = re.findall(r"\$\{ENV:([A-Z0-9_]+)\}", v or '')
        vv = v
        for var in m:
            ev = os.environ.get(var)
            if not ev:
                raise RuntimeError(f"missing env for header {k}: {var}")
            vv = vv.replace(f"${{ENV:{var}}}", ev)
        out[k] = vv
    return out


def http_request(method: str, url: str, headers: dict[str, str] | None = None, data: str | None = None) -> tuple[int, str]:
    cmd = ["curl", "-sS", "-m", "10", "-X", method.upper()]
    for hk, hv in (headers or {}).items():
        cmd += ["-H", f"{hk}: {hv}"]
    if data:
        cmd += ["--data", data]
    cmd.append(url)
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    out, err = p.communicate()
    if p.returncode != 0:
        raise RuntimeError(f"curl failed: {p.returncode}\n{err}")
    return p.returncode, out


def extract_json_path(obj: dict, path: str):
    # very small JSON path: 'a.b[0].c'
    cur = obj
    try:
        for seg in re.split(r"\.", path.strip().strip('$')):
            m = re.match(r"([A-Za-z0-9_\-]+)(\[(\d+)\])?", seg)
            if not m:
                return None
            key = m.group(1)
            idx = m.group(3)
            cur = cur.get(key)
            if idx is not None:
                cur = cur[int(idx)]
        return cur
    except Exception:
        return None


def matches(cfg: dict, raw: str) -> bool:
    if cfg.get('match_any_contains'):
        return any(s in raw for s in cfg['match_any_contains'])
    if cfg.get('json_path'):
        try:
            js = json.loads(raw)
        except Exception:
            return False
        val = extract_json_path(js, cfg['json_path'])
        op = cfg.get('operator', 'eq')
        target = cfg.get('value')
        if op == 'eq':
            return str(val) == str(target)
        if op == 'ne':
            return str(val) != str(target)
        if op == 'contains':
            return target in str(val)
        return False
    return False


def notify_bus(command: str, payload: dict | None = None):
    env = os.environ.copy()
    env['AGENT_BUS_COMMAND'] = command
    if payload:
        env['AGENT_BUS_PAYLOAD'] = json.dumps(payload)
    p = subprocess.Popen(["python3", str(ROOT / 'scripts/agent_bus.py')], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
    out, err = p.communicate()
    print(out)
    if p.returncode != 0:
        print(err, file=sys.stderr)


def main():
    cfg = load_cfg()
    state = load_state()
    monitors = cfg.get('monitor', {})
    now = time.time()
    for name, m in monitors.items():
        last = state.get(name, 0)
        interval = int(m.get('poll_interval_sec', 300))
        if now - last < interval:
            continue
        try:
            headers = resolve_headers(m.get('headers', {}))
            _, out = http_request(m.get('method', 'GET'), m['url'], headers, None)
            if matches(m, out):
                notify_bus(m.get('route_command', '/notify'), payload={"monitor": name, "sample": out[:1000]})
        except Exception as e:
            print(f"monitor {name} error: {e}")
        finally:
            state[name] = now
    save_state(state)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

