#!/usr/bin/env python3
"""Submit release artifacts for notarization and wait for the result."""

import argparse
import base64
import datetime
import hashlib
import hmac
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any

APPLE_NOTARY_URL = "https://appstoreconnect.apple.com/notary/v2/submissions"
AZURE_KEYVAULT_API_VERSION = "7.4"
AWS_REGION = "us-west-2"
JWT_LIFETIME_SECONDS = 15 * 60
POLL_INTERVAL_SECONDS = 10
MAX_SINGLE_UPLOAD_BYTES = 5 * 1024 * 1024 * 1024


class NotarizationError(RuntimeError):
    """Describe a notarization failure."""


@dataclass(frozen=True)
class NotarizationConfiguration:
    """Hold the account and signing-key configuration."""

    issuer_id: str
    apple_key_id: str
    vault_name: str
    vault_key_name: str
    vault_key_version: str

    @classmethod
    def from_environment(cls) -> "NotarizationConfiguration":
        """Load and validate the notarization configuration."""

        required = {
            "vault_name": "AZURE_KEYVAULT_NAME",
            "vault_key_name": "APPLE_NOTARIZATION_AKV_KEY_NAME",
        }
        values = {field: os.environ.get(name, "").strip() for field, name in required.items()}
        missing = [name for field, name in required.items() if not values[field]]
        if missing:
            raise NotarizationError("Missing notarization configuration: " + ", ".join(missing))
        if not re.fullmatch(r"[A-Za-z][A-Za-z0-9-]{2,23}", values["vault_name"]):
            raise NotarizationError("Invalid signing vault name")
        if not re.fullmatch(r"[A-Za-z0-9-]+", values["vault_key_name"]):
            raise NotarizationError("Invalid notarization key name")
        key_version = os.environ.get("APPLE_NOTARIZATION_AKV_KEY_VERSION", "").strip()
        if key_version and not re.fullmatch(r"[0-9a-fA-F]{32}", key_version):
            raise NotarizationError(
                "Notarization key must resolve to a 32-character pinned version"
            )

        command = [
            "az",
            "keyvault",
            "key",
            "show",
            "--vault-name",
            values["vault_name"],
            "--name",
            values["vault_key_name"],
        ]
        if key_version:
            command.extend(["--version", key_version])
        command.extend(
            [
                "--query",
                '{id:key.kid,apple_key_id:tags."apple-key-id",'
                'apple_issuer_id:tags."apple-issuer-id"}',
                "--output",
                "json",
                "--only-show-errors",
            ]
        )

        # Resolve the key identifier and version from the same lookup.
        try:
            result = subprocess.run(command, check=True, capture_output=True, text=True)
        except (OSError, subprocess.CalledProcessError) as error:
            raise NotarizationError("Notarization signing key could not be read") from error
        try:
            metadata = json.loads(result.stdout)
            returned_key_id = metadata["id"]
            apple_key_id = metadata["apple_key_id"]
            apple_issuer_id = metadata["apple_issuer_id"]
        except (json.JSONDecodeError, KeyError, TypeError) as error:
            raise NotarizationError("Notarization key metadata is invalid") from error

        key_id_prefix = (
            f"https://{values['vault_name']}.vault.azure.net/keys/{values['vault_key_name']}/"
        )
        if not isinstance(returned_key_id, str) or not returned_key_id.startswith(key_id_prefix):
            raise NotarizationError("Unexpected notarization signing key")
        returned_key_version = returned_key_id[len(key_id_prefix) :]
        if not re.fullmatch(r"[0-9a-fA-F]{32}", returned_key_version):
            raise NotarizationError(
                "Notarization key must resolve to a 32-character pinned version"
            )
        if key_version and returned_key_version.lower() != key_version.lower():
            raise NotarizationError("Unexpected notarization key version")
        if not isinstance(apple_key_id, str) or not apple_key_id.strip():
            raise NotarizationError("Notarization key must have an apple-key-id tag")
        if not isinstance(apple_issuer_id, str) or not apple_issuer_id.strip():
            raise NotarizationError("Notarization key must have an apple-issuer-id tag")
        if not re.fullmatch(
            r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-"
            r"[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
            apple_issuer_id.strip(),
        ):
            raise NotarizationError("Notarization apple-issuer-id tag must contain a valid UUID")

        values["issuer_id"] = apple_issuer_id.strip()
        values["apple_key_id"] = apple_key_id.strip()
        values["vault_key_version"] = returned_key_version
        return cls(**values)

    @property
    def versioned_key_id(self) -> str:
        """Return the versioned signing-key identifier."""

        return (
            f"https://{self.vault_name}.vault.azure.net/keys/"
            f"{self.vault_key_name}/{self.vault_key_version}"
        )


