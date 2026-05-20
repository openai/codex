#!/usr/bin/env python3

from __future__ import annotations

import argparse
import base64
import json
import os
import shutil
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

VALID_STATUSES = {"valid", "present_only", "service_forbidden"}
FAIL_STATUSES = {"invalid", "auth_rejected", "missing_input"}


@dataclass(frozen=True)
class Check:
    check_id: str
    repo: str
    validator: str
    covered_secrets: tuple[str, ...]
    inputs: dict[str, str]
    options: dict[str, Any]
    note: str


@dataclass(frozen=True)
class Result:
    check_id: str
    repo: str
    validator: str
    covered_secrets: tuple[str, ...]
    status: str
    detail: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate workflow secrets without printing secret values or mutating external systems."
    )
    parser.add_argument(
        "--manifest",
        default=str(Path(__file__).with_name("workflow_secret_validation_manifest.json")),
        help="Path to the validation manifest.",
    )
    parser.add_argument(
        "--output-json",
        help="Optional path for structured JSON results.",
    )
    parser.add_argument(
        "--repo",
        action="append",
        default=[],
        metavar="OWNER/REPO",
        help="Run only checks for this repository. May be repeated.",
    )
    parser.add_argument(
        "--only",
        action="append",
        default=[],
        metavar="REPO:SECRET",
        help="Run only checks covering this repo/secret pair. May be repeated.",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Exit non-zero when a check is invalid, rejected, or missing required input.",
    )
    return parser.parse_args()


def load_manifest(path: Path) -> list[Check]:
    payload = json.loads(path.read_text())
    checks = []
    for item in payload["checks"]:
        checks.append(
            Check(
                check_id=item["id"],
                repo=item["repo"],
                validator=item["validator"],
                covered_secrets=tuple(item["covers"]),
                inputs=dict(item.get("inputs", {})),
                options=dict(item.get("options", {})),
                note=item.get("note", ""),
            )
        )
    return checks


def selected(
    check: Check,
    repo_filters: set[str],
    only_filters: set[tuple[str, str]],
) -> bool:
    if repo_filters and check.repo not in repo_filters:
        return False
    if not only_filters:
        return True
    return any((check.repo, secret) in only_filters for secret in check.covered_secrets)


def env_value(env_name: str) -> str | None:
    value = os.environ.get(env_name)
    if value is None or value == "":
        return None
    return value


def require_inputs(
    check: Check, aliases: tuple[str, ...]
) -> tuple[dict[str, str] | None, Result | None]:
    resolved: dict[str, str] = {}
    missing = []
    for alias in aliases:
        env_name = check.inputs[alias]
        value = env_value(env_name)
        if value is None:
            missing.append(env_name)
        else:
            resolved[alias] = value
    if missing:
        return None, result(
            check, "missing_input", f"missing required environment variables: {', '.join(missing)}"
        )
    return resolved, None


def result(check: Check, status: str, detail: str) -> Result:
    return Result(
        check_id=check.check_id,
        repo=check.repo,
        validator=check.validator,
        covered_secrets=check.covered_secrets,
        status=status,
        detail=detail,
    )


def command_exists(binary: str) -> bool:
    return shutil.which(binary) is not None


