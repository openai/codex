import json
import os
import subprocess
import hmac
import hashlib


def http_post(
    base_url: str,
    path: str,
    data: dict,
    headers: dict[str, str] | None = None,
    allowed_paths: list[str] | None = None,
    hmac_secret_env: str | None = None,
    hmac_header: str = "X-Hub-Signature-256",
    timeout_sec: int = 5,
    retries: int = 3,
) -> None:
    if not base_url:
        raise ValueError("base_url is required")
    if not path.startswith("/"):
        raise ValueError("path must start with /")
    if allowed_paths is not None and path not in set(allowed_paths):
        raise PermissionError(f"path not allowed: {path}")

    url = base_url.rstrip("/") + path
    body = json.dumps(data, separators=(",", ":"))

    hdrs_vec = []
    # User-provided headers (allowlisted by caller/config)
    for k, v in (headers or {}).items():
        hdrs_vec += ["-H", f"{k}: {v}"]

    # Optional HMAC signing (sha256) of the body
    if hmac_secret_env:
        secret = os.environ.get(hmac_secret_env, "")
        if secret:
            sig = hmac.new(secret.encode("utf-8"), body.encode("utf-8"), hashlib.sha256).hexdigest()
            hdrs_vec += ["-H", f"{hmac_header}: sha256={sig}"]

    cmd = [
        "curl",
        "-sS",
        "--fail-with-body",
        "-m",
        str(timeout_sec),
        "--retry-all-errors",
        "--retry",
        str(retries),
        "--retry-delay",
        "1",
        "-X",
        "POST",
        *hdrs_vec,
        "-H",
        "Content-Type: application/json",
        "--data",
        body,
        url,
    ]
    rc = subprocess.call(cmd)
    if rc != 0:
        raise RuntimeError(f"curl POST failed: {rc}")
