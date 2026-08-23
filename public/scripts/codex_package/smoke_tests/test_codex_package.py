"""Exercise each assembled package in the order its shipped files appear.

ROOT_OF_EXTRACTED_PACKAGE
├── bin
│   ├── codex[.exe]                       # CLI package only
│   ├── codex-app-server[.exe]            # app-server package only
│   └── codex-code-mode-host[.exe]
├── codex-package.json
├── codex-path
│   └── rg[.exe]
└── codex-resources
    ├── bwrap                             # Linux only
    ├── codex-command-runner.exe          # Windows only
    ├── codex-windows-sandbox-setup.exe   # Windows only
    └── zsh/bin/zsh                       # supported Unix targets only

Debug symbols for all shipped binaries arrive in a separate companion archive.
Each package contains one entrypoint, not both codex and codex-app-server.
"""

import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest
from app_server_harness import (
    MockResponsesServer,
    ev_completed,
    ev_response_created,
    sse,
)
from openai_codex import ApprovalMode, Codex, CodexConfig, Sandbox

from fixtures import SmokePackage


@pytest.mark.parametrize(
    ("arguments", "expected"),
    [
        pytest.param(("--help",), "Usage:", id="help"),
        pytest.param(("--version",), None, id="version"),
        pytest.param(("features", "list"), "code_mode", id="features"),
        pytest.param(("completion", "bash"), "codex", id="completion"),
    ],
)
def test_cli_public_commands(
    package: SmokePackage,
    arguments: tuple[str, ...],
    expected: str | None,
) -> None:
    """Packaged CLI remains usable for discovery, version, features, and completions."""
    output = package.run(*arguments).stdout
    assert expected in output if expected is not None else output.strip()


@pytest.mark.parametrize("entrypoint", ["codex", "codex-app-server"])
def test_app_server_runs_code_mode_through_python_sdk(
    package: SmokePackage,
    responses_server: MockResponsesServer,
    entrypoint: str,
) -> None:
    """Both packages run their own ripgrep through sandboxed SDK code mode."""
    windows = "windows" in package.target
    sandbox_config = ("--config", 'windows.sandbox="unelevated"') if windows else ()
    if entrypoint == "codex":
        executable = package.cli
        package_root = package.cli_root
        package_path_dir = package.cli_path_dir
        config = CodexConfig(
            codex_bin=str(executable),
            config_overrides=sandbox_config[1:],
            cwd=str(package.directory),
            env=package.environment,
        )
    else:
        executable = package.app_server
        package_root = package.app_server_root
        package_path_dir = package.app_server_path_dir
        config = CodexConfig(
            launch_args_override=(str(executable), *sandbox_config),
            cwd=str(package.directory),
            env=package.environment,
        )
    if windows:
        # TODO(anp): Assert resolved rg paths once Windows sandbox PATH finds them.
        ripgrep = str(package_path_dir / "rg.exe").replace("'", "''")
        command = (
            f"$rg = '{ripgrep}'; Write-Output $rg; & $rg --version; "
            "if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"
        )
    else:
        command = "command -v rg && rg --version"
    exec_arguments = {"cmd": command, "login": False, "yield_time_ms": 10_000}
    responses_server.enqueue_sse(
        sse(
            [
                ev_response_created("code-mode"),
                {
                    "type": "response.output_item.done",
                    "item": {
                        "type": "custom_tool_call",
                        "call_id": "package-smoke",
                        "name": "exec",
                        "input": (
                            "text(JSON.stringify(await tools.exec_command("
                            f"{json.dumps(exec_arguments)})))"
                        ),
                    },
                },
                ev_completed("code-mode"),
            ]
        )
    )
    responses_server.enqueue_assistant_message("Done", response_id="code-mode-done")
    with Codex(config=config) as client:
        turn = client.thread_start(
            ephemeral=True,
            approval_mode=ApprovalMode.deny_all,
            sandbox=Sandbox.workspace_write,
        ).run("run package smoke")
        assert turn.final_response == "Done", turn
    output = next(
        item
        for request in responses_server.requests()
        for item in request.input()
        if item.get("type") == "custom_tool_call_output"
        and item.get("call_id") == "package-smoke"
    )["output"]
    if not isinstance(output, str):
        output = next(
            item["text"] for item in output if item.get("text", "").startswith("{")
        )
    execution = json.loads(output)
    execution_output = execution["output"]
    assert execution["exit_code"] == 0, execution_output
    assert "ripgrep" in execution_output, execution_output

    # Each entrypoint must execute the ripgrep bundled in its own package.
    reported_ripgrep_path = Path(execution_output.splitlines()[0].strip())
    assert reported_ripgrep_path.is_relative_to(package_root), execution_output


