"""Prepare and verify provisioned CLI bundles using an explicitly approved profile."""

import argparse
import hashlib
import json
import os
import plistlib
import re
import shutil
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

BUNDLE_ID = "com.openai.codex.cli"
# Existing login-keychain ACLs identify the CLI by its code-signing identifier.
# Keep this stable independently of the bundle and provisioned App ID.
CODE_SIGNING_ID = "codex"
APP = Path("CodexCLI.app")
EXECUTABLE = APP / "Contents/MacOS/codex"
SIGNING = Path(__file__).resolve().parent
HELPERS = ("bin/codex-code-mode-host", "codex-path/rg", "codex-resources/zsh/bin/zsh")
LAUNCHER = """#!/bin/sh
set -eu
entry="$0"
while [ -L "$entry" ]; do
  parent=$(CDPATH= cd -P -- "$(dirname -- "$entry")" && pwd)
  entry=$(readlink "$entry")
  case "$entry" in /*) ;; *) entry="$parent/$entry" ;; esac
done
bin_dir=$(CDPATH= cd -P -- "$(dirname -- "$entry")" && pwd)
exec "$bin_dir/../CodexCLI.app/Contents/MacOS/codex" "$@"
"""


@dataclass(frozen=True)
class ProfileConfiguration:
    """Independent release expectations; never infer these from the signed package."""

    profile: Path
    profile_sha256: str
    certificate_sha256: str
    team_id: str

    def __post_init__(self):
        for name in ("profile_sha256", "certificate_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", getattr(self, name)):
                raise ValueError(f"{name} must be a lowercase SHA-256 digest")
        if not re.fullmatch(r"[A-Z0-9]{10}", self.team_id):
            raise ValueError("team_id must be a ten-character Apple Team ID")

    def validate_signing_certificate(self, certificate: Path):
        certificate_der = subprocess.check_output(
            ["openssl", "x509", "-in", str(certificate), "-outform", "DER"]
        )
        if hashlib.sha256(certificate_der).hexdigest() != self.certificate_sha256:
            raise ValueError("AKV signing certificate is not authorized by the profile")


def load_profile(configuration: ProfileConfiguration):
    # Pin the portal-approved CMS as well as checking its signature. -noverify
    # skips OpenSSL's non-Apple trust store, not CMS signature verification.
    if (
        hashlib.sha256(configuration.profile.read_bytes()).hexdigest()
        != configuration.profile_sha256
    ):
        raise ValueError("Provisioning profile changed; review and update its pin")
    profile = plistlib.loads(
        subprocess.check_output(
            [
                "openssl",
                "cms",
                "-verify",
                "-inform",
                "DER",
                "-noverify",
                "-in",
                str(configuration.profile),
            ]
        )
    )
    validate_profile(profile, configuration)
    return profile


def validate_profile(profile, configuration: ProfileConfiguration):
    allowed = profile["Entitlements"]
    if (
        profile["TeamIdentifier"] != [configuration.team_id]
        or profile["ApplicationIdentifierPrefix"] != [configuration.team_id]
        or allowed["com.apple.application-identifier"]
        != f"{configuration.team_id}.{BUNDLE_ID}"
        or allowed["com.apple.developer.team-identifier"] != configuration.team_id
        or allowed["keychain-access-groups"] != [f"{configuration.team_id}.*"]
        or allowed.get("get-task-allow", False)
        or not profile.get("ProvisionsAllDevices")
        or "ProvisionedDevices" in profile
    ):
        raise ValueError("Profile does not authorize the CLI Developer ID identity")
    now = datetime.now(timezone.utc).replace(tzinfo=None)
    if not profile["CreationDate"] <= now < profile["ExpirationDate"]:
        raise ValueError("Provisioning profile is not currently valid")
    if [
        hashlib.sha256(cert).hexdigest() for cert in profile["DeveloperCertificates"]
    ] != [configuration.certificate_sha256]:
        raise ValueError("Profile does not authorize the approved signing certificate")


def entitlements(configuration: ProfileConfiguration):
    base = plistlib.loads((SIGNING / "codex.entitlements.plist").read_bytes())
    return {
        **base,
        "com.apple.application-identifier": f"{configuration.team_id}.{BUNDLE_ID}",
        "com.apple.developer.team-identifier": configuration.team_id,
        "keychain-access-groups": [f"{configuration.team_id}.{BUNDLE_ID}"],
    }


