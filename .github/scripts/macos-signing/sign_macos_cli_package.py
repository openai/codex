"""Sign and verify macOS CLI packages using the shared AKV and native tools."""

import argparse
import os
import subprocess
import sys
from pathlib import Path

import provisioned_macos_cli_package as bundle


def main(configuration: bundle.ProfileConfiguration | None = None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("sign", "verify"))
    parser.add_argument("package", type=Path)
    args, configuration = bundle.parse_profile_args(parser, configuration)
    package = args.package.resolve(strict=True)
    if not package.is_dir():
        parser.error("package must be an extracted CLI package directory")
    repo_root = os.environ.get("CODEX_REPO_ROOT")
    if not repo_root:
        parser.error("CODEX_REPO_ROOT must be set")
    reports = Path(repo_root) / "signing-verification"
    reports.mkdir(parents=True, exist_ok=True)
    provisioned = os.environ.get("PROVISIONED_MACOS", "false") == "true"
    if provisioned:
        if args.operation == "sign":
            configuration.validate_signing_certificate(
                Path(os.environ["OAI_AKV_SIGNING_CERTIFICATE_PEM"])
            )
            bundle.prepare(package, reports, configuration)
        else:
            bundle.verify(package, reports, os.environ["TARGET"], configuration)

    for relative in ("bin/codex", *bundle.HELPERS):
        binary = package / relative
        name = binary.name
        target = binary
        identifier = name
        entitlements = bundle.SIGNING / f"{name}.entitlements.plist"
        if provisioned and relative == "bin/codex":
            binary = package / bundle.EXECUTABLE
            target = package / bundle.APP
            identifier = bundle.CODE_SIGNING_ID
            entitlements = reports / "codex-provisioned-entitlements.plist"
        if args.operation == "sign":
            command = [
                "bash",
                str(bundle.SIGNING / "sign_macos_code.sh"),
                "--target",
                str(target),
                "--identity",
                "unused",
                "--deep",
                "false",
                "--options",
                "runtime",
                "--timestamp",
                "true",
            ]
            if relative.startswith("bin/"):
                command.extend(
                    ["--identifier", identifier, "--entitlements", str(entitlements)]
                )
            else:
                command.extend(["--identifier", f"com.openai.codex.{name}"])
            subprocess.run(command, check=True)
            with (reports / f"{name}-signature.yaml").open("wb") as output:
                subprocess.run(
                    ["rcodesign", "print-signature-info", str(binary)],
                    stdout=output,
                    check=True,
                )
            if not provisioned:
                subprocess.run(
                    [
                        "bash",
                        str(bundle.SIGNING / "notarize_macos_binary_with_akv.sh"),
                        "--binary",
                        str(binary),
                        "--report-dir",
                        str(reports / name),
                    ],
                    check=True,
                )
        else:
            target_triple = os.environ["TARGET"]
            architectures = {
                "aarch64-apple-darwin": "arm64",
                "x86_64-apple-darwin": "x86_64",
            }
            if target_triple not in architectures:
                raise ValueError(f"Unexpected macOS target: {target_triple}")
            subprocess.run(
                ["lipo", str(binary), "-verify_arch", architectures[target_triple]],
                check=True,
            )
            # Preserve the CLI's existing login-keychain identity while checking
            # its bundle/App ID and provisioning independently in bundle.verify.
            requirement = (
                "=anchor apple generic"
                " and certificate 1[field.1.2.840.113635.100.6.2.6] exists"
                " and certificate leaf[field.1.2.840.113635.100.6.1.13] exists"
            )
            if provisioned and relative == "bin/codex":
                requirement += (
                    f' and identifier "{bundle.CODE_SIGNING_ID}"'
                    f' and certificate leaf[subject.OU] = "{configuration.team_id}"'
                )
            subprocess.run(
                [
                    "codesign",
                    "--verify",
                    "--strict",
                    "--verbose=2",
                    "--test-requirement",
                    requirement,
                    str(target),
                ],
                check=True,
            )
            signature = reports / f"{name}-signature.txt"
            with signature.open("wb") as output:
                subprocess.run(
                    ["codesign", "-d", "--verbose=4", str(target)],
                    stderr=output,
                    check=True,
                )
            actual = reports / f"{name}-entitlements.plist"
            with actual.open("wb") as output:
                subprocess.run(
                    ["codesign", "-d", "--entitlements", ":-", str(target)],
                    stdout=output,
                    check=True,
                )
            if relative.startswith("bin/"):
                expected = reports / f"{name}-expected.plist"
                subprocess.run(
                    [
                        "plutil",
                        "-convert",
                        "xml1",
                        "-o",
                        str(expected),
                        str(entitlements),
                    ],
                    check=True,
                )
                subprocess.run(["plutil", "-convert", "xml1", str(actual)], check=True)
                subprocess.run(["diff", "-u", str(expected), str(actual)], check=True)
            elif actual.stat().st_size:
                raise ValueError(
                    f"Bundled helper {name} must not have signing entitlements"
                )

    if provisioned and args.operation == "sign":
        archive = Path(os.environ["RUNNER_TEMP"]) / "provisioned-cli.zip"
        subprocess.run(["zip", "-q", "-r", str(archive), "."], cwd=package, check=True)
        command = [
            sys.executable,
            str(bundle.SIGNING / "notarize_with_akv.py"),
            "--file",
            str(archive),
            "--report-log",
            str(reports / "notarization-developer-log.json"),
            "--max-wait-seconds",
            "1200",
        ]
        with (
            (reports / "notarization.log").open("w") as output,
            subprocess.Popen(
                command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
            ) as process,
        ):
            assert process.stdout is not None
            for line in process.stdout:
                print(line, end="", flush=True)
                output.write(line)
            if returncode := process.wait():
                raise subprocess.CalledProcessError(returncode, command)


if __name__ == "__main__":
    main()
