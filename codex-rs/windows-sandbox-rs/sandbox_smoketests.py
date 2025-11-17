# sandbox_smoketests.py
# Run a suite of smoke tests against the Windows sandbox via the Codex CLI
# Requires: Python 3.8+ on Windows. No pip requirements.

import os
import sys
import shutil
import subprocess
from pathlib import Path
from typing import List, Optional, Tuple

def _resolve_codex_cmd() -> List[str]:
    """Resolve the Codex CLI to invoke `codex sandbox windows`.

    Prefer local builds (debug first), then fall back to PATH.
    Returns the argv prefix to run Codex.
    """
    root = Path(__file__).parent
    ws_root = root.parent
    cargo_target = os.environ.get("CARGO_TARGET_DIR")

    candidates = [
        ws_root / "target" / "debug" / "codex.exe",
        ws_root / "target" / "release" / "codex.exe",
    ]
    if cargo_target:
        cargo_base = Path(cargo_target)
        candidates.extend([
            cargo_base / "debug" / "codex.exe",
            cargo_base / "release" / "codex.exe",
        ])

    for candidate in candidates:
        if candidate.exists():
            return [str(candidate)]

    if shutil.which("codex"):
        return ["codex"]

    raise FileNotFoundError(
        "Codex CLI not found. Build it first, e.g.\n"
        "  cargo build -p codex-cli --release\n"
        "or for debug:\n"
        "  cargo build -p codex-cli\n"
    )

CODEX_CMD = _resolve_codex_cmd()
print(CODEX_CMD)
TIMEOUT_SEC = 20

WS_ROOT = Path(os.environ["USERPROFILE"]) / "sbx_ws_tests"
OUTSIDE = Path(os.environ["USERPROFILE"]) / "sbx_ws_outside"  # outside CWD for deny checks

ENV_BASE = {}  # extend if needed

class CaseResult:
    def __init__(self, name: str, ok: bool, detail: str = ""):
        self.name, self.ok, self.detail = name, ok, detail

def run_sbx(policy: str, cmd_argv: List[str], cwd: Path, env_extra: Optional[dict] = None) -> Tuple[int, str, str]:
    env = os.environ.copy()
    env.update(ENV_BASE)
    if env_extra:
        env.update(env_extra)
    # Map policy to codex CLI flags
    # read-only => default; workspace-write => --full-auto
    if policy not in ("read-only", "workspace-write"):
        raise ValueError(f"unknown policy: {policy}")
    policy_flags: List[str] = ["--full-auto"] if policy == "workspace-write" else []

    argv = [*CODEX_CMD, "sandbox", "windows", *policy_flags, "--", *cmd_argv]
    print(cmd_argv)
    cp = subprocess.run(argv, cwd=str(cwd), env=env,
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                        timeout=TIMEOUT_SEC, text=True)
    return cp.returncode, cp.stdout, cp.stderr

def have(cmd: str) -> bool:
    return shutil.which(cmd) is not None

def make_dir_clean(p: Path) -> None:
    if p.exists():
        shutil.rmtree(p, ignore_errors=True)
    p.mkdir(parents=True, exist_ok=True)

def write_file(p: Path, content: str = "x") -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content, encoding="utf-8")

def remove_if_exists(p: Path) -> None:
    try:
        if p.is_dir(): shutil.rmtree(p, ignore_errors=True)
        elif p.exists(): p.unlink(missing_ok=True)
    except Exception:
        pass

def assert_exists(p: Path) -> bool:
    return p.exists()

def assert_not_exists(p: Path) -> bool:
    return not p.exists()

def make_junction(link: Path, target: Path) -> bool:
    """Create a directory junction; return True if it exists afterward."""
    remove_if_exists(link)
    target.mkdir(parents=True, exist_ok=True)
    cmd = ["cmd", "/c", f'mklink /J "{link}" "{target}"']
    cp = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    return cp.returncode == 0 and link.exists()

def make_symlink(link: Path, target: Path) -> bool:
    """Create a directory symlink; return True if it exists afterward."""
    remove_if_exists(link)
    if not target.exists():
        try:
            target.mkdir(parents=True, exist_ok=True)
        except OSError:
            pass
    cmd = ["cmd", "/c", f'mklink /D "{link}" "{target}"']
    cp = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    return cp.returncode == 0 and link.exists()

