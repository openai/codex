from __future__ import annotations

import argparse

from rotation_shared import (
    DEFAULT_GITHUB_REPO,
    DEFAULT_ONE_PASSWORD_ACCOUNT,
    NEW_MAC_SIGNING_CERTIFICATE_PASSWORD_SECRET_NAME,
    NEW_MAC_SIGNING_CERTIFICATE_SECRET_NAME,
    cmd_upload_mac_signing_secrets,
)


def register(subparsers: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = subparsers.add_parser(
        "upload-mac-signing-secrets",
        help="Upload a Developer ID Application p12 from 1Password to temporary GitHub Actions secrets",
    )
    parser.add_argument("--github-repo", default=DEFAULT_GITHUB_REPO)
    parser.add_argument(
        "--one-password-item",
        required=True,
        help="1Password item ID or title that has one attached .p12 file",
    )
    parser.add_argument(
        "--one-password-vault",
        required=True,
        help="1Password vault containing the mac signing item",
    )
    parser.add_argument(
        "--one-password-account",
        default=DEFAULT_ONE_PASSWORD_ACCOUNT,
        help="1Password account to use when loading the item",
    )
    parser.add_argument(
        "--p12-file-name",
        help="Attached .p12 filename to download; required only if the item has multiple .p12 files",
    )
    parser.add_argument("--certificate-secret-name", default=NEW_MAC_SIGNING_CERTIFICATE_SECRET_NAME)
    parser.add_argument(
        "--password-secret-name", default=NEW_MAC_SIGNING_CERTIFICATE_PASSWORD_SECRET_NAME
    )
    parser.add_argument(
        "--certificate-password",
        default="",
        help="Password for the p12. Defaults to the empty password used by the temporary certificate.",
    )
    parser.set_defaults(func=cmd_upload_mac_signing_secrets)
