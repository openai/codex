import contextlib
import hashlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SIGNING_DIRECTORY = Path(__file__).resolve().parent
sys.path.insert(0, str(SIGNING_DIRECTORY))
import notarize_with_akv as notary  # noqa: E402

CONFIGURATION = notary.NotarizationConfiguration(
    "01234567-89ab-cdef-0123-456789abcdef",
    "APPLEKEY01",
    "notary-vault",
    "example-signing-key",
    "0123456789abcdef" * 2,
)
ENVIRONMENT = {
    "AZURE_KEYVAULT_NAME": CONFIGURATION.vault_name,
    "APPLE_NOTARIZATION_AKV_KEY_NAME": CONFIGURATION.vault_key_name,
}
CREDENTIALS = {
    "awsAccessKeyId": "temporary-access-key",
    "awsSecretAccessKey": "temporary-secret-key",
    "awsSessionToken": "temporary-session-token",
    "bucket": "apple-notary-bucket",
    "object": "uploads/release artifact.dmg",
}


def response(payload):
    return io.BytesIO(json.dumps(payload).encode() if isinstance(payload, dict) else payload)


def azure_response(payload):
    return subprocess.CompletedProcess([], 0, stdout=json.dumps(payload), stderr="")


class NotarizationTest(unittest.TestCase):
    def test_validates_the_apple_key_tags_and_pins_its_version(self) -> None:
        metadata = {
            "id": CONFIGURATION.versioned_key_id,
            "apple_key_id": "APPLEKEY01",
            "apple_issuer_id": CONFIGURATION.issuer_id,
        }
        with (
            patch.dict(os.environ, ENVIRONMENT, clear=True),
            patch.object(
                notary.subprocess,
                "run",
                return_value=azure_response(metadata),
            ) as show,
        ):
            self.assertEqual(
                notary.NotarizationConfiguration.from_environment(),
                CONFIGURATION,
            )
            show.return_value = azure_response({**metadata, "apple_key_id": None})
            with self.assertRaisesRegex(notary.NotarizationError, "apple-key-id tag"):
                notary.NotarizationConfiguration.from_environment()
            for issuer_id in (None, "", "   "):
                with self.subTest(issuer_id=issuer_id):
                    show.return_value = azure_response({**metadata, "apple_issuer_id": issuer_id})
                    with self.assertRaisesRegex(notary.NotarizationError, "apple-issuer-id tag"):
                        notary.NotarizationConfiguration.from_environment()
            show.return_value = azure_response({**metadata, "apple_issuer_id": "invalid-issuer"})
            with self.assertRaisesRegex(notary.NotarizationError, "valid UUID"):
                notary.NotarizationConfiguration.from_environment()
        self.assertIn(
            '{id:key.kid,apple_key_id:tags."apple-key-id",'
            'apple_issuer_id:tags."apple-issuer-id"}',
            show.call_args.args[0],
        )

        invalid_environment = {**ENVIRONMENT, "APPLE_NOTARIZATION_AKV_KEY_NAME": "../bad"}
        with (
            patch.dict(os.environ, invalid_environment, clear=True),
            self.assertRaisesRegex(notary.NotarizationError, "key name"),
        ):
            notary.NotarizationConfiguration.from_environment()

    def test_signs_the_notary_scoped_digest_with_the_pinned_key(self) -> None:
        signature = bytes(range(64))
        payload = {
            "kid": CONFIGURATION.versioned_key_id,
            "value": notary.base64url_encode(signature),
        }
        with patch.object(notary.subprocess, "run", return_value=azure_response(payload)) as sign:
            token = notary.create_apple_jwt(CONFIGURATION, issued_at=1_780_000_000)
        header, claims, encoded_signature = token.split(".")
        self.assertEqual(json.loads(notary.base64url_decode(header))["kid"], "APPLEKEY01")
        self.assertEqual(
            json.loads(notary.base64url_decode(claims))["iss"], CONFIGURATION.issuer_id
        )
        self.assertEqual(
            json.loads(notary.base64url_decode(claims))["scope"],
            ["/notary/v2"],
        )
        self.assertEqual(notary.base64url_decode(encoded_signature), signature)
        command = sign.call_args.args[0]
        body = json.loads(command[command.index("--body") + 1])
        self.assertEqual(body["alg"], "ES256")
        self.assertEqual(
            notary.base64url_decode(body["value"]),
            hashlib.sha256(f"{header}.{claims}".encode()).digest(),
        )

    def test_rejects_wrong_key_versions_and_invalid_signatures(self) -> None:
        cases = (
            (CONFIGURATION.versioned_key_id + "wrong", bytes(64), "unexpected key version"),
            (CONFIGURATION.versioned_key_id, b"short", "64 JOSE"),
        )
        for key_id, signature, message in cases:
            payload = {"kid": key_id, "value": notary.base64url_encode(signature)}
            with (
                self.subTest(message=message),
                patch.object(
                    notary.subprocess,
                    "run",
                    return_value=azure_response(payload),
                ),
                self.assertRaisesRegex(notary.NotarizationError, message),
            ):
                notary.create_apple_jwt(CONFIGURATION, issued_at=1)

    def test_uploads_polls_and_retains_sanitized_apple_diagnostics(self) -> None:
        signature = azure_response(
            {
                "kid": CONFIGURATION.versioned_key_id,
                "value": notary.base64url_encode(bytes(64)),
            }
        )
        responses = [
            response({"data": {"id": "submission-1", "attributes": CREDENTIALS}}),
            response(b""),
            response({"data": {"attributes": {"status": "In Progress"}}}),
            response({"data": {"attributes": {"status": "Accepted"}}}),
            response({"data": {"attributes": {"developerLogUrl": "https://logs.example.com/log"}}}),
            response(b'{"status":"Accepted"}'),
        ]
        with tempfile.TemporaryDirectory() as directory:
            artifact, log = (
                Path(directory) / "codex.zip",
                Path(directory) / "notary-log.json",
            )
            artifact.write_bytes(b"signed release binary")
            output = io.StringIO()
            with (
                patch.object(notary.subprocess, "run", return_value=signature),
                patch.object(notary.urllib.request, "urlopen", side_effect=responses) as requests,
                patch.object(notary.time, "sleep") as sleep,
                contextlib.redirect_stdout(output),
            ):
                submission = notary.notarize(
                    artifact, CONFIGURATION, max_wait_seconds=600, report_log=log
                )
            self.assertEqual(submission, "submission-1")
            self.assertEqual(json.loads(log.read_text()), {"status": "Accepted"})
            sleep.assert_called_once_with(10)
            upload = requests.call_args_list[1].args[0]
            self.assertEqual(upload.get_method(), "PUT")
            self.assertNotIn("temporary-secret-key", upload.get_header("Authorization"))
            self.assertNotIn("temporary-secret-key", output.getvalue())