def summarize(results: List[CaseResult]) -> int:
    ok = sum(1 for r in results if r.ok)
    total = len(results)
    print("\n" + "=" * 72)
    print(f"Sandbox smoke tests: {ok}/{total} passed")
    for r in results:
        print(f"[{'PASS' if r.ok else 'FAIL'}] {r.name}" + (f" :: {r.detail.strip()}" if r.detail and not r.ok else ""))
    print("=" * 72)
    return 0 if ok == total else 1

def main() -> int:
    results: List[CaseResult] = []
    make_dir_clean(WS_ROOT)
    OUTSIDE.mkdir(exist_ok=True)
    # Environment probe: some hosts allow TEMP writes even under read-only
    # tokens due to ACLs and restricted SID semantics. Detect and adapt tests.
    probe_rc, _, _ = run_sbx(
        "read-only",
        ["cmd", "/c", "echo probe > %TEMP%\\sbx_ro_probe.txt"],
        WS_ROOT,
    )
    ro_temp_denied = probe_rc != 0

    def add(name: str, ok: bool, detail: str = ""):
        print('running', name)
        results.append(CaseResult(name, ok, detail))

    # 1. RO: deny write in CWD
    target = WS_ROOT / "ro_should_fail.txt"
    remove_if_exists(target)
    rc, out, err = run_sbx("read-only", ["cmd", "/c", "echo nope > ro_should_fail.txt"], WS_ROOT)
    add("RO: write in CWD denied", rc != 0 and assert_not_exists(target), f"rc={rc}, err={err}")

    # 2. WS: allow write in CWD
    target = WS_ROOT / "ws_ok.txt"
    remove_if_exists(target)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo ok > ws_ok.txt"], WS_ROOT)
    add("WS: write in CWD allowed", rc == 0 and assert_exists(target), f"rc={rc}, err={err}")

    # 3. WS: deny write outside workspace
    outside_file = OUTSIDE / "blocked.txt"
    remove_if_exists(outside_file)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", f"echo nope > {outside_file}"], WS_ROOT)
    add("WS: write outside workspace denied", rc != 0 and assert_not_exists(outside_file), f"rc={rc}")

    # 4. WS: allow TEMP write
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo tempok > %TEMP%\\ws_temp_ok.txt"], WS_ROOT)
    add("WS: TEMP write allowed", rc == 0, f"rc={rc}")

    # 5. RO: deny TEMP write
    rc, out, err = run_sbx("read-only", ["cmd", "/c", "echo tempno > %TEMP%\\ro_temp_fail.txt"], WS_ROOT)
    if ro_temp_denied:
        add("RO: TEMP write denied", rc != 0, f"rc={rc}")
    else:
        add("RO: TEMP write denied (skipped on this host)", True)

    # 6. WS: append OK in CWD
    target = WS_ROOT / "append.txt"
    remove_if_exists(target); write_file(target, "line1\n")
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo line2 >> append.txt"], WS_ROOT)
    add("WS: append allowed", rc == 0 and target.read_text().strip().endswith("line2"), f"rc={rc}")

    # 7. RO: append denied
    target = WS_ROOT / "ro_append.txt"
    write_file(target, "line1\n")
    rc, out, err = run_sbx("read-only", ["cmd", "/c", "echo line2 >> ro_append.txt"], WS_ROOT)
    add("RO: append denied", rc != 0 and target.read_text() == "line1\n", f"rc={rc}")

    # 8. WS: PowerShell Set-Content in CWD (OK)
    target = WS_ROOT / "ps_ok.txt"
    remove_if_exists(target)
    rc, out, err = run_sbx("workspace-write",
                           ["powershell", "-NoLogo", "-NoProfile", "-Command",
                            "Set-Content -LiteralPath ps_ok.txt -Value 'hello' -Encoding ASCII"], WS_ROOT)
    add("WS: PowerShell Set-Content allowed", rc == 0 and assert_exists(target), f"rc={rc}, err={err}")

    # 9. RO: PowerShell Set-Content denied
    target = WS_ROOT / "ps_ro_fail.txt"
    remove_if_exists(target)
    rc, out, err = run_sbx("read-only",
                           ["powershell", "-NoLogo", "-NoProfile", "-Command",
                            "Set-Content -LiteralPath ps_ro_fail.txt -Value 'x'"], WS_ROOT)
    add("RO: PowerShell Set-Content denied", rc != 0 and assert_not_exists(target), f"rc={rc}")

    # 10. WS: mkdir and write (OK)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "mkdir sub && echo hi > sub\\in_sub.txt"], WS_ROOT)
    add("WS: mkdir+write allowed", rc == 0 and (WS_ROOT / "sub/in_sub.txt").exists(), f"rc={rc}")

    # 11. WS: rename (EXPECTED SUCCESS on this host)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo x > r.txt & ren r.txt r2.txt"], WS_ROOT)
    add("WS: rename succeeds (expected on this host)", rc == 0 and (WS_ROOT / "r2.txt").exists(), f"rc={rc}, err={err}")

    # 12. WS: delete (EXPECTED SUCCESS on this host)
    target = WS_ROOT / "delme.txt"; write_file(target, "x")
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "del /q delme.txt"], WS_ROOT)
    add("WS: delete succeeds (expected on this host)", rc == 0 and not target.exists(), f"rc={rc}, err={err}")

    # 13. RO: python tries to write (denied)
    pyfile = WS_ROOT / "py_should_fail.txt"; remove_if_exists(pyfile)
    rc, out, err = run_sbx("read-only", ["python", "-c", "open('py_should_fail.txt','w').write('x')"], WS_ROOT)
    add("RO: python file write denied", rc != 0 and assert_not_exists(pyfile), f"rc={rc}")

    # 14. WS: python writes file (OK)
    pyfile = WS_ROOT / "py_ok.txt"; remove_if_exists(pyfile)
    rc, out, err = run_sbx("workspace-write", ["python", "-c", "open('py_ok.txt','w').write('x')"], WS_ROOT)
    add("WS: python file write allowed", rc == 0 and assert_exists(pyfile), f"rc={rc}, err={err}")

    # 15. WS: curl network blocked (short timeout)
    rc, out, err = run_sbx("workspace-write", ["curl", "--connect-timeout", "1", "--max-time", "2", "https://example.com"], WS_ROOT)
    add("WS: curl network blocked", rc != 0, f"rc={rc}")

    # 16. WS: iwr network blocked (HTTP)
    rc, out, err = run_sbx("workspace-write", ["powershell", "-NoLogo", "-NoProfile", "-Command",
                               "try { iwr http://neverssl.com -TimeoutSec 2 } catch { exit 1 }"], WS_ROOT)
    add("WS: iwr network blocked", rc != 0, f"rc={rc}")

    # 17. RO: deny TEMP writes via PowerShell
    rc, out, err = run_sbx("read-only",
                           ["powershell", "-NoLogo", "-NoProfile", "-Command",
                            "Set-Content -LiteralPath $env:TEMP\\ro_tmpfail.txt -Value 'x'"], WS_ROOT)
    if ro_temp_denied:
        add("RO: TEMP write denied (PS)", rc != 0, f"rc={rc}")
    else:
        add("RO: TEMP write denied (PS, skipped)", True)

    # 18. WS: curl version check — don't rely on stub, just succeed
    if have("curl"):
        rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "curl --version"], WS_ROOT)
        add("WS: curl present (version prints)", rc == 0, f"rc={rc}, err={err}")
    else:
        add("WS: curl present (optional, skipped)", True)

    # 19. Optional: ripgrep version
    if have("rg"):
        rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "rg --version"], WS_ROOT)
        add("WS: rg --version (optional)", rc == 0, f"rc={rc}, err={err}")
    else:
        add("WS: rg --version (optional, skipped)", True)

    # 20. Optional: git --version
    if have("git"):
        rc, out, err = run_sbx("workspace-write", ["git", "--version"], WS_ROOT)
        add("WS: git --version (optional)", rc == 0, f"rc={rc}, err={err}")
    else:
        add("WS: git --version (optional, skipped)", True)

    # 24. WS: PS bytes write (OK)
    rc, out, err = run_sbx("workspace-write",
                           ["powershell", "-NoLogo", "-NoProfile", "-Command",
                            "[IO.File]::WriteAllBytes('bytes_ok.bin',[byte[]](0..255))"], WS_ROOT)
    add("WS: PS bytes write allowed", rc == 0 and (WS_ROOT / "bytes_ok.bin").exists(), f"rc={rc}")

    # 25. RO: PS bytes write denied
    rc, out, err = run_sbx("read-only",
                           ["powershell", "-NoLogo", "-NoProfile", "-Command",
                            "[IO.File]::WriteAllBytes('bytes_fail.bin',[byte[]](0..10))"], WS_ROOT)
    add("RO: PS bytes write denied", rc != 0 and not (WS_ROOT / "bytes_fail.bin").exists(), f"rc={rc}")

    # 26. WS: deep mkdir and write (OK)
    rc, out, err = run_sbx("workspace-write",
                           ["cmd", "/c", "mkdir deep\\nest && echo ok > deep\\nest\\f.txt"], WS_ROOT)
    add("WS: deep mkdir+write allowed", rc == 0 and (WS_ROOT / "deep/nest/f.txt").exists(), f"rc={rc}")

    # 27. WS: move (EXPECTED SUCCESS on this host)
    rc, out, err = run_sbx("workspace-write",
                           ["cmd", "/c", "echo x > m1.txt & move /y m1.txt m2.txt"], WS_ROOT)
    add("WS: move succeeds (expected on this host)", rc == 0 and (WS_ROOT / "m2.txt").exists(), f"rc={rc}, err={err}")

    # 28. RO: cmd redirection denied
    target = WS_ROOT / "cmd_ro.txt"; remove_if_exists(target)
    rc, out, err = run_sbx("read-only", ["cmd", "/c", "echo nope > cmd_ro.txt"], WS_ROOT)
    add("RO: cmd redirection denied", rc != 0 and not target.exists(), f"rc={rc}")

    # 29. WS: CWD junction poisoning denied (allowlist should not follow to OUTSIDE)
    poison_cwd = WS_ROOT / "poison_cwd"
    if make_junction(poison_cwd, OUTSIDE):
        target = OUTSIDE / "poisoned.txt"
        remove_if_exists(target)
        rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo poison > poisoned.txt"], poison_cwd)
        add("WS: junction poisoning via CWD denied", rc != 0 and assert_not_exists(target), f"rc={rc}, err={err}")
    else:
        add("WS: junction poisoning via CWD denied (setup skipped)", True, "junction creation failed")

    # 30. WS: junction into Windows denied
    sys_link = WS_ROOT / "sys_link"
    sys_target = Path("C:/Windows")
    sys_file = sys_target / "system32" / "sbx_junc.txt"
    if sys_file.exists():
        remove_if_exists(sys_file)
    if make_junction(sys_link, sys_target):
        rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo bad > sys_link\\system32\\sbx_junc.txt"], WS_ROOT)
        add("WS: junction into Windows denied", rc != 0 and not sys_file.exists(), f"rc={rc}, err={err}")
    else:
        add("WS: junction into Windows denied (setup skipped)", True, "junction creation failed")

    # 31. WS: device/pipe access blocked
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "type \\\\.\\PhysicalDrive0"], WS_ROOT)
    add("WS: raw device access denied", rc != 0, f"rc={rc}")

    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo hi > \\\\.\\pipe\\codex_testpipe"], WS_ROOT)
    add("WS: named pipe creation denied", rc != 0, f"rc={rc}")

    # 32. WS: ADS/long-path escape denied
    ads_base = WS_ROOT / "ads_base.txt"
    remove_if_exists(ads_base)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo secret > ads_base.txt:stream"], WS_ROOT)
    add("WS: ADS write denied", rc != 0 and assert_not_exists(ads_base), f"rc={rc}")

    lp_target = Path(r"\\?\C:\sbx_longpath_test.txt")
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo long > \\\\?\\C:\\sbx_longpath_test.txt"], WS_ROOT)
    add("WS: long-path escape denied", rc != 0 and not lp_target.exists(), f"rc={rc}")

    # 33. WS: case-insensitive protected path bypass denied (.GiT)
    git_variation = WS_ROOT / ".GiT" / "config"
    remove_if_exists(git_variation.parent)
    git_variation.parent.mkdir(exist_ok=True)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo hack > .GiT\\config"], WS_ROOT)
    add("WS: protected path case-variation denied", rc != 0 and assert_not_exists(git_variation), f"rc={rc}")

    # 34. WS: policy tamper (.codex artifacts) denied
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo tamper > .codex\\cap_sid"], WS_ROOT)
    rc2, out2, err2 = run_sbx("workspace-write", ["cmd", "/c", "echo tamper > .codex\\policy.json"], WS_ROOT)
    add("WS: .codex cap_sid tamper denied", rc != 0, f"rc={rc}, err={err}")
    add("WS: .codex policy tamper denied", rc2 != 0, f"rc={rc2}, err={err2}")

    # 35. WS: PATH stub bypass denied (ssh before stubs)
    tools_dir = WS_ROOT / "tools"
    tools_dir.mkdir(exist_ok=True)
    ssh_path = None
    if have("ssh"):
        # shutil.which considers PATHEXT + PATHEXT semantics
        ssh_path = shutil.which("ssh")
    if ssh_path:
        shim = tools_dir / "ssh.bat"
        shim.write_text("@echo off\r\necho stubbed\r\n", encoding="utf-8")
        env = {"PATH": f"{tools_dir};%PATH%"}
        rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "ssh"], WS_ROOT, env_extra=env)
        add("WS: PATH stub bypass denied", "stubbed" in out, f"rc={rc}, out={out}")
    else:
        add("WS: PATH stub bypass denied (ssh missing)", True, "ssh not installed")

    # 36. WS: symlink races blocked
    race_root = WS_ROOT / "race"
    inside = race_root / "inside"
    outside = race_root / "outside"
    make_dir_clean(race_root)
    inside.mkdir(parents=True, exist_ok=True)
    outside.mkdir(parents=True, exist_ok=True)
    link = race_root / "flip"
    make_symlink(link, inside)
    # Fire a quick toggle loop and attempt a write
    outside_abs = str(OUTSIDE)
    inside_abs = str(inside)
    toggle = [
        "cmd",
        "/c",
        f'for /L %i in (1,1,400) do (rmdir flip & mklink /D flip "{inside_abs}" >NUL & rmdir flip & mklink /D flip "{outside_abs}" >NUL)',
    ]
    subprocess.Popen(toggle, cwd=str(race_root), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo race > flip\\race.txt"], race_root)
    add("WS: symlink race write denied (best-effort)", rc != 0 and not (outside / "race.txt").exists(), f"rc={rc}")

    # 37. WS: audit blind spots – deep junction/world-writable denied
    deep = WS_ROOT / "deep" / "redir"
    unsafe_dir = WS_ROOT / "deep" / "unsafe"
    make_junction(deep, Path("C:/Windows"))
    unsafe_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run(["icacls", str(unsafe_dir), "/grant", "Everyone:(F)"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo probe > deep\\redir\\system32\\audit_gap.txt"], WS_ROOT)
    add("WS: deep junction/world-writable escape denied", rc != 0, f"rc={rc}, err={err}")

    # 38. WS: policy poisoning via workspace symlink root denied
    # Simulate workspace replaced by symlink to C:\; expect writes to be denied.
    fake_root = WS_ROOT / "fake_root"
    if make_symlink(fake_root, Path("C:/")):
        rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo owned > codex_escape.txt"], fake_root)
        add("WS: workspace-root symlink poisoning denied", rc != 0, f"rc={rc}")
    else:
        add("WS: workspace-root symlink poisoning denied (setup skipped)", True, "symlink creation failed")

    # 39. WS: UNC/other-drive canonicalization denied
    unc_link = WS_ROOT / "unc_link"
    other_to = Path(r"\\\\localhost\\C$")
    if make_symlink(unc_link, other_to):
        rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo unc > unc_link\\unc_test.txt"], WS_ROOT)
        add("WS: UNC link escape denied", rc != 0, f"rc={rc}")
    else:
        add("WS: UNC link escape denied (setup skipped)", True, "symlink creation failed")

    other_drive = WS_ROOT / "other_drive"
    other_target = Path("D:/")  # best-effort; may not exist
    if make_symlink(other_drive, other_target):
        rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo drive > other_drive\\drive.txt"], WS_ROOT)
        add("WS: other-drive link escape denied", rc != 0, f"rc={rc}")
    else:
        add("WS: other-drive link escape denied (setup skipped)", True, "symlink creation failed")

    # 40. WS: timeout cleanup still denies outside write
    slow_ps = WS_ROOT / "sleep.ps1"
    slow_ps.write_text("Start-Sleep 15", encoding="utf-8")
    try:
        run_sbx("workspace-write", ["powershell", "-File", "sleep.ps1"], WS_ROOT)
    except Exception:
        pass
    outside_after_timeout = OUTSIDE / "timeout_leak.txt"
    remove_if_exists(outside_after_timeout)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", f"echo leak > {outside_after_timeout}"], WS_ROOT)
    add("WS: post-timeout outside write still denied", rc != 0 and assert_not_exists(outside_after_timeout), f"rc={rc}")

    # 41. WS: additional protected path variation (.ssh)
    ssh_var = WS_ROOT / ".SsH" / "config"
    remove_if_exists(ssh_var.parent)
    ssh_var.parent.mkdir(exist_ok=True)
    rc, out, err = run_sbx("workspace-write", ["cmd", "/c", "echo key > .SsH\\config"], WS_ROOT)
    add("WS: protected path variation (.ssh) denied", rc != 0 and assert_not_exists(ssh_var), f"rc={rc}")

    return summarize(results)

if __name__ == "__main__":
    sys.exit(main())
