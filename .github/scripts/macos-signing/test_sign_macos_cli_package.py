"""Exercise signing orchestration with generated credentials and stubbed native tools."""

import itertools
import json
import os
import plistlib
import shutil
import ssl
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

import provisioned_macos_cli_package as bundle
from test_provisioned_macos_cli_package import ProvisioningTestCase

NATIVE_TOOL = """#!/usr/bin/env python3
import json, os, plistlib, sys
from pathlib import Path
name = Path(sys.argv[0]).name
args = sys.argv[1:]
with open(os.environ["CALL_LOG"], "a") as log:
    log.write(json.dumps([name, *args]) + "\\n")
if name == "codesign":
    for arg in args:
        if arg.startswith("--extract-certificates="):
            Path(arg.split("=", 1)[1] + "0").write_bytes(Path(os.environ["MOCK_CERTIFICATE"]).read_bytes())
    if "--test-requirement" in args and os.environ.get("MOCK_WRONG_SIGNING_IDENTITY"):
        print("code failed to satisfy specified code requirement(s)", file=sys.stderr)
        sys.exit(1)
    if "--entitlements" in args:
        target = Path(args[-1]).name
        if target == "CodexCLI.app":
            source = Path(os.environ["CODEX_REPO_ROOT"]) / "signing-verification/codex-provisioned-entitlements.plist"
        elif target in ("codex", "codex-code-mode-host"):
            source = Path(os.environ["MOCK_SIGNING_SCRIPTS"]) / (target + ".entitlements.plist")
        else:
            sys.exit(0)
        if os.environ.get("MOCK_WRONG_ENTITLEMENTS"):
            sys.stdout.buffer.write(plistlib.dumps({"unexpected": True}))
        else:
            sys.stdout.buffer.write(source.read_bytes())
elif name == "plutil":
    source = Path(args[-1])
    target = Path(args[args.index("-o") + 1]) if "-o" in args else source
    target.write_bytes(plistlib.dumps(plistlib.loads(source.read_bytes())))
"""


