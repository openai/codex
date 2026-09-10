"""Adapt declared Windows tool paths to the existing native build/runtime recipes."""

import hashlib
import json
import os
import re
import shutil
import sys
import tarfile
import tempfile
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_native import NativeBuild
from windows_build_inputs import build_environment


def selected_inputs(config, root):
    inputs = dict(config["inputs"])
    tools = {name: (root / path).absolute() for name, path in inputs["tools"].items()}
    manifest = (root / config["manifest"]).absolute()
    declared = {(root / path).absolute() for path in config["installed_files"]}
    if manifest.stat().st_size > 65536:
        raise ValueError("installed Windows tool metadata exceeds limits")
    installed = json.loads(manifest.read_text(encoding="utf-8"))
    expected = {"shell", "make", "cygpath", "automake", "pkg_config"}
    if (
        installed.get("schemaVersion") != 1
        or installed.get("target") != inputs["target"]
        or installed.get("cygwinArchitecture") != "x86_64"
        or set(installed.get("tools", {})) != expected
    ):
        raise ValueError("installed Windows tools do not match the selected target")
    for name in expected:
        relative = Path(installed["tools"][name])
        source = (manifest.parent / relative).absolute()
        if (
            relative.is_absolute()
            or ".." in relative.parts
            or source not in declared
            or (name == "pkg_config" and tools.get(name) != source)
            or not source.resolve(strict=True).is_relative_to(
                manifest.parent.resolve(strict=True)
            )
        ):
            raise ValueError(f"installed Windows tool selection differs: {name}")
        tools[name] = source
    inputs["tools"] = {name: str(path) for name, path in tools.items()}
    for name in ("INCLUDE", "LIB"):
        inputs[name] = [str((root / path).absolute()) for path in inputs[name]]
    return inputs


def main():
    operation, config_path = sys.argv[1:]
    config = json.loads(Path(config_path).read_text(encoding="utf-8"))
    root = Path.cwd()
    with tempfile.TemporaryDirectory(prefix="voice-windows-") as temporary:
        temporary = Path(temporary)
        document = temporary / "windows-inputs.json"
        inputs = selected_inputs(config, root)
        document.write_text(json.dumps(inputs), encoding="utf-8")
        environment, _ = build_environment(
            document, inputs["target"], {"python": Path(sys.executable)}
        )
        # Keep host identity and scratch directories, never developer tool search paths.
        environment.update(
            {
                name: os.environ[name]
                for name in ("TMP", "TEMP", "PROCESSOR_ARCHITECTURE")
                if name in os.environ
            }
        )
        home = temporary / "home"
        home.mkdir()
        environment.update({"HOME": str(home), "USERPROFILE": str(home)})
        os.environ.clear()
        os.environ.update(environment)
        if operation == "prepare":
            from prepare_built_runtime import prepare_archive

            output = root / config["output"]
            sdk = root / config["sdk"]
            for path in (output, sdk):
                if path.exists():
                    path.rmdir()
            try:
                prepare_archive(
                    root / config["prefix"],
                    root / config["receipt"],
                    root / config["status"],
                    inputs["target"],
                    output,
                    sdk_output=sdk,
                )
            except ValueError as exc:
                if str(exc) != "native build receipt is incomplete or mismatched":
                    raise
                receipt = json.loads((root / config["receipt"]).read_text())
                commits = [
                    line
                    for line in (root / config["status"]).read_text().splitlines()
                    if line.startswith("STABLE_GIT_COMMIT ")
                ]
                steps = receipt.get("steps", [])
                checks = {
                    "commit_count": len(commits) == 1,
                    "commit_format": len(commits) == 1
                    and bool(
                        re.fullmatch(r"STABLE_GIT_COMMIT [0-9a-f]{40}", commits[0])
                    ),
                    "target": receipt.get("target") == inputs["target"],
                    "manifest": receipt.get("manifest_sha256")
                    == hashlib.sha256(
                        Path(__file__).with_name("sources.json").read_bytes()
                    ).hexdigest(),
                    "step_count": 1 <= len(steps) <= 128,
                    "step_exit": all(step.get("exit_code") == 0 for step in steps),
                    "last_step": bool(steps)
                    and steps[-1].get("name") == "gst-plugins-good-install",
                }
                print(
                    "Receipt checks failed: "
                    + ", ".join(name for name, valid in checks.items() if not valid),
                    file=sys.stderr,
                )
                raise
            return
        if operation != "build":
            raise ValueError("unknown Windows native action")
        archives = temporary / "archives"
        archives.mkdir()
        for path in config["archives"]:
            source = root / path
            shutil.copyfile(source, archives / source.name)
        tools = inputs["tools"]
        builder = NativeBuild(
            SimpleNamespace(
                target=inputs["target"],
                deployment_target=None,
                jobs=8,
                output=temporary / "build",
                archives=archives,
                windows_build_inputs=document,
                **{
                    name: Path(tools[name])
                    for name in (
                        "cc",
                        "cxx",
                        "cmake",
                        "make",
                        "pkg_config",
                        "shell",
                        "bootstrap_make",
                    )
                },
            ),
            environment,
        )
        try:
            builder.build()
        except Exception:
            if builder.record["steps"]:
                log = builder.output / (builder.record["steps"][-1]["name"] + ".log")
                if log.is_file():
                    print(log.read_text(errors="replace"), file=sys.stderr)
            raise
        with tarfile.open(root / config["prefix"], "w") as archive:
            archive.add(builder.prefix, arcname=".")
        shutil.copyfile(builder.output / "built.json", root / config["receipt"])


if __name__ == "__main__":
    main()
