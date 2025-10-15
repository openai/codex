# network_sandbox.py
# Non-admin “best effort” network blocking helpers for Windows sandboxes.

import os
from dataclasses import dataclass, field
from typing import Dict, Iterable, Optional, Sequence

try:
    import winreg  # type: ignore
except Exception:
    winreg = None  # WinINET proxy helper won’t be available on non-Windows hosts


@dataclass
class NoNetConfig:
    """
    Configuration for non-admin network blocking.
    - env sinkhole: HTTP(S) proxies to 127.0.0.1:9, tool-specific offline toggles
    - denybin: prepend a folder of 'deny' stubs (ssh/curl/wget...) to PATH
    - wininet_proxy: optional per-user WinINET proxy (HKCU) while the process runs
      NOTE: this is user-global for the duration of the run; use sparingly.
    """
    use_env_sinkhole: bool = True
    use_denybin: bool = True
    tools_to_block: Sequence[str] = field(default_factory=lambda: (
        "ssh", "scp", "curl", "wget"
    ))
    denybin_dir: Optional[str] = None  # default: %USERPROFILE%\.sbx-denybin
    use_wininet_proxy: bool = False
    wininet_proxy_hostport: str = "127.0.0.1:9"
    # Extra language/tool toggles
    enable_pip_offline: bool = True
    enable_npm_offline: bool = True
    enable_cargo_offline: bool = True
    enable_go_offline: bool = True
    enable_git_proxy: bool = True


def _ensure_dir(p: str) -> str:
    os.makedirs(p, exist_ok=True)
    return p


def _make_deny_stub_bat(path: str) -> None:
    # Minimal .bat/.cmd stub that exits with failure
    with open(path, "w", encoding="ascii", newline="\r\n") as f:
        f.write("@echo off\r\nexit /b 1\r\n")


def _ensure_denybin(tools: Iterable[str], denybin_dir: Optional[str]) -> str:
    """
    Create a denybin folder with .BAT and .CMD stubs for each tool.
    Combined with PATHEXT reordering, these stubs take precedence over .EXE.
    """
    base = denybin_dir or os.path.join(os.path.expanduser("~"), ".sbx-denybin")
    _ensure_dir(base)
    for tool in tools:
        for ext in (".bat", ".cmd"):
            stub = os.path.join(base, f"{tool}{ext}")
            if not os.path.exists(stub):
                _make_deny_stub_bat(stub)
    return base


def _prepend_path(env: Dict[str, str], prefix: str) -> None:
    sep = os.pathsep
    existing = env.get("PATH") or os.environ.get("PATH") or ""
    parts = existing.split(sep) if existing else []
    # Avoid duplicates; ensure prefix is at the very front
    if not parts or os.path.normcase(parts[0]) != os.path.normcase(prefix):
        env["PATH"] = prefix + (sep + existing if existing else "")


def _reorder_pathext_for_stubs(env: Dict[str, str]) -> None:
    """
    Move .BAT/.CMD ahead of .EXE in PATHEXT so our stubs beat real executables.
    """
    default = env.get("PATHEXT") or os.environ.get("PATHEXT") or ".COM;.EXE;.BAT;.CMD"
    exts = [e for e in default.split(";") if e]
    # Normalize to upper-case (PATHEXT comparisons are case-insensitive)
    exts_norm = [e.upper() for e in exts]
    want = [".BAT", ".CMD"]
    front = [e for e in want if e in exts_norm]
    rest = [exts[i] for i in range(len(exts)) if exts_norm[i] not in want]
    env["PATHEXT"] = ";".join(front + rest)


def _apply_env_sinkhole(env: Dict[str, str], cfg: NoNetConfig) -> None:
    # Generic proxies (widely honored)
    env.setdefault("HTTP_PROXY", "http://127.0.0.1:9")
    env.setdefault("HTTPS_PROXY", "http://127.0.0.1:9")
    env.setdefault("ALL_PROXY", "http://127.0.0.1:9")
    env.setdefault("NO_PROXY", "localhost,127.0.0.1,::1")

    # Git (HTTP/S). SSH is handled by deny stubs and/or GIT_SSH_COMMAND.
    if cfg.enable_git_proxy:
        env.setdefault("GIT_HTTP_PROXY", "http://127.0.0.1:9")
        env.setdefault("GIT_HTTPS_PROXY", "http://127.0.0.1:9")
        env.setdefault("GIT_SSH_COMMAND", "cmd /c exit 1")
        # Optional: block custom protocols over git (empty allowlist)
        env.setdefault("GIT_ALLOW_PROTOCOLS", "")

    # pip / Python packaging
    if cfg.enable_pip_offline:
        env.setdefault("PIP_NO_INDEX", "1")
        env.setdefault("PIP_DISABLE_PIP_VERSION_CHECK", "1")

    # npm / yarn / pnpm (envs are often read in lowercase too)
    if cfg.enable_npm_offline:
        env.setdefault("NPM_CONFIG_OFFLINE", "true")
        env.setdefault("npm_config_proxy", "http://127.0.0.1:9")
        env.setdefault("npm_config_https_proxy", "http://127.0.0.1:9")

    # cargo
    if cfg.enable_cargo_offline:
        env.setdefault("CARGO_NET_OFFLINE", "true")

    # go
    if cfg.enable_go_offline:
        env.setdefault("GOPROXY", "off")

    # dotnet/NuGet: reduce network activity; full lock-down needs a custom config
    env.setdefault("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1")
    env.setdefault("DOTNET_CLI_TELEMETRY_OPTOUT", "1")