class MacosSigningTests(ProvisioningTestCase):
    def fixture(self, root, provisioned, target="aarch64-apple-darwin"):
        source = Path(bundle.__file__).parent
        common = Path(os.path.commonpath((source, bundle.SIGNING)))
        helpers = root / source.relative_to(common)
        scripts = root / bundle.SIGNING.relative_to(common)
        helpers.mkdir(parents=True, exist_ok=True)
        scripts.mkdir(parents=True, exist_ok=True)
        for name in ("provisioned_macos_cli_package.py", "sign_macos_cli_package.py"):
            shutil.copyfile(source / name, helpers / name)
        for path in bundle.SIGNING.glob("*.entitlements.plist"):
            shutil.copyfile(path, scripts / path.name)
        (root / "record.py").write_text(
            "import json, os, sys\n"
            "with open(os.environ['CALL_LOG'], 'a') as log:\n"
            "    log.write(json.dumps(sys.argv[1:]) + '\\n')\n"
            "if os.environ.get('FAIL_TOOL') == sys.argv[1]:\n"
            "    print('fixture stdout', flush=True)\n"
            "    print('fixture stderr', file=sys.stderr, flush=True)\n"
            "    sys.exit(17)\n"
        )
        for name in ("sign_macos_code.sh", "notarize_macos_binary_with_akv.sh"):
            (scripts / name).write_text(
                'python3 "$CODEX_REPO_ROOT/record.py" "$(basename "$0")" "$@"\n'
            )
        (scripts / "notarize_with_akv.py").write_text(
            "import os, runpy, sys\n"
            "sys.argv.insert(1, 'notarize_with_akv.py')\n"
            "runpy.run_path(os.environ['CODEX_REPO_ROOT'] + '/record.py')\n"
        )
        for name in ("rcodesign", "codesign", "lipo", "plutil"):
            path = root / name
            path.write_text(NATIVE_TOOL)
            path.chmod(0o755)
        certificate_der = self.profile["DeveloperCertificates"][0]
        certificate = root / "certificate.pem"
        certificate.write_text(ssl.DER_cert_to_PEM_cert(certificate_der))
        (root / "certificate.der").write_bytes(certificate_der)
        package = root / "package with spaces"
        for relative in ("bin/codex", *bundle.HELPERS):
            path = package / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("#!/bin/sh\nexit 0\n")
            path.chmod(0o755)
        (package / "codex-package.json").write_text(
            json.dumps(
                {
                    "variant": "codex",
                    "layoutVersion": 1,
                    "target": target,
                    "entrypoint": "bin/codex",
                }
            )
        )
        env = {
            key: value
            for key, value in os.environ.items()
            if key not in ("GITHUB_WORKSPACE", "CODEX_PROVISIONING_HELPER")
        }
        env.update(
            CODEX_REPO_ROOT=str(root),
            RUNNER_TEMP=str(root),
            TARGET=target,
            PATH=f"{root}{os.pathsep}{os.environ['PATH']}",
            CALL_LOG=str(root / "calls.jsonl"),
            PROVISIONED_MACOS=str(provisioned).lower(),
            OAI_AKV_SIGNING_CERTIFICATE_PEM=str(certificate),
            MOCK_CERTIFICATE=str(root / "certificate.der"),
            MOCK_SIGNING_SCRIPTS=str(scripts),
        )
        return package, helpers / "sign_macos_cli_package.py", env

    def run_driver(self, driver, operation, package, env):
        return subprocess.run(
            [
                sys.executable,
                str(driver),
                operation,
                str(package),
                "--profile",
                str(self.configuration.profile),
                "--profile-sha256",
                self.configuration.profile_sha256,
                "--certificate-sha256",
                self.configuration.certificate_sha256,
                "--team-id",
                self.configuration.team_id,
            ],
            env=env,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )

    def test_standard_and_provisioned_signing_and_verification(self):
        for provisioned, target in itertools.product(
            (False, True), ("aarch64-apple-darwin", "x86_64-apple-darwin")
        ):
            with (
                self.subTest(provisioned=provisioned, target=target),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = Path(directory).resolve()
                package, driver, env = self.fixture(root, provisioned, target)
                result = self.run_driver(driver, "sign", package, env)
                self.assertEqual(result.returncode, 0, result.stderr)
                result = self.run_driver(driver, "verify", package, env)
                self.assertEqual(result.returncode, 0, result.stderr)
                calls = [
                    json.loads(line)
                    for line in (root / "calls.jsonl").read_text().splitlines()
                ]
                signing = [call for call in calls if call[0] == "sign_macos_code.sh"]
                self.assertEqual(
                    [call[call.index("--target") + 1] for call in signing],
                    [
                        str(package / relative)
                        for relative in (
                            bundle.APP if provisioned else "bin/codex",
                            *bundle.HELPERS,
                        )
                    ],
                )
                self.assertEqual(
                    signing[0][signing[0].index("--identifier") + 1], "codex"
                )
                self.assertEqual(
                    signing[0][signing[0].index("--entitlements") + 1],
                    str(
                        root
                        / "signing-verification/codex-provisioned-entitlements.plist"
                    )
                    if provisioned
                    else str(
                        Path(env["MOCK_SIGNING_SCRIPTS"]) / "codex.entitlements.plist"
                    ),
                )
                self.assertEqual(
                    ["--entitlements" in call for call in signing],
                    [True, True, False, False],
                )
                self.assertEqual(
                    [call[0] for call in calls if call[0].startswith("notarize")],
                    ["notarize_with_akv.py"]
                    if provisioned
                    else ["notarize_macos_binary_with_akv.sh"] * 4,
                )
                self.assertEqual(
                    [call[1:] for call in calls if call[0] == "lipo"],
                    [
                        [
                            str(package / relative),
                            "-verify_arch",
                            "arm64" if target.startswith("aarch64") else "x86_64",
                        ]
                        for relative in (
                            bundle.EXECUTABLE if provisioned else "bin/codex",
                            *bundle.HELPERS,
                        )
                    ],
                )
                if provisioned:
                    verification = [
                        call
                        for call in calls
                        if call[0] == "codesign" and "--test-requirement" in call
                    ]
                    cli_verification = verification[0]
                    self.assertEqual(cli_verification[-1], str(package / bundle.APP))
                    requirement = cli_verification[
                        cli_verification.index("--test-requirement") + 1
                    ]
                    self.assertIn(' and identifier "codex"', requirement)
                    self.assertIn(
                        ' and certificate leaf[subject.OU] = "TESTTEAM01"', requirement
                    )
                    with zipfile.ZipFile(root / "provisioned-cli.zip") as archive:
                        info = plistlib.loads(
                            archive.read("CodexCLI.app/Contents/Info.plist")
                        )
                        self.assertEqual(
                            info["CFBundleIdentifier"], "com.openai.codex.cli"
                        )
                        self.assertEqual(info["CFBundleExecutable"], "codex")
                        self.assertEqual(
                            plistlib.loads(
                                (
                                    root
                                    / "signing-verification/codex-provisioned-entitlements.plist"
                                ).read_bytes()
                            ),
                            {
                                **plistlib.loads(
                                    (
                                        Path(env["MOCK_SIGNING_SCRIPTS"])
                                        / "codex.entitlements.plist"
                                    ).read_bytes()
                                ),
                                "com.apple.application-identifier": "TESTTEAM01.com.openai.codex.cli",
                                "com.apple.developer.team-identifier": "TESTTEAM01",
                                "keychain-access-groups": [
                                    "TESTTEAM01.com.openai.codex.cli"
                                ],
                            },
                        )
                        self.assertEqual(
                            archive.read(
                                "CodexCLI.app/Contents/embedded.provisionprofile"
                            ),
                            self.configuration.profile.read_bytes(),
                        )
                        self.assertEqual(
                            archive.read("bin/codex").decode(), bundle.LAUNCHER
                        )
                        self.assertTrue(
                            all(
                                helper in archive.namelist()
                                for helper in bundle.HELPERS
                            )
                        )

    def test_provisioned_verification_rejects_changed_bundle_identity(self):
        for field, value in (
            ("CFBundleIdentifier", "codex"),
            ("CFBundleExecutable", "another-codex"),
            ("CFBundlePackageType", "BNDL"),
        ):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                package, driver, env = self.fixture(root, True)
                signed = self.run_driver(driver, "sign", package, env)
                self.assertEqual(signed.returncode, 0, signed.stderr)
                info_path = package / bundle.APP / "Contents/Info.plist"
                info = plistlib.loads(info_path.read_bytes())
                info[field] = value
                info_path.write_bytes(plistlib.dumps(info))
                result = self.run_driver(driver, "verify", package, env)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(
                    "Unexpected provisioned CLI bundle identity or executable",
                    result.stderr,
                )

    def test_signing_and_notarization_failures_stop_the_driver(self):
        for tool in ("sign_macos_code.sh", "notarize_with_akv.py"):
            with self.subTest(tool=tool), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                package, driver, env = self.fixture(root, True)
                result = self.run_driver(
                    driver, "sign", package, {**env, "FAIL_TOOL": tool}
                )
                self.assertNotEqual(result.returncode, 0)
                calls = [
                    json.loads(line)
                    for line in (root / "calls.jsonl").read_text().splitlines()
                ]
                self.assertEqual(calls[-1][0], tool)
                if tool == "notarize_with_akv.py":
                    self.assertEqual(
                        (root / "signing-verification/notarization.log").read_text(),
                        "fixture stdout\nfixture stderr\n",
                    )
                    self.assertIn("fixture stdout\nfixture stderr\n", result.stdout)
                else:
                    self.assertFalse((root / "provisioned-cli.zip").exists())

    def test_verification_rejects_wrong_signing_identity_and_entitlements(self):
        for provisioned, failure in itertools.product(
            (False, True), ("MOCK_WRONG_SIGNING_IDENTITY", "MOCK_WRONG_ENTITLEMENTS")
        ):
            with (
                self.subTest(provisioned=provisioned, failure=failure),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = Path(directory).resolve()
                package, driver, env = self.fixture(root, provisioned)
                signed = self.run_driver(driver, "sign", package, env)
                self.assertEqual(signed.returncode, 0, signed.stderr)
                result = self.run_driver(
                    driver, "verify", package, {**env, failure: "true"}
                )
                self.assertNotEqual(result.returncode, 0)
                if failure == "MOCK_WRONG_SIGNING_IDENTITY":
                    self.assertIn(
                        "code failed to satisfy specified code requirement(s)",
                        result.stderr,
                    )
                else:
                    self.assertIn("unexpected", result.stdout)

    def test_cli_requires_independent_profile_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(bundle.__file__).with_name("sign_macos_cli_package.py")),
                    "verify",
                    directory,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 2)
            self.assertIn("the following arguments are required", result.stderr)


if __name__ == "__main__":
    unittest.main()