def _code_mode_host_symbol_address(
    package: SmokePackage,
    symbols: Path,
    *arguments: str,
) -> str:
    symbol_output = subprocess.run(
        ["nm", *arguments, str(symbols)],
        cwd=package.directory,
        env=package.environment,
        capture_output=True,
        text=True,
        check=True,
        timeout=45,
    ).stdout
    return next(
        parts[0]
        for line in symbol_output.splitlines()
        if len(parts := line.split()) == 3
        and parts[1].lower() == "t"
        and "codex_code_mode_host" in parts[2]
    )


@pytest.mark.skipif(sys.platform != "linux", reason="requires Linux debug symbols")
def test_linux_debug_symbols_resolve_packaged_code(
    package: SmokePackage,
    code_mode_host_debug_symbols: Path,
) -> None:
    """Linux host symbols resolve a packaged function and its source location."""
    address = _code_mode_host_symbol_address(
        package, code_mode_host_debug_symbols, "--defined-only", "--extern-only"
    )
    resolved = subprocess.run(
        [
            "addr2line",
            "-e",
            str(code_mode_host_debug_symbols),
            "-f",
            "-C",
            f"0x{address}",
        ],
        cwd=package.directory,
        env=package.environment,
        capture_output=True,
        text=True,
        check=True,
        timeout=45,
    ).stdout
    assert "codex_code_mode_host" in resolved, resolved
    source = resolved.splitlines()[-1]
    assert source != "??:?" and source.rsplit(":", 1)[-1].isdigit(), source


@pytest.mark.skipif(sys.platform != "darwin", reason="requires macOS debug symbols")
def test_macos_debug_symbols_resolve_packaged_code(
    package: SmokePackage,
    code_mode_host_debug_symbols: Path,
) -> None:
    """macOS host symbols resolve a packaged function."""
    address = _code_mode_host_symbol_address(package, code_mode_host_debug_symbols)
    resolved = subprocess.run(
        ["atos", "-o", str(code_mode_host_debug_symbols), f"0x{address}"],
        cwd=package.directory,
        env=package.environment,
        capture_output=True,
        text=True,
        check=True,
        timeout=45,
    ).stdout
    assert "codex_code_mode_host" in resolved, resolved


@pytest.mark.skipif(sys.platform != "win32", reason="requires Windows debug symbols")
def test_windows_debug_symbols_resolve_packaged_code(
    package: SmokePackage,
    code_mode_host_debug_symbols: Path,
) -> None:
    """Windows host symbols match the packaged executable's debug signature."""
    # Rust embeds an underscored PDB name, but release archives normalize it.
    symbols = code_mode_host_debug_symbols.rename(
        code_mode_host_debug_symbols.with_name("codex_code_mode_host.pdb")
    )
    host = symbols.with_suffix(".exe")
    shutil.copy2(package.cli.with_name("codex-code-mode-host.exe"), host)
    result = subprocess.run(
        ["dumpbin", "/PDBPATH", str(host)],
        cwd=package.directory,
        env=package.environment,
        capture_output=True,
        text=True,
        check=True,
        timeout=45,
    )
    assert str(symbols).casefold() in result.stdout.casefold(), result.stdout
