import gzip
import json
import os
import subprocess
import tarfile
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path
from typing import Self

import pytest
import zstandard
from app_server_harness import MockResponsesServer


@dataclass(frozen=True)
class SmokePackage:
    target: str
    cli: Path
    cli_root: Path
    cli_path_dir: Path
    app_server: Path
    app_server_root: Path
    app_server_path_dir: Path
    directory: Path
    environment: dict[str, str]

    @classmethod
    def from_archives(
        cls,
        directory: Path,
        target: str,
        cli_archive: Path,
        app_server_archive: Path,
        compression: str,
    ) -> Self:
        config_dir = directory / "codex-config"
        config_dir.mkdir()
        environment = dict(os.environ)

        # Preserve host proxies while routing the fake localhost model directly.
        inherited_bypass = (
            environment.get("NO_PROXY") or environment.get("no_proxy") or ""
        ).split(",")
        environment["NO_PROXY"] = ",".join(
            dict.fromkeys(
                host.strip()
                for host in [*inherited_bypass, "127.0.0.1", "localhost"]
                if host.strip()
            )
        )

        # Isolate package configuration and state from the user's Codex setup.
        environment["CODEX_HOME"] = str(config_dir)
        # Shell startup files can replace PATH and hide the packaged ripgrep.
        environment.pop("BASH_ENV", None)
        environment["ZDOTDIR"] = str(directory)

        extracted_packages = []
        for index, archive_path in enumerate((cli_archive, app_server_archive)):
            extracted = directory / f"package-{index}"
            extracted.mkdir()
            decompress = zstandard.open if compression == "zstd" else gzip.open
            with (
                decompress(archive_path, "rb") as source,
                tarfile.open(fileobj=source, mode="r|") as archive,
            ):
                archive.extractall(extracted, filter="data")
            manifest = json.loads((extracted / "codex-package.json").read_text())
            extracted_packages.append(
                (
                    extracted,
                    extracted / manifest["entrypoint"],
                    extracted / manifest["pathDir"],
                )
            )

        cli_root, cli, cli_path_dir = extracted_packages[0]
        app_server_root, app_server, app_server_path_dir = extracted_packages[1]

        (config_dir / "config.toml").write_text(
            """approval_policy = "never"
sandbox_mode = "workspace-write"
suppress_unstable_features_warning = true

[sandbox_workspace_write]
network_access = true

[features]
code_mode_only = true
code_mode_host = true
memories = false
apps = false
plugins = false

[analytics]
enabled = false

[otel]
exporter = "none"
trace_exporter = "none"
metrics_exporter = "none"
"""
        )
        return cls(
            target=target,
            cli=cli,
            cli_root=cli_root,
            cli_path_dir=cli_path_dir,
            app_server=app_server,
            app_server_root=app_server_root,
            app_server_path_dir=app_server_path_dir,
            directory=directory,
            environment=environment,
        )

    def run(
        self,
        *arguments: str,
        stdin: str | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(self.cli), *arguments],
            cwd=self.directory,
            env=self.environment,
            input=stdin,
            check=True,
            text=True,
            capture_output=True,
            timeout=45,
        )


@pytest.fixture(scope="session")
def package(
    request: pytest.FixtureRequest,
    tmp_path_factory: pytest.TempPathFactory,
) -> SmokePackage:
    compression = request.config.getoption("compression")
    target = request.config.getoption("package_target")
    directory = tmp_path_factory.mktemp(f"codex-package-smoke-{compression}")
    if "windows" in target:
        # pytest's 0700 directories have protected ACLs, so restricted-token
        # sandboxes need read/execute access to reach the packaged binaries.
        for path in (directory.parent, directory):
            subprocess.run(
                ["icacls", str(path), "/grant", "*S-1-1-0:(OI)(CI)(RX)"],
                check=True,
                capture_output=True,
                text=True,
            )
    return SmokePackage.from_archives(
        directory,
        target,
        request.config.getoption("cli_archive"),
        request.config.getoption("app_server_archive"),
        compression,
    )


@pytest.fixture
def responses_server(package: SmokePackage) -> Iterator[MockResponsesServer]:
    config_path = Path(package.environment["CODEX_HOME"]) / "config.toml"
    original_config = config_path.read_text()
    with MockResponsesServer() as server:
        # Direct CLI commands and SDK-launched servers must share a provider;
        # CodexConfig only controls how the SDK launches its server process.
        config_path.write_text(
            f"""model = "package-smoke"
model_provider = "package_smoke"

{original_config}

[model_providers.package_smoke]
name = "package smoke"
base_url = "{server.url}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"""
        )
        try:
            yield server
        finally:
            config_path.write_text(original_config)


@pytest.fixture(scope="session")
def code_mode_host_debug_symbols(
    pytestconfig: pytest.Config,
    tmp_path_factory: pytest.TempPathFactory,
) -> Path:
    target = pytestconfig.getoption("package_target")
    binaries = {"codex", "codex-app-server", "codex-code-mode-host"}
    if "windows" in target:
        binaries.update({"codex-command-runner", "codex-windows-sandbox-setup"})

    if "apple-darwin" in target:
        markers = {
            binary: f"/{binary}.dSYM/Contents/Resources/DWARF/" for binary in binaries
        }
    else:
        extension = "pdb" if "windows" in target else "debug"
        markers = {binary: f"/{binary}.{extension}" for binary in binaries}

    destination = tmp_path_factory.mktemp("codex-debug-symbols")
    found: set[str] = set()
    symbol_path = None
    with tarfile.open(pytestconfig.getoption("symbols_archive"), "r|gz") as archive:
        for member in archive:
            if not member.isfile():
                continue
            for binary, marker in markers.items():
                if "apple-darwin" in target:
                    _, found_marker, dwarf_name = member.name.partition(marker)
                    matches = bool(
                        found_marker and dwarf_name and "/" not in dwarf_name
                    )
                else:
                    matches = member.name.endswith(marker)
                if not matches:
                    continue
                found.add(binary)
                if binary == "codex-code-mode-host":
                    archive.extract(member, destination, filter="data")
                    symbol_path = destination / member.name
                break
    assert found == binaries, found
    assert symbol_path is not None
    return symbol_path
