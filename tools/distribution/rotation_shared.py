from __future__ import annotations

import argparse
import base64
import csv
import io
import json
import os
import shlex
import subprocess
import sys
import tempfile
from datetime import datetime
from pathlib import Path
from typing import Any, Sequence
from zoneinfo import ZoneInfo

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_OUT_DIR = REPO_ROOT / "tools" / "distribution" / "secrets"
DEFAULT_API_KEY_METADATA_JSON_FILENAME = "app_store_connect_api_key.import.json"
DEFAULT_ONE_PASSWORD_ACCOUNT = "openai-team.1password.com"
DEFAULT_ONE_PASSWORD_PASSWORD_FIELD = "password"
DEFAULT_ONE_PASSWORD_API_KEY_TITLE_BASENAME = "Codex CLI Notarization API Key"
DEFAULT_GITHUB_REPO = "openai/codex"
DEFAULT_ISSUER_ID_SECRET_NAME = "APPLE_NOTARIZATION_ISSUER_ID"
DEFAULT_KEY_ID_SECRET_NAME = "APPLE_NOTARIZATION_KEY_ID"
DEFAULT_PRIVATE_KEY_SECRET_NAME = "APPLE_NOTARIZATION_KEY_P8"
NEW_MAC_SIGNING_CERTIFICATE_SECRET_NAME = "NEW_APPLE_CERTIFICATE_P12"
NEW_MAC_SIGNING_CERTIFICATE_PASSWORD_SECRET_NAME = "NEW_APPLE_CERTIFICATE_PASSWORD"
PACIFIC_TZ = ZoneInfo("America/Los_Angeles")


class CommandRunner:
    def __init__(
        self,
        *,
        dry_run: bool,
        env_overrides: dict[str, str] | None = None,
        verbose: bool = False,
    ) -> None:
        self.dry_run = dry_run
        self.verbose = verbose
        self.env = os.environ.copy()
        if env_overrides:
            self.env.update({key: value for key, value in env_overrides.items() if value})

    def run(
        self,
        argv: Sequence[str],
        *,
        stdin_text: str | None = None,
        capture_json: bool = False,
        cwd: Path | None = None,
        redacted_argv: Sequence[str] | None = None,
    ) -> Any:
        pretty = " ".join(shlex.quote(part) for part in (redacted_argv or argv))
        prefix = "[dry-run]" if self.dry_run else "[exec]"
        if self.verbose:
            print(f"{prefix} {pretty}")
        if stdin_text is not None and self.verbose:
            print(f"{prefix} stdin: <{len(stdin_text)} bytes>")
        if self.dry_run:
            if capture_json:
                return {}
            return None

        try:
            completed = subprocess.run(
                list(argv),
                cwd=str(cwd or REPO_ROOT),
                env=self.env,
                input=stdin_text,
                text=True,
                capture_output=capture_json,
                check=True,
            )
        except subprocess.CalledProcessError as error:
            print(
                f"[error] Command failed with exit code {error.returncode}: {pretty}",
                file=sys.stderr,
            )
            if getattr(error, "stdout", None):
                print("[error] stdout:", file=sys.stderr)
                print(error.stdout, file=sys.stderr)
            if getattr(error, "stderr", None):
                print("[error] stderr:", file=sys.stderr)
                print(error.stderr, file=sys.stderr)
            raise
        if not capture_json:
            return None
        if not completed.stdout.strip():
            return {}
        return json.loads(completed.stdout)


def ensure_parent(path: Path, *, dry_run: bool, verbose: bool = False) -> None:
    if dry_run:
        if verbose:
            print(f"[dry-run] Would create directory: {path}")
        return
    path.mkdir(parents=True, exist_ok=True)


def repo_path(value: str) -> Path:
    path = Path(value).expanduser()
    if path.is_absolute():
        return path
    return REPO_ROOT / path


def default_one_password_notarization_api_key_title() -> str:
    return f"{DEFAULT_ONE_PASSWORD_API_KEY_TITLE_BASENAME} {datetime.now(PACIFIC_TZ).date().isoformat()}"