def run_command(
    args: list[str],
    *,
    input_bytes: bytes | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[bytes]:
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    return subprocess.run(
        args,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=merged_env,
        check=False,
    )


def base64url(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode()


def normalized_multiline(value: str, *, escaped_newlines: bool) -> str:
    if escaped_newlines and "\\n" in value and "\n" not in value:
        return value.replace("\\n", "\n")
    return value


def secret_bytes(value: str, encoding: str) -> bytes:
    if encoding == "raw":
        return value.encode()
    if encoding == "base64":
        return base64.b64decode(value, validate=True)
    if encoding == "base64_or_raw":
        try:
            return base64.b64decode(value, validate=True)
        except Exception:
            return value.encode()
    raise ValueError(f"unsupported encoding: {encoding}")


def http_json(
    url: str,
    *,
    headers: dict[str, str],
    method: str = "GET",
    body: bytes | None = None,
) -> tuple[int, bytes]:
    request = urllib.request.Request(url, data=body, method=method)
    for name, value in headers.items():
        request.add_header(name, value)
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read()


def classify_http_auth(check: Check, status_code: int, success_detail: str) -> Result:
    if 200 <= status_code < 300:
        return result(check, "valid", success_detail)
    if status_code == 401:
        return result(check, "auth_rejected", "service rejected the credential with HTTP 401")
    if status_code == 403:
        return result(
            check, "service_forbidden", "service recognized the request but returned HTTP 403"
        )
    return result(check, "invalid", f"unexpected HTTP status {status_code}")


def validate_presence_only(check: Check) -> Result:
    _, failure = require_inputs(check, tuple(check.inputs))
    if failure:
        return failure
    return result(
        check,
        "present_only",
        check.note or "required inputs are present; no safe live validity probe is defined",
    )


def validate_not_safely_testable(check: Check) -> Result:
    _, failure = require_inputs(check, tuple(check.inputs))
    if failure:
        return failure
    return result(
        check,
        "not_safely_testable",
        check.note or "would require an externally visible or state-changing probe",
    )


def validate_needs_context(check: Check) -> Result:
    _, failure = require_inputs(check, tuple(check.inputs))
    if failure:
        return failure
    return result(
        check,
        "needs_context",
        check.note or "additional companion values or target context are required",
    )


def validate_openai_api_key(check: Check) -> Result:
    values, failure = require_inputs(check, ("token",))
    if failure:
        return failure
    status_code, _ = http_json(
        "https://api.openai.com/v1/models",
        headers={"Authorization": f"Bearer {values['token']}"},
    )
    return classify_http_auth(check, status_code, "OpenAI API accepted GET /v1/models")


def validate_buildkite_token(check: Check) -> Result:
    values, failure = require_inputs(check, ("token",))
    if failure:
        return failure
    status_code, _ = http_json(
        "https://api.buildkite.com/v2/user",
        headers={"Authorization": f"Bearer {values['token']}"},
    )
    return classify_http_auth(check, status_code, "Buildkite API accepted GET /v2/user")


def validate_github_token(check: Check) -> Result:
    values, failure = require_inputs(check, ("token",))
    if failure:
        return failure
    status_code, _ = http_json(
        "https://api.github.com/user",
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {values['token']}",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    return classify_http_auth(check, status_code, "GitHub API accepted GET /user")


def validate_contentful_cma_token(check: Check) -> Result:
    values, failure = require_inputs(check, ("token",))
    if failure:
        return failure
    status_code, _ = http_json(
        "https://api.contentful.com/spaces?limit=1",
        headers={"Authorization": f"Bearer {values['token']}"},
    )
    return classify_http_auth(check, status_code, "Contentful CMA accepted GET /spaces?limit=1")


def validate_ssh_private_key(check: Check) -> Result:
    if not command_exists("ssh-keygen"):
        return result(check, "needs_tool", "ssh-keygen is not available")
    values, failure = require_inputs(check, ("private_key",))
    if failure:
        return failure
    with tempfile.TemporaryDirectory(prefix="secret-validator-ssh-") as tmpdir:
        key_path = Path(tmpdir) / "key"
        key_path.write_text(values["private_key"])
        key_path.chmod(0o600)
        completed = run_command(["ssh-keygen", "-y", "-f", str(key_path)])
    if completed.returncode == 0:
        return result(check, "valid", "ssh-keygen parsed the private key")
    return result(check, "invalid", "ssh-keygen could not parse the private key")


def validate_pkcs12(check: Check) -> Result:
    if not command_exists("openssl"):
        return result(check, "needs_tool", "openssl is not available")
    values, failure = require_inputs(check, ("certificate", "password"))
    if failure:
        return failure
    encoding = str(check.options.get("encoding", "base64_or_raw"))
    try:
        certificate_bytes = secret_bytes(values["certificate"], encoding)
    except Exception:
        return result(check, "invalid", f"certificate could not be decoded using {encoding}")
    password_env = check.inputs["password"]
    with tempfile.TemporaryDirectory(prefix="secret-validator-p12-") as tmpdir:
        certificate_path = Path(tmpdir) / "certificate.p12"
        certificate_path.write_bytes(certificate_bytes)
        certificate_path.chmod(0o600)
        completed = run_command(
            [
                "openssl",
                "pkcs12",
                "-in",
                str(certificate_path),
                "-noout",
                "-passin",
                f"env:{password_env}",
            ]
        )
    if completed.returncode == 0:
        return result(
            check, "valid", "openssl parsed the PKCS#12 bundle with the supplied password"
        )
    return result(
        check, "invalid", "openssl could not parse the PKCS#12 bundle with the supplied password"
    )


def validate_pem_private_key(check: Check) -> Result:
    if not command_exists("openssl"):
        return result(check, "needs_tool", "openssl is not available")
    values, failure = require_inputs(check, ("private_key",))
    if failure:
        return failure
    escaped_newlines = bool(check.options.get("escaped_newlines", False))
    private_key = normalized_multiline(values["private_key"], escaped_newlines=escaped_newlines)
    with tempfile.TemporaryDirectory(prefix="secret-validator-pem-") as tmpdir:
        key_path = Path(tmpdir) / "key.pem"
        key_path.write_text(private_key)
        key_path.chmod(0o600)
        completed = run_command(["openssl", "pkey", "-in", str(key_path), "-noout"])
    if completed.returncode == 0:
        return result(check, "valid", "openssl parsed the PEM private key")
    return result(check, "invalid", "openssl could not parse the PEM private key")


def validate_apple_notary_history(check: Check) -> Result:
    if not command_exists("xcrun"):
        return result(check, "needs_tool", "xcrun is not available; run this check on macOS")
    values, failure = require_inputs(check, ("private_key", "key_id", "issuer_id"))
    if failure:
        return failure
    escaped_newlines = bool(check.options.get("escaped_newlines", False))
    private_key = normalized_multiline(values["private_key"], escaped_newlines=escaped_newlines)
    with tempfile.TemporaryDirectory(prefix="secret-validator-notary-") as tmpdir:
        key_path = Path(tmpdir) / "AuthKey.p8"
        key_path.write_text(private_key)
        key_path.chmod(0o600)
        completed = run_command(
            [
                "xcrun",
                "notarytool",
                "history",
                "--key",
                str(key_path),
                "--key-id",
                values["key_id"],
                "--issuer",
                values["issuer_id"],
                "--output-format",
                "json",
            ]
        )
    if completed.returncode == 0:
        return result(
            check, "valid", "Apple notarytool accepted the credentials for read-only history"
        )
    return result(check, "invalid", "Apple notarytool history rejected the credentials")


def validate_github_app_private_key(check: Check) -> Result:
    if not command_exists("openssl"):
        return result(check, "needs_tool", "openssl is not available")
    values, failure = require_inputs(check, ("private_key",))
    if failure:
        return failure
    app_id = str(check.options["app_id"])
    now = int(time.time())
    header = base64url(json.dumps({"alg": "RS256", "typ": "JWT"}, separators=(",", ":")).encode())
    payload = base64url(
        json.dumps(
            {"iat": now - 60, "exp": now + 540, "iss": app_id}, separators=(",", ":")
        ).encode()
    )
    signing_input = f"{header}.{payload}".encode()
    private_key = normalized_multiline(
        values["private_key"],
        escaped_newlines=bool(check.options.get("escaped_newlines", False)),
    )
    with tempfile.TemporaryDirectory(prefix="secret-validator-ghapp-") as tmpdir:
        key_path = Path(tmpdir) / "app.pem"
        key_path.write_text(private_key)
        key_path.chmod(0o600)
        signed = run_command(
            ["openssl", "dgst", "-sha256", "-sign", str(key_path)], input_bytes=signing_input
        )
    if signed.returncode != 0:
        return result(
            check, "invalid", "openssl could not sign a GitHub App JWT with the private key"
        )
    jwt = f"{header}.{payload}.{base64url(signed.stdout)}"
    status_code, _ = http_json(
        "https://api.github.com/app",
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {jwt}",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    return classify_http_auth(check, status_code, f"GitHub accepted app JWT for app id {app_id}")


def validate_android_keystore(check: Check) -> Result:
    for binary in ("jarsigner",):
        if not command_exists(binary):
            return result(check, "needs_tool", f"{binary} is not available")
    values, failure = require_inputs(check, ("keystore", "store_password", "alias", "key_password"))
    if failure:
        return failure
    try:
        keystore_bytes = secret_bytes(
            values["keystore"], str(check.options.get("encoding", "base64"))
        )
    except Exception:
        return result(check, "invalid", "Android keystore could not be base64-decoded")
    with tempfile.TemporaryDirectory(prefix="secret-validator-android-") as tmpdir:
        tmp_path = Path(tmpdir)
        keystore_path = tmp_path / "release.jks"
        unsigned_jar = tmp_path / "probe.jar"
        signed_jar = tmp_path / "probe-signed.jar"
        keystore_path.write_bytes(keystore_bytes)
        keystore_path.chmod(0o600)
        with zipfile.ZipFile(unsigned_jar, "w") as archive:
            archive.writestr("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n\n")
        completed = run_command(
            [
                "jarsigner",
                "-keystore",
                str(keystore_path),
                "-storepass:env",
                check.inputs["store_password"],
                "-keypass:env",
                check.inputs["key_password"],
                "-signedjar",
                str(signed_jar),
                str(unsigned_jar),
                values["alias"],
            ]
        )
    if completed.returncode == 0:
        return result(
            check,
            "valid",
            "jarsigner locally signed a temporary probe JAR with the keystore credentials",
        )
    return result(
        check, "invalid", "jarsigner could not use the keystore, alias, and passwords together"
    )


def validate_macos_provisioning_profile(check: Check) -> Result:
    if not command_exists("security"):
        return result(check, "needs_tool", "security is not available; run this check on macOS")
    values, failure = require_inputs(check, ("profile",))
    if failure:
        return failure
    try:
        profile_bytes = secret_bytes(
            values["profile"], str(check.options.get("encoding", "base64_or_raw"))
        )
    except Exception:
        return result(check, "invalid", "provisioning profile could not be decoded")
    with tempfile.TemporaryDirectory(prefix="secret-validator-profile-") as tmpdir:
        profile_path = Path(tmpdir) / "profile.mobileprovision"
        profile_path.write_bytes(profile_bytes)
        profile_path.chmod(0o600)
        completed = run_command(["security", "cms", "-D", "-i", str(profile_path)])
    if completed.returncode == 0:
        return result(check, "valid", "security cms parsed the provisioning profile")
    return result(check, "invalid", "security cms could not parse the provisioning profile")


VALIDATORS: dict[str, Callable[[Check], Result]] = {
    "presence_only": validate_presence_only,
    "not_safely_testable": validate_not_safely_testable,
    "needs_context": validate_needs_context,
    "openai_api_key": validate_openai_api_key,
    "buildkite_token": validate_buildkite_token,
    "github_token": validate_github_token,
    "contentful_cma_token": validate_contentful_cma_token,
    "ssh_private_key_parse": validate_ssh_private_key,
    "pkcs12_parse": validate_pkcs12,
    "pem_private_key_parse": validate_pem_private_key,
    "apple_notary_history": validate_apple_notary_history,
    "github_app_private_key": validate_github_app_private_key,
    "android_keystore_local_sign": validate_android_keystore,
    "macos_provisioning_profile_parse": validate_macos_provisioning_profile,
}


def print_results(results: list[Result]) -> None:
    print("| Status | Repo | Check | Covered secrets | Detail |")
    print("|---|---|---|---|---|")
    for item in results:
        covered = ", ".join(item.covered_secrets)
        detail = item.detail.replace("|", "/")
        print(f"| {item.status} | {item.repo} | {item.check_id} | {covered} | {detail} |")


def write_json(path: Path, results: list[Result]) -> None:
    payload = [
        {
            "check_id": item.check_id,
            "repo": item.repo,
            "validator": item.validator,
            "covered_secrets": list(item.covered_secrets),
            "status": item.status,
            "detail": item.detail,
        }
        for item in results
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n")


def main() -> int:
    args = parse_args()
    only_filters = set()
    for raw in args.only:
        if ":" not in raw:
            raise SystemExit(f"--only must use REPO:SECRET, got {raw!r}")
        repo, secret = raw.rsplit(":", 1)
        only_filters.add((repo, secret))

    repo_filters = set(args.repo)
    checks = [
        check
        for check in load_manifest(Path(args.manifest))
        if selected(check, repo_filters, only_filters)
    ]
    results = [VALIDATORS[check.validator](check) for check in checks]
    print_results(results)
    if args.output_json:
        write_json(Path(args.output_json), results)

    if args.strict and any(item.status in FAIL_STATUSES for item in results):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
