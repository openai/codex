import json
import os
import subprocess


def http_post(base_url: str, path: str, data: dict, headers: dict[str, str] | None = None) -> None:
    if not path.startswith('/'):
        raise ValueError('path must start with /')
    url = base_url.rstrip('/') + path
    hdrs = []
    for k, v in (headers or {}).items():
        hdrs += ["-H", f"{k}: {v}"]
    body = json.dumps(data)
    cmd = ["curl", "-sS", "-X", "POST", *hdrs, "-H", "Content-Type: application/json", "--data", body, url]
    rc = subprocess.call(cmd)
    if rc != 0:
        raise RuntimeError(f"curl POST failed: {rc}")