def normalize_base64_secret_body(body: str) -> str:
    # GitHub secrets and local .base64 files should be stored as a single line.
    return "".join(body.split())


def normalize_private_key_secret_body(body: str) -> str:
    if ("BEGIN " + "PRIVATE KEY") in body:
        return normalize_base64_secret_body(base64.b64encode(body.encode("utf-8")).decode("ascii"))
    return normalize_base64_secret_body(body)


def _escape_one_password_assignment_name(value: str) -> str:
    return value.replace("\\", "\\\\").replace(".", "\\.").replace("=", "\\=")


def _parse_one_password_fields(raw: str, fields: Sequence[str]) -> dict[str, str]:
    rows = list(csv.reader(io.StringIO(raw)))
    if not rows or not rows[0]:
        raise SystemExit("Failed to parse 1Password field output")
    values = rows[0]
    if len(values) < len(fields):
        raise SystemExit(f"Expected {len(fields)} fields from 1Password, got {len(values)}")
    return {field: values[index] for index, field in enumerate(fields)}


def load_one_password_item_fields(
    *,
    item: str,
    fields: Sequence[str],
    vault: str | None,
    account: str,
) -> dict[str, str]:
    argv = [
        "op",
        "item",
        "get",
        item,
        f"--fields={','.join(fields)}",
        "--account",
        account,
        "--reveal",
    ]
    if vault:
        argv.extend(["--vault", vault])
    try:
        raw = subprocess.check_output(argv, text=True)
    except Exception as error:
        raise SystemExit(f"Failed to load 1Password fields from {item!r}: {error}") from error
    return _parse_one_password_fields(raw, fields)


def load_one_password_item_json(
    *,
    item: str,
    vault: str | None,
    account: str,
) -> dict[str, Any]:
    argv = [
        "op",
        "item",
        "get",
        item,
        "--account",
        account,
        "--format",
        "json",
    ]
    if vault:
        argv.extend(["--vault", vault])
    try:
        raw = subprocess.check_output(argv, text=True)
    except Exception as error:
        raise SystemExit(f"Failed to load 1Password item {item!r}: {error}") from error
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as error:
        raise SystemExit(f"Failed to parse 1Password item JSON for {item!r}: {error}") from error
    if not isinstance(payload, dict):
        raise SystemExit(f"1Password item JSON for {item!r} was not an object")
    return payload


def _one_password_item_file_name(file_payload: object) -> str | None:
    if not isinstance(file_payload, dict):
        return None
    for key in ("name", "fileName", "title"):
        value = file_payload.get(key)
        if isinstance(value, str) and value.strip():
            return Path(value).name
    return None


def one_password_item_file_names(
    *,
    item: str,
    vault: str | None,
    account: str,
) -> list[str]:
    payload = load_one_password_item_json(item=item, vault=vault, account=account)
    raw_files = payload.get("files", [])
    if not isinstance(raw_files, list):
        return []
    return [
        file_name
        for file_name in (_one_password_item_file_name(file_payload) for file_payload in raw_files)
        if file_name
    ]


def select_one_password_p12_file_name(file_names: Sequence[str]) -> str:
    p12_file_names = sorted(
        {file_name for file_name in file_names if file_name.lower().endswith(".p12")}
    )
    if not p12_file_names:
        raise SystemExit("The 1Password item does not have an attached .p12 file")
    if len(p12_file_names) > 1:
        raise SystemExit(
            "The 1Password item has multiple .p12 files; pass --p12-file-name with one of: "
            + ", ".join(p12_file_names)
        )
    return p12_file_names[0]


def one_password_secret_reference(*, vault: str, item: str, field_or_file: str) -> str:
    return f"op://{vault}/{item}/{field_or_file}"


