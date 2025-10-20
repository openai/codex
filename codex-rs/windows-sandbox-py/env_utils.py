from __future__ import annotations

from typing import Dict

WINDOWS_NULL_DEVICE = "NUL"


def is_unix_null_path(value: str) -> bool:
    trimmed = value.strip()
    return trimmed.lower() in {"/dev/null", r"\\dev\\null"}


def normalize_null_device_env(env_map: Dict[str, str]) -> None:
    for key, value in list(env_map.items()):
        if is_unix_null_path(value):
            env_map[key] = WINDOWS_NULL_DEVICE


def ensure_non_interactive_pager(env_map: Dict[str, str]) -> None:
    env_map.setdefault("GIT_PAGER", "more.com")
    env_map.setdefault("PAGER", "more.com")
    env_map.setdefault("LESS", "")


def apply_best_effort_network_block(env_map: Dict[str, str]) -> None:
    sink = "http://127.0.0.1:9"
    env_map.setdefault("HTTP_PROXY", sink)
    env_map.setdefault("HTTPS_PROXY", sink)
    env_map.setdefault("ALL_PROXY", sink)
    env_map.setdefault("NO_PROXY", "localhost,127.0.0.1,::1")
    env_map.setdefault("PIP_NO_INDEX", "1")
    env_map.setdefault("PIP_DISABLE_PIP_VERSION_CHECK", "1")
    env_map.setdefault("NPM_CONFIG_OFFLINE", "true")
    env_map.setdefault("CARGO_NET_OFFLINE", "true")