class WinInetProxyGuard:
    """
    Optional: per-user WinINET proxy sink (HKCU). Use only if you need to
    catch apps that ignore env proxies but honor WinINET (e.g., many .NET apps).
    WARNING: This flips settings for the *current user profile* while active.
    """

    KEY = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings"

    def __init__(self, hostport: str):
        self.hostport = hostport
        self._prev = None

    def __enter__(self):
        if winreg is None:
            return self
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.KEY, 0, winreg.KEY_READ) as k:
                prev_enable = _reg_get_dword(k, "ProxyEnable", 0)
                prev_server = _reg_get_str(k, "ProxyServer", "")
                prev_override = _reg_get_str(k, "ProxyOverride", "")
                self._prev = (prev_enable, prev_server, prev_override)
        except OSError:
            self._prev = (0, "", "")
        # Set sinkhole
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.KEY, 0, winreg.KEY_SET_VALUE) as k:
                winreg.SetValueEx(k, "ProxyEnable", 0, winreg.REG_DWORD, 1)
                winreg.SetValueEx(k, "ProxyServer", 0, winreg.REG_SZ, self.hostport)
                winreg.SetValueEx(k, "ProxyOverride", 0, winreg.REG_SZ, "")
        except OSError:
            pass  # best effort
        return self

    def __exit__(self, exc_type, exc, tb):
        if winreg is None or self._prev is None:
            return
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.KEY, 0, winreg.KEY_SET_VALUE) as k:
                winreg.SetValueEx(k, "ProxyEnable", 0, winreg.REG_DWORD, int(self._prev[0]))
                winreg.SetValueEx(k, "ProxyServer", 0, winreg.REG_SZ, self._prev[1])
                winreg.SetValueEx(k, "ProxyOverride", 0, winreg.REG_SZ, self._prev[2])
        except OSError:
            pass


def _reg_get_dword(key, name: str, default: int) -> int:
    try:
        v, t = winreg.QueryValueEx(key, name)
        return int(v) if t == winreg.REG_DWORD else default
    except OSError:
        return default


def _reg_get_str(key, name: str, default: str) -> str:
    try:
        v, _ = winreg.QueryValueEx(key, name)
        return str(v)
    except OSError:
        return default


def apply_no_network_to_env(env_map: Dict[str, str], cfg: Optional[NoNetConfig] = None) -> None:
    """
    Mutate the child environment map to strongly discourage network access
    without requiring elevation. Safe to call multiple times.

    - Sets proxy/env offline sinkholes (HTTP(S), pip/npm/cargo/go, git)
    - Creates deny stubs (ssh/curl/wget/…) and prepends them to PATH
    - Reorders PATHEXT so .BAT/.CMD take precedence over .EXE (stubs win)
    - Optionally (cfg.use_wininet_proxy) use WinInetProxyGuard in parent

    IMPORTANT: WinINET changes are user-global while active; we do not apply
    them automatically here—wrap your spawn in WinInetProxyGuard if needed.
    """
    cfg = cfg or NoNetConfig()

    # Marker for easy in-child verification
    env_map["SBX_NONET_ACTIVE"] = "1"

    if cfg.use_env_sinkhole:
        _apply_env_sinkhole(env_map, cfg)

    if cfg.use_denybin:
        denybin = _ensure_denybin(cfg.tools_to_block, cfg.denybin_dir)
        _prepend_path(env_map, denybin)
        _reorder_pathext_for_stubs(env_map)

    # NOTE: We do NOT toggle WinINET here (it affects the parent/user).
    # If you want it:
    #   with WinInetProxyGuard(cfg.wininet_proxy_hostport):
    #       ...spawn the child...