def download_one_password_file(
    *,
    item: str,
    vault: str,
    account: str,
    file_name: str,
    output_path: Path,
) -> None:
    argv = [
        "op",
        "read",
        "--out-file",
        str(output_path),
        "--file-mode",
        "0600",
        one_password_secret_reference(vault=vault, item=item, field_or_file=file_name),
        "--account",
        account,
    ]
    try:
        subprocess.run(argv, check=True, stdout=subprocess.DEVNULL)
    except subprocess.CalledProcessError as error:
        raise SystemExit(
            f"Failed to download 1Password file {file_name!r} from {item!r}: {error}"
        ) from error
    if not output_path.exists():
        raise SystemExit(f"1Password file download did not produce {output_path}")


def _one_password_item_exists(
    *,
    item: str,
    vault: str,
    account: str,
) -> bool:
    argv = [
        "op",
        "item",
        "get",
        item,
        "--vault",
        vault,
        "--account",
        account,
        "--format",
        "json",
    ]
    completed = subprocess.run(
        argv, text=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    return completed.returncode == 0


def _one_password_assignment_for_file(path: Path) -> str:
    return f"{_escape_one_password_assignment_name(path.name)}[file]={path.resolve(strict=False)}"


def _one_password_assignment(name: str, value: str, *, field_type: str = "text") -> str:
    escaped_name = _escape_one_password_assignment_name(name)
    if field_type:
        return f"{escaped_name}[{field_type}]={value}"
    return f"{escaped_name}={value}"


def create_one_password_notarization_api_key_item(
    runner: CommandRunner,
    *,
    vault: str,
    title: str,
    account: str,
    private_key_base64: str,
    issuer_id: str,
    key_id: str,
    role: str | None,
    created_date: str,
    key_path: Path,
) -> None:
    file_assignments = [_one_password_assignment_for_file(key_path)]
    sensitive_assignment = f"password={private_key_base64}"
    field_assignments = [
        _one_password_assignment("issuer_id", issuer_id),
        _one_password_assignment("key_id", key_id),
        _one_password_assignment("role", role or ""),
        _one_password_assignment("created_date", created_date),
    ]
    redacted_field_assignments = list(field_assignments)
    item_exists = (
        True
        if runner.dry_run
        else _one_password_item_exists(item=title, vault=vault, account=account)
    )

    if item_exists:
        runner.run(
            [
                "op",
                "item",
                "edit",
                title,
                "--account",
                account,
                "--vault",
                vault,
                sensitive_assignment,
                *field_assignments,
                *file_assignments,
            ],
            redacted_argv=[
                "op",
                "item",
                "edit",
                title,
                "--account",
                account,
                "--vault",
                vault,
                "password=<private-key-base64>",
                *redacted_field_assignments,
                *file_assignments,
            ],
        )
        return

    runner.run(
        [
            "op",
            "item",
            "create",
            "--account",
            account,
            "--category",
            "password",
            "--vault",
            vault,
            "--title",
            title,
            sensitive_assignment,
            *field_assignments,
            *file_assignments,
        ],
        redacted_argv=[
            "op",
            "item",
            "create",
            "--account",
            account,
            "--category",
            "password",
            "--vault",
            vault,
            "--title",
            title,
            "password=<private-key-base64>",
            *redacted_field_assignments,
            *file_assignments,
        ],
    )


def cmd_import_api_key(args: argparse.Namespace) -> None:
    out_dir = repo_path(args.out_dir)
    ensure_parent(out_dir, dry_run=not args.execute, verbose=args.log_verbose)
    source_key_path = Path(args.private_key_path).expanduser()
    key_path = out_dir / f"AuthKey_{args.key_id}.p8"
    metadata_path = out_dir / args.metadata_json_filename
    private_key = source_key_path.read_text(encoding="utf-8")
    if not private_key.endswith("\n"):
        private_key = f"{private_key}\n"
    private_key_base64 = normalize_private_key_secret_body(private_key)
    created_date = datetime.now(PACIFIC_TZ).date().isoformat()
    runner = CommandRunner(dry_run=not args.execute, verbose=args.log_verbose)

    if args.execute:
        key_path.write_text(private_key, encoding="utf-8")
        os.chmod(key_path, 0o600)
        metadata_payload = {
            "issuer_id": args.issuer_id,
            "key_id": args.key_id,
            "role": args.role,
            "source_private_key_path": str(source_key_path),
        }
        metadata_path.write_text(
            json.dumps(metadata_payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    elif args.log_verbose:
        print(f"[dry-run] Would copy API key contents from {source_key_path} to: {key_path}")
        print(f"[dry-run] Would write API key metadata to: {metadata_path}")

    one_password_title = args.one_password_title
    if args.one_password_vault and not one_password_title:
        one_password_title = default_one_password_notarization_api_key_title()

    if args.one_password_vault and one_password_title:
        create_one_password_notarization_api_key_item(
            runner,
            vault=args.one_password_vault,
            title=one_password_title,
            account=args.one_password_account,
            private_key_base64=private_key_base64,
            issuer_id=args.issuer_id,
            key_id=args.key_id,
            role=args.role,
            created_date=created_date,
            key_path=key_path,
        )

    print(f"Imported App Store Connect API key ID: {args.key_id}")
    print(f"Issuer ID: {args.issuer_id}")
    if args.one_password_vault and one_password_title:
        print(f"Saved 1Password item: {one_password_title}")


def cmd_upload_api_key_secrets(args: argparse.Namespace) -> None:
    runner = CommandRunner(dry_run=not args.execute, verbose=args.log_verbose)
    if not args.execute:
        private_key_body = "<loaded-from-1password>"
        issuer_id = "<loaded-from-1password>"
        key_id = "<loaded-from-1password>"
    else:
        loaded_fields = load_one_password_item_fields(
            item=args.one_password_item,
            fields=[args.private_key_field, args.issuer_id_field, args.key_id_field],
            vault=args.one_password_vault,
            account=args.one_password_account,
        )
        private_key_body = normalize_private_key_secret_body(loaded_fields[args.private_key_field])
        issuer_id = loaded_fields[args.issuer_id_field].strip()
        key_id = loaded_fields[args.key_id_field].strip()
        if not private_key_body or not issuer_id or not key_id:
            raise SystemExit(
                "The 1Password notarization API key item is missing one or more required fields. "
                "Expected a base64 private key in the password field plus issuer_id and key_id custom fields."
            )

    runner.run(
        [
            "gh",
            "secret",
            "set",
            "--repo",
            args.github_repo,
            args.issuer_id_secret_name,
        ],
        stdin_text=issuer_id,
    )
    runner.run(
        [
            "gh",
            "secret",
            "set",
            "--repo",
            args.github_repo,
            args.key_id_secret_name,
        ],
        stdin_text=key_id,
    )
    runner.run(
        [
            "gh",
            "secret",
            "set",
            "--repo",
            args.github_repo,
            args.private_key_secret_name,
        ],
        stdin_text=private_key_body,
    )


def cmd_upload_mac_signing_secrets(args: argparse.Namespace) -> None:
    runner = CommandRunner(dry_run=not args.execute, verbose=args.log_verbose)

    file_name = args.p12_file_name or select_one_password_p12_file_name(
        one_password_item_file_names(
            item=args.one_password_item,
            vault=args.one_password_vault,
            account=args.one_password_account,
        )
    )
    with tempfile.TemporaryDirectory(prefix="codex-cli-mac-signing-p12-") as temp_dir:
        p12_path = Path(temp_dir) / file_name
        download_one_password_file(
            item=args.one_password_item,
            vault=args.one_password_vault,
            account=args.one_password_account,
            file_name=file_name,
            output_path=p12_path,
        )
        p12_body = normalize_base64_secret_body(
            base64.b64encode(p12_path.read_bytes()).decode("ascii")
        )

    runner.run(
        [
            "gh",
            "secret",
            "set",
            "--repo",
            args.github_repo,
            args.certificate_secret_name,
        ],
        stdin_text=p12_body,
    )
    runner.run(
        [
            "gh",
            "secret",
            "set",
            "--repo",
            args.github_repo,
            args.password_secret_name,
            "--body",
            args.certificate_password,
        ],
    )