class NotarizationWrapperTest(unittest.TestCase):
    def test_binary_and_dmg_wrappers_preserve_notarization_contracts(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, tools = Path(directory), Path(directory) / "tools"
            tools.mkdir()
            call_log = root / "calls.txt"
            environment = {
                **os.environ,
                **ENVIRONMENT,
                "PATH": f"{tools}:{os.environ['PATH']}",
                "RUNNER_TEMP": str(root),
                "CALL_LOG": str(call_log),
            }
            for name in ("az", "python3", "rcodesign"):
                body = "exit 0" if name == "az" else f'printf "{name} %s\\n" "$*" >> "$CALL_LOG"'
                executable = tools / name
                executable.write_text(f"#!/usr/bin/env bash\nset -euo pipefail\n{body}\n")
                executable.chmod(0o755)
            for kind in ("binary", "dmg"):
                with self.subTest(kind=kind):
                    artifact = root / ("codex.dmg" if kind == "dmg" else "codex")
                    artifact.write_bytes(b"signed release artifact")
                    subprocess.run(
                        [
                            str(SIGNING_DIRECTORY / f"notarize_macos_{kind}_with_akv.sh"),
                            f"--{kind}",
                            str(artifact),
                            "--report-dir",
                            str(root / "report"),
                        ],
                        env=environment,
                        check=True,
                        capture_output=True,
                        text=True,
                    )
                    calls = call_log.read_text().splitlines()
                    self.assertIn("notarize_with_akv.py --file", calls[0])
                    if kind == "dmg":
                        self.assertEqual(calls[1], f"rcodesign staple {artifact}")
                    else:
                        self.assertEqual(len(calls), 1)
                    self.assertEqual(list(root.rglob("*.p8")), [])
                    call_log.unlink()


if __name__ == "__main__":
    unittest.main()
