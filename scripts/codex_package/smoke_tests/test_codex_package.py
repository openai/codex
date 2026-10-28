"""Exercise each assembled package in the order its shipped files appear.

ROOT_OF_EXTRACTED_PACKAGE
├── bin
│   ├── codex[.exe]                       # CLI package only
│   ├── codex-app-server[.exe]            # app-server package only
│   ├── codex-code-mode-host[.exe]
│   └── codex-exec-server[.exe]
├── codex-package.json
├── codex-path
│   └── rg[.exe]
└── codex-resources
    ├── bwrap                             # Linux only
    ├── codex-command-runner.exe          # Windows only
    ├── codex-windows-sandbox-setup.exe   # Windows only

Debug symbols for all shipped binaries arrive in a separate companion archive.
Each package contains one entrypoint, not both codex and codex-app-server.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tomllib
from concurrent.futures import ThreadPoolExecutor
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
        pytest.param(("features", "list"), "code_mode", id="features"),
        pytest.param(("completion", "bash"), "codex", id="completion"),
    ],
)
def test_cli_public_commands(
    package: SmokePackage,
    arguments: tuple[str, ...],
    expected: str,
) -> None:
    """Packaged CLI remains usable for discovery, features, and completions."""
    output = package.run(*arguments).stdout
    assert expected in output


def test_cli_version_matches_release_sources(package: SmokePackage) -> None:
    """Verify the compiled banner against the sources used to package the release."""
    root = next(
        parent
        for parent in Path(__file__).resolve().parents
        if (parent / "codex-rs/Cargo.toml").is_file()
    )
    manifest = tomllib.loads((root / "codex-rs/Cargo.toml").read_text())
    version = manifest["workspace"]["package"]["version"]
    assert package.run("--version").stdout == f"codex-cli {version}\n"


@pytest.mark.parametrize("variant", ["cli_root", "app_server_root"])
def test_packaged_executor_initializes_and_enforces_sandbox(
    package: SmokePackage, variant: str
) -> None:
    """Each package re-enters its executor helpers and enforces filesystem/process policy."""
    root = getattr(package, variant)
    windows = "windows" in package.target
    suffix = ".exe" if windows else ""
    executor = root / "bin" / f"codex-exec-server{suffix}"
    directory = package.directory / f"{variant}-executor-sandbox"
    directory.mkdir()
    workspace = directory / "workspace"
    workspace.mkdir()
    home = directory / "home"
    home.mkdir()
    blocked = directory / "blocked.txt"
    blocked.write_text("unchanged")
    environment = {**package.environment, "CODEX_HOME": str(home)}
    if windows:
        # Elevated mode must discover the setup/runner in this package's resources.
        # Exclude build outputs and host Codex installations from helper fallback.
        environment["PATH"] = str(Path(os.environ["SystemRoot"]) / "System32")
    sandbox = {
        "permissions": {
            "type": "managed",
            "file_system": {
                "type": "restricted",
                "entries": [
                    {
                        "path": {"type": "special", "value": {"kind": "root"}},
                        "access": "read",
                    },
                    {
                        "path": {"type": "path", "path": workspace.as_uri()},
                        "access": "write",
                    },
                ],
            },
            "network": "restricted",
        },
        "cwd": workspace.as_uri(),
        "windowsSandboxLevel": "elevated" if windows else "disabled",
    }
    version = (
        subprocess.check_output(
            [str(executor), "--version"], env=package.environment, text=True, timeout=45
        )
        .strip()
        .split()[-1]
    )
    assert version == package.run("--version").stdout.strip().split()[-1]
    with (
        ThreadPoolExecutor(max_workers=1) as reader,
        subprocess.Popen(
            [str(executor), "--listen", "stdio"],
            cwd=package.directory,
            env=environment,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        ) as process,
    ):
        assert process.stdin is not None and process.stdout is not None
        try:
            process.stdin.write(
                json.dumps(
                    {
                        "id": 1,
                        "method": "initialize",
                        "params": {"clientName": "package-smoke"},
                    }
                )
                + "\n"
            )
            process.stdin.flush()
            initialized = json.loads(
                reader.submit(process.stdout.readline).result(timeout=45)
            )
            assert initialized["id"] == 1
            assert initialized["result"]["sessionId"]
            reported_environment = initialized["result"]["environmentInfo"]
            metadata = json.loads((root / "codex-package.json").read_text())
            assert reported_environment["executorVersion"] == metadata["version"]
            process.stdin.write('{"method":"initialized","params":{}}\n')

            # Restricted writes always launch the filesystem helper, even when allowed.
            for request_id, path in [(2, workspace / "allowed.txt"), (3, blocked)]:
                process.stdin.write(
                    json.dumps(
                        {
                            "id": request_id,
                            "method": "fs/writeFile",
                            "params": {
                                "path": path.as_uri(),
                                "dataBase64": "b2s=",
                                "sandbox": sandbox,
                            },
                        }
                    )
                    + "\n"
                )
                process.stdin.flush()
                # The first Windows helper can provision sandbox accounts before replying.
                response = json.loads(
                    reader.submit(process.stdout.readline).result(
                        timeout=180 if windows and request_id == 2 else 45
                    )
                )
                if request_id == 2:
                    assert response == {"id": 2, "result": {}}, response
                    assert path.read_text() == "ok"
                else:
                    assert response["id"] == 3 and "error" in response, response
                    assert blocked.read_text() == "unchanged"

            command = (
                [
                    str(Path(os.environ["SystemRoot"]) / "System32" / "cmd.exe"),
                    "/D",
                    "/S",
                    "/C",
                    r"echo process>process.txt & echo forbidden>..\blocked.txt & exit /b 0",
                ]
                if windows
                else [
                    "/bin/sh",
                    "-c",
                    "printf process >process.txt; printf forbidden >../blocked.txt; exit 0",
                ]
            )
            process.stdin.write(
                json.dumps(
                    {
                        "id": 4,
                        "method": "process/start",
                        "params": {
                            "processId": "package-sandbox",
                            "argv": command,
                            "cwd": workspace.as_uri(),
                            "env": {
                                key: value
                                for key, value in environment.items()
                                if key.casefold() in {"path", "systemroot"}
                            },
                            "sandbox": sandbox,
                            "tty": False,
                            "pipeStdin": False,
                            "arg0": None,
                        },
                    }
                )
                + "\n"
            )
            process.stdin.flush()
            started = False
            exit_code = None
            notifications = []
            while not started or exit_code is None:
                response = json.loads(
                    reader.submit(process.stdout.readline).result(timeout=45)
                )
                notifications.append(response)
                if response.get("id") == 4:
                    expected_type = (
                        "windowsRestrictedToken"
                        if windows
                        else "macosSeatbelt"
                        if "apple-darwin" in package.target
                        else "linuxSeccomp"
                    )
                    assert response["result"]["sandboxType"] == expected_type, response
                    started = True
                if response.get("method") == "process/exited":
                    exit_code = response["params"]["exitCode"]
            assert exit_code == 0, notifications
            assert (workspace / "process.txt").is_file(), notifications
            assert (workspace / "process.txt").read_text().strip() == "process"
            assert blocked.read_text() == "unchanged"
            process.stdin.close()
            assert process.wait(timeout=45) == 0
        finally:
            if process.poll() is None:
                process.kill()


def test_packaged_executor_matches_subcommand_startup(
    package: SmokePackage,
    tmp_path: Path,
) -> None:
    """Both binaries reject the same invalid executor arguments and config."""
    home = tmp_path / "home"
    temporary = tmp_path / "tmp"
    home.mkdir()
    temporary.mkdir()
    (home / "config.toml").write_text("unknown_executor_option = true\n")
    # Release builds require the helper directory to live outside their temp root.
    environment = {
        **package.environment,
        "CODEX_HOME": str(home),
        "TMPDIR": str(temporary),
        "TEMP": str(temporary),
        "TMP": str(temporary),
    }
    suffix = ".exe" if "windows" in package.target else ""
    executor = package.cli_root / "bin" / f"codex-exec-server{suffix}"
    for arguments in [
        ["--strict-config", "--listen", "stdio"],
        ["-c", "unknown_executor_option=true", "--strict-config", "--listen", "stdio"],
        ["--listen", "http://127.0.0.1:0"],
        ["--concurrent-requests", "0"],
        [
            "--aws-sigv4",
            "--remote",
            "https://example.openai.com",
            "--environment-id",
            "env-1",
        ],
        ["--remote-transport", "direct"],
        ["forward", "--connect", "ws://127.0.0.1:0"],
        [
            "-c",
            "unknown_executor_option=true",
            "forward",
            "-c",
            "analytics.enabled=false",
            "--strict-config",
            "--connect",
            "ws://127.0.0.1:0",
            "--remote",
            "https://example.openai.com",
            "--environment-id",
            "env-1",
        ],
    ]:
        outputs = []
        for command in ([str(package.cli), "exec-server"], [str(executor)]):
            result = subprocess.run(
                [*command, *arguments],
                cwd=tmp_path,
                env=environment,
                capture_output=True,
                text=True,
                timeout=45,
            )
            assert result.returncode != 0, arguments
            outputs.append(
                (
                    result.returncode,
                    result.stdout,
                    result.stderr.replace("codex exec-server", "codex-exec-server"),
                )
            )
        assert outputs[0] == outputs[1], arguments


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
    # The mock also records analytics POSTs, which have no Responses input list.
    output = next(
        item
        for request in responses_server.requests()
        if request.path == "/v1/responses"
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
        ["nm", "-C", *arguments, str(symbols)],
        cwd=package.directory,
        env=package.environment,
        capture_output=True,
        text=True,
        check=True,
        timeout=45,
    ).stdout
    # Rust v0 symbols can name this crate only as the instantiating crate of
    # a dependency's generic function. Match the demangled entrypoint instead.
    return next(
        parts[0]
        for line in symbol_output.splitlines()
        if len(parts := line.split(maxsplit=2)) == 3
        and parts[1].lower() == "t"
        and re.fullmatch(r"codex_code_mode_host::main(?:::h[0-9a-f]{16})?", parts[2])
    )


@pytest.mark.skipif(sys.platform != "linux", reason="requires Linux debug symbols")
def test_linux_debug_symbols_resolve_packaged_code(
    package: SmokePackage,
    code_mode_host_debug_symbols: Path,
) -> None:
    """Linux host symbols resolve a packaged function and its source location."""
    address = _code_mode_host_symbol_address(
        package, code_mode_host_debug_symbols, "--defined-only"
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
    # Cargo embeds an underscored PDB name; Bazel uses the binary's name.
    # Offer the archived PDB under both names so dumpbin still verifies that
    # its signature matches the packaged executable, regardless of builder.
    cargo_symbols = code_mode_host_debug_symbols.with_name("codex_code_mode_host.pdb")
    shutil.copy2(code_mode_host_debug_symbols, cargo_symbols)
    symbols = (code_mode_host_debug_symbols, cargo_symbols)
    host = code_mode_host_debug_symbols.with_suffix(".exe")
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
    assert any(str(path).casefold() in result.stdout.casefold() for path in symbols), (
        result.stdout
    )