def base64url_encode(value: bytes) -> str:
    """Encode bytes using the unpadded base64url format required by JWTs."""

    return base64.urlsafe_b64encode(value).decode("ascii").rstrip("=")


def base64url_decode(value: str) -> bytes:
    """Decode a base64url value and reject malformed input."""

    try:
        return base64.b64decode(value + "=" * (-len(value) % 4), altchars=b"-_", validate=True)
    except (ValueError, UnicodeEncodeError) as error:
        raise NotarizationError("Signing service returned an invalid signature") from error


def create_apple_jwt(configuration: NotarizationConfiguration, *, issued_at: int) -> str:
    """Create an authentication token for notarization requests."""

    header = {"alg": "ES256", "kid": configuration.apple_key_id, "typ": "JWT"}
    # Restrict the token to notarization requests.
    claims = {
        "iss": configuration.issuer_id,
        "iat": issued_at,
        "exp": issued_at + JWT_LIFETIME_SECONDS,
        "aud": "appstoreconnect-v1",
        "scope": ["/notary/v2"],
    }
    encoded_header = base64url_encode(json.dumps(header, separators=(",", ":")).encode("utf-8"))
    encoded_claims = base64url_encode(json.dumps(claims, separators=(",", ":")).encode("utf-8"))
    signing_input = f"{encoded_header}.{encoded_claims}"
    digest = hashlib.sha256(signing_input.encode("ascii")).digest()
    key_url = configuration.versioned_key_id
    request_body = json.dumps({"alg": "ES256", "value": base64url_encode(digest)})

    try:
        result = subprocess.run(
            [
                "az",
                "rest",
                "--method",
                "post",
                "--url",
                f"{key_url}/sign?api-version={AZURE_KEYVAULT_API_VERSION}",
                "--resource",
                "https://vault.azure.net",
                "--headers",
                "Content-Type=application/json",
                "--body",
                request_body,
                "--output",
                "json",
                "--only-show-errors",
            ],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "").strip()
        message = "Signing service could not sign the authentication token"
        if detail:
            message = f"{message}: {detail}"
        raise NotarizationError(message) from error

    try:
        response = json.loads(result.stdout)
        returned_key_id = response["kid"]
        encoded_signature = response["value"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise NotarizationError("Signing service returned an invalid response") from error

    # Reject signatures generated with a different key version.
    if returned_key_id != key_url:
        raise NotarizationError("Signing service returned an unexpected key version")
    if not isinstance(encoded_signature, str):
        raise NotarizationError("Signing service returned an invalid signature")
    signature = base64url_decode(encoded_signature)
    # ES256 JWT signatures are 64 raw R || S bytes, not ASN.1/DER-encoded ECDSA signatures.
    if len(signature) != 64:
        raise NotarizationError("ES256 signature must contain 64 JOSE R || S bytes")
    return f"{signing_input}.{base64url_encode(signature)}"


def json_request(url: str, token: str, *, body: dict[str, Any] | None = None) -> dict[str, Any]:
    """Send an authenticated JSON request to Apple's notarization service."""

    data = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=data,
        method="GET" if body is None else "POST",
        headers={
            "Accept": "application/json",
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            result = json.load(response)
    except urllib.error.HTTPError as error:
        detail = error.read(1024).decode("utf-8", errors="replace")
        raise NotarizationError(f"Apple Notary API returned HTTP {error.code}: {detail}") from error
    except (OSError, json.JSONDecodeError) as error:
        raise NotarizationError("Apple Notary API request failed") from error

    if not isinstance(result, dict):
        raise NotarizationError("Apple Notary API returned an invalid response")
    return result


def sha256_file(path: Path) -> str:
    """Hash an artifact incrementally so large release files stay off the heap."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def hmac_sha256(key: bytes, value: str) -> bytes:
    """Derive one AWS Signature Version 4 signing-key component."""

    return hmac.new(key, value.encode("utf-8"), hashlib.sha256).digest()


def upload_to_apple_s3(
    path: Path,
    file_digest: str,
    credentials: dict[str, Any],
    *,
    timestamp: datetime.datetime | None = None,
) -> None:
    """Upload an artifact using the provided temporary credentials."""

    required = (
        "awsAccessKeyId",
        "awsSecretAccessKey",
        "awsSessionToken",
        "bucket",
        "object",
    )
    if any(
        not isinstance(credentials.get(name), str) or not credentials[name] for name in required
    ):
        raise NotarizationError("Apple returned incomplete temporary upload credentials")

    size = path.stat().st_size
    if not 0 < size <= MAX_SINGLE_UPLOAD_BYTES:
        raise NotarizationError("Notarization payload must be between 1 byte and 5 GiB")

    now = timestamp or datetime.datetime.now(datetime.timezone.utc)
    amz_date = now.strftime("%Y%m%dT%H%M%SZ")
    date_stamp = now.strftime("%Y%m%d")
    host = f"{credentials['bucket']}.s3.{AWS_REGION}.amazonaws.com"
    object_path = "/" + urllib.parse.quote(credentials["object"].lstrip("/"), safe="/-_.~")
    headers = {
        "content-type": "application/octet-stream",
        "host": host,
        "x-amz-content-sha256": file_digest,
        "x-amz-date": amz_date,
        "x-amz-security-token": credentials["awsSessionToken"],
    }
    # Build the signature required by the upload endpoint.
    signed_headers = ";".join(sorted(headers))
    canonical_headers = "".join(f"{name}:{headers[name]}\n" for name in sorted(headers))
    canonical_request = "\n".join(
        ["PUT", object_path, "", canonical_headers, signed_headers, file_digest]
    )
    credential_scope = f"{date_stamp}/{AWS_REGION}/s3/aws4_request"
    string_to_sign = "\n".join(
        [
            "AWS4-HMAC-SHA256",
            amz_date,
            credential_scope,
            hashlib.sha256(canonical_request.encode("utf-8")).hexdigest(),
        ]
    )
    signing_key = ("AWS4" + credentials["awsSecretAccessKey"]).encode("utf-8")
    for component in (date_stamp, AWS_REGION, "s3", "aws4_request"):
        signing_key = hmac_sha256(signing_key, component)
    signature = hmac.new(signing_key, string_to_sign.encode("utf-8"), hashlib.sha256).hexdigest()
    authorization = (
        "AWS4-HMAC-SHA256 "
        f"Credential={credentials['awsAccessKeyId']}/{credential_scope}, "
        f"SignedHeaders={signed_headers}, Signature={signature}"
    )

    # Stream the artifact to avoid buffering its full contents.
    with path.open("rb") as source:
        request = urllib.request.Request(
            f"https://{host}{object_path}",
            data=source,
            method="PUT",
            headers={
                **headers,
                "Authorization": authorization,
                "Content-Length": str(size),
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=300):
                pass
        except urllib.error.HTTPError as error:
            raise NotarizationError(
                f"Apple notarization payload upload failed with HTTP {error.code}"
            ) from error
        except OSError as error:
            raise NotarizationError("Apple notarization payload upload failed") from error


def write_developer_log(submission_id: str, token: str, destination: Path | None) -> None:
    """Write the submission's optional diagnostic report."""

    if destination is None:
        return
    response = json_request(f"{APPLE_NOTARY_URL}/{submission_id}/logs", token)
    try:
        log_url = response["data"]["attributes"]["developerLogUrl"]
    except (KeyError, TypeError) as error:
        raise NotarizationError("Apple did not provide a notarization developer log") from error
    if not isinstance(log_url, str) or not log_url.startswith("https://"):
        raise NotarizationError("Apple returned an invalid notarization developer log URL")
    try:
        # Do not forward unrelated authorization headers to the download URL.
        with urllib.request.urlopen(log_url, timeout=60) as response:
            destination.write_bytes(response.read())
    except urllib.error.HTTPError as error:
        raise NotarizationError(
            f"Apple notarization developer log download failed with HTTP {error.code}"
        ) from error
    except OSError as error:
        raise NotarizationError("Apple notarization developer log download failed") from error


def notarize(
    path: Path,
    configuration: NotarizationConfiguration,
    *,
    max_wait_seconds: int,
    report_log: Path | None,
) -> str:
    """Submit an artifact, upload it to Apple, and wait for a terminal result."""

    issued_at = int(time.time())
    token = create_apple_jwt(configuration, issued_at=issued_at)
    file_digest = sha256_file(path)
    # Create the submission before uploading the artifact.
    response = json_request(
        APPLE_NOTARY_URL,
        token,
        body={"sha256": file_digest, "submissionName": path.name},
    )
    try:
        submission_id = response["data"]["id"]
        upload_credentials = response["data"]["attributes"]
    except (KeyError, TypeError) as error:
        raise NotarizationError("Apple returned an invalid notarization submission") from error
    if not isinstance(submission_id, str) or not submission_id:
        raise NotarizationError("Apple did not return a notarization submission ID")
    if not isinstance(upload_credentials, dict):
        raise NotarizationError("Apple returned invalid notarization upload credentials")

    print(f"Uploading notarization submission {submission_id} for {path.name}")
    upload_to_apple_s3(path, file_digest, upload_credentials)
    # A monotonic deadline keeps the wait limit stable if the system clock changes.
    deadline = time.monotonic() + max_wait_seconds

    # Wait for completion, equivalent to `notarytool submit --wait`.
    while True:
        # Refresh the token before it expires.
        if int(time.time()) >= issued_at + JWT_LIFETIME_SECONDS - 60:
            issued_at = int(time.time())
            token = create_apple_jwt(configuration, issued_at=issued_at)
        result = json_request(f"{APPLE_NOTARY_URL}/{submission_id}", token)
        try:
            status = result["data"]["attributes"]["status"]
        except (KeyError, TypeError) as error:
            raise NotarizationError("Apple returned an invalid notarization status") from error

        if status in {"Accepted", "Invalid", "Rejected"}:
            # Save diagnostics for both accepted and rejected submissions.
            write_developer_log(submission_id, token, report_log)
            message = f"Notarization submission {submission_id} completed with status {status}"
            if status != "Accepted":
                raise NotarizationError(message)
            print(message)
            return submission_id
        if status != "In Progress":
            raise NotarizationError(f"Unexpected notarization status: {status!r}")

        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise NotarizationError(
                f"Notarization submission {submission_id} exceeded {max_wait_seconds} seconds"
            )
        print(f"Notarization submission {submission_id} is still in progress")
        time.sleep(min(POLL_INTERVAL_SECONDS, remaining))


def main() -> int:
    """Run notarization using command-line arguments."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--file", type=Path, required=True)
    parser.add_argument("--report-log", type=Path)
    parser.add_argument("--max-wait-seconds", type=int, default=600)
    arguments = parser.parse_args()

    try:
        if not arguments.file.is_file():
            raise NotarizationError(f"Notarization payload does not exist: {arguments.file}")
        if arguments.max_wait_seconds < 0:
            raise NotarizationError("--max-wait-seconds must be non-negative")
        if arguments.report_log is not None:
            arguments.report_log.parent.mkdir(parents=True, exist_ok=True)
        notarize(
            arguments.file,
            NotarizationConfiguration.from_environment(),
            max_wait_seconds=arguments.max_wait_seconds,
            report_log=arguments.report_log,
        )
    except NotarizationError as error:
        print(f"Notarization failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