def prepare(package, reports, configuration: ProfileConfiguration):
    load_profile(configuration)
    metadata = json.loads((package / "codex-package.json").read_text())
    if (
        metadata["variant"] != "codex"
        or metadata["layoutVersion"] != 1
        or metadata["target"] not in ("aarch64-apple-darwin", "x86_64-apple-darwin")
        or metadata["entrypoint"] != "bin/codex"
    ):
        raise ValueError("Expected a canonical macOS CLI package")
    for relative in ("bin/codex", *HELPERS):
        path = package / relative
        if path.is_symlink() or not path.is_file() or not os.access(path, os.X_OK):
            raise ValueError(f"Expected a regular executable: {relative}")
    # Refuse to overwrite a previously prepared or signed bundle.
    (package / EXECUTABLE).parent.mkdir(parents=True, exist_ok=False)
    (package / "bin/codex").rename(package / EXECUTABLE)
    (package / "bin/codex").write_text(LAUNCHER)
    (package / "bin/codex").chmod(0o755)
    contents = package / APP / "Contents"
    shutil.copyfile(configuration.profile, contents / "embedded.provisionprofile")
    (contents / "Info.plist").write_bytes(
        plistlib.dumps(
            {
                "CFBundleIdentifier": BUNDLE_ID,
                "CFBundleExecutable": "codex",
                "CFBundleName": "Codex CLI",
                "CFBundlePackageType": "APPL",
                "CFBundleVersion": "1",
            }
        )
    )
    reports.mkdir(parents=True, exist_ok=True)
    # Keep expected entitlements separate from the shared verifier's extracted
    # codex-entitlements.plist, otherwise it would overwrite its own expectation.
    (reports / "codex-provisioned-entitlements.plist").write_bytes(
        plistlib.dumps(entitlements(configuration))
    )


def verify(package, reports, expected_target, configuration: ProfileConfiguration):
    """Check profile and certificate pins; the signing driver verifies code."""
    load_profile(configuration)
    info = plistlib.loads((package / APP / "Contents/Info.plist").read_bytes())
    if (
        info.get("CFBundleIdentifier") != BUNDLE_ID
        or info.get("CFBundleExecutable") != EXECUTABLE.name
        or info.get("CFBundlePackageType") != "APPL"
    ):
        raise ValueError("Unexpected provisioned CLI bundle identity or executable")
    if (
        package / APP / "Contents/embedded.provisionprofile"
    ).read_bytes() != configuration.profile.read_bytes():
        raise ValueError("Embedded profile differs from the reviewed profile")
    if (package / "bin/codex").read_text() != LAUNCHER:
        raise ValueError("Unexpected CLI launcher")
    metadata = json.loads((package / "codex-package.json").read_text())
    if metadata["target"] != expected_target:
        raise ValueError(
            "Package target differs from the requested verification target"
        )
    reports.mkdir(parents=True, exist_ok=True)
    (reports / "codex-provisioned-entitlements.plist").write_bytes(
        plistlib.dumps(entitlements(configuration))
    )
    # sign_macos_cli_package.py owns shared architecture, signature and entitlement
    # checks. Here, require the exact certificate authorized by the profile.
    for relative in (EXECUTABLE, *(Path(helper) for helper in HELPERS)):
        binary = package / relative
        target = package / APP if relative == EXECUTABLE else binary
        prefix = reports / f"{binary.name}-cert-"
        subprocess.run(
            ["codesign", "-d", f"--extract-certificates={prefix}", str(target)],
            check=True,
        )
        if (
            hashlib.sha256(Path(f"{prefix}0").read_bytes()).hexdigest()
            != configuration.certificate_sha256
        ):
            raise ValueError(f"Unexpected signing certificate: {relative}")


def parse_profile_args(parser, configuration: ProfileConfiguration | None):
    """Require independent profile expectations unless the caller supplies them."""
    if configuration is None:
        parser.add_argument("--profile", type=Path, required=True)
        parser.add_argument("--profile-sha256", required=True)
        parser.add_argument("--certificate-sha256", required=True)
        parser.add_argument("--team-id", required=True)
    args = parser.parse_args()
    if configuration is None:
        configuration = ProfileConfiguration(
            args.profile, args.profile_sha256, args.certificate_sha256, args.team_id
        )
    return args, configuration


def main(configuration: ProfileConfiguration | None = None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("prepare", "verify", "validate-profile"))
    parser.add_argument("--package", type=Path, default=Path("package"))
    parser.add_argument("--reports", type=Path, default=Path("provisioned-reports"))
    parser.add_argument("--certificate", type=Path)
    parser.add_argument(
        "--target", choices=("aarch64-apple-darwin", "x86_64-apple-darwin")
    )
    args, configuration = parse_profile_args(parser, configuration)
    if args.certificate:
        configuration.validate_signing_certificate(args.certificate)
    if args.operation == "validate-profile":
        load_profile(configuration)
    elif args.operation == "prepare":
        prepare(args.package, args.reports, configuration)
    else:
        if not args.target:
            parser.error("--target is required for verification")
        verify(args.package, args.reports, args.target, configuration)


if __name__ == "__main__":
    main()
