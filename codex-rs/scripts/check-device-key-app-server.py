#!/usr/bin/env python3
"""Exercise app-server device/key/* RPCs against a local Codex binary.

This is intentionally dependency-free so it can be run against a freshly built
and code-signed local CLI:

    python3 codex-rs/scripts/check-device-key-app-server.py \
      --binary codex-rs/target/debug/codex

The script uses a temporary CODEX_HOME and spawns `codex app-server --listen
stdio://`. It creates a new persistent device key each successful run; the
app-server API currently does not expose a delete RPC.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


DEFAULT_TOKEN_SHA256_EMPTY = "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU"


class DeviceKeyRpcError(RuntimeError):
    def __init__(self, method: str, error: dict[str, Any]) -> None:
        self.method = method
        self.error = error
        super().__init__(f"{method} failed: {error}")


class AppServerClient:
    def __init__(
        self,
        *,
        binary: Path,
        codex_home: Path,
        verbose: bool,
    ) -> None:
        self._verbose = verbose
        env = os.environ.copy()
        env["CODEX_HOME"] = str(codex_home)
        env["CODEX_APP_SERVER_MANAGED_CONFIG_PATH"] = str(
            codex_home / "managed_config.toml"
        )
        env.setdefault("RUST_LOG", "info")
        env.pop("CODEX_INTERNAL_ORIGINATOR_OVERRIDE", None)

        self._process = subprocess.Popen(
            [str(binary), "app-server", "--listen", "stdio://"],
            cwd=codex_home,
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        if self._process.stdin is None or self._process.stdout is None:
            raise RuntimeError("failed to open app-server stdio pipes")

        self._stdin = self._process.stdin
        self._stdout = self._process.stdout

    def close(self) -> None:
        if self._process.poll() is None:
            self._process.terminate()
            try:
                self._process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self._process.kill()
                self._process.wait(timeout=5)
        stderr = self._process.stderr.read() if self._process.stderr else ""
        if self._verbose and stderr:
            print("app-server stderr:", file=sys.stderr)
            print(stderr, file=sys.stderr, end="" if stderr.endswith("\n") else "\n")

    def send(self, message: dict[str, Any]) -> None:
        line = json.dumps(message, separators=(",", ":"))
        if self._verbose:
            print(f">>> {line}", file=sys.stderr)
        self._stdin.write(line + "\n")
        self._stdin.flush()

    def read_until_id(self, request_id: int, timeout_seconds: float) -> dict[str, Any]:
        deadline = time.monotonic() + timeout_seconds
        while time.monotonic() < deadline:
            line = self._stdout.readline()
            if not line:
                raise RuntimeError("app-server stdout closed")
            if self._verbose:
                print(f"<<< {line.strip()}", file=sys.stderr)
            message = json.loads(line)
            if message.get("id") == request_id:
                return message
        raise TimeoutError(f"timed out waiting for response id {request_id}")

    def request(
        self,
        request_id: int,
        method: str,
        params: dict[str, Any] | None,
        *,
        timeout_seconds: float,
    ) -> dict[str, Any]:
        message: dict[str, Any] = {"id": request_id, "method": method}
        if params is not None:
            message["params"] = params
        self.send(message)
        response = self.read_until_id(request_id, timeout_seconds)
        if "error" in response:
            raise DeviceKeyRpcError(method, response["error"])
        result = response.get("result")
        if not isinstance(result, dict):
            raise RuntimeError(f"{method} returned non-object result: {result!r}")
        return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run app-server device/key/create, device/key/sign, and device/key/public.",
    )
    parser.add_argument(
        "--binary",
        type=Path,
        default=Path("codex-rs/target/debug/codex"),
        help="Path to the codex CLI binary to test.",
    )
    parser.add_argument(
        "--codex-home",
        type=Path,
        help="CODEX_HOME to use. Defaults to a temporary directory.",
    )
    parser.add_argument(
        "--keep-codex-home",
        action="store_true",
        help="Do not delete the temporary CODEX_HOME after the run.",
    )
    parser.add_argument(
        "--protection-policy",
        choices=("hardware_only", "allow_os_protected_nonextractable"),
        default="hardware_only",
        help="Protection policy sent to device/key/create.",
    )
    parser.add_argument(
        "--account-user-id",
        default="acct_local_secure_enclave_check",
        help="accountUserId used for device/key/create and sign payload.",
    )
    parser.add_argument(
        "--client-id",
        default="cli_local_secure_enclave_check",
        help="clientId used for device/key/create and sign payload.",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=120.0,
        help="Timeout for each app-server request.",
    )
    parser.add_argument("--verbose", action="store_true", help="Print JSON-RPC traffic.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    binary = args.binary.expanduser().resolve()
    if not binary.is_file():
        print(f"binary not found: {binary}", file=sys.stderr)
        return 2

    temp_home: tempfile.TemporaryDirectory[str] | None = None
    if args.codex_home:
        codex_home = args.codex_home.expanduser().resolve()
        codex_home.mkdir(parents=True, exist_ok=True)
    else:
        temp_home = tempfile.TemporaryDirectory(prefix="codex-device-key-check-")
        codex_home = Path(temp_home.name)

    client = AppServerClient(binary=binary, codex_home=codex_home, verbose=args.verbose)
    try:
        init_result = client.request(
            1,
            "initialize",
            {
                "clientInfo": {
                    "name": "device-key-local-signing-check",
                    "version": "0.1.0",
                },
                "capabilities": {"experimentalApi": True},
            },
            timeout_seconds=args.timeout_seconds,
        )
        client.send({"method": "initialized"})

        created = client.request(
            2,
            "device/key/create",
            {
                "accountUserId": args.account_user_id,
                "clientId": args.client_id,
                "protectionPolicy": args.protection_policy,
            },
            timeout_seconds=args.timeout_seconds,
        )

        sign_payload = {
            "type": "remoteControlClientConnection",
            "nonce": "nonce-local-secure-enclave-check",
            "audience": "remote_control_client_websocket",
            "sessionId": "wssess_local_secure_enclave_check",
            "targetOrigin": "https://chatgpt.com",
            "targetPath": "/api/codex/remote/control/client",
            "accountUserId": args.account_user_id,
            "clientId": args.client_id,
            "tokenSha256Base64url": DEFAULT_TOKEN_SHA256_EMPTY,
            "tokenExpiresAt": 4_102_444_800,
            "scopes": ["remote_control_controller_websocket"],
        }
        signed = client.request(
            3,
            "device/key/sign",
            {"keyId": created["keyId"], "payload": sign_payload},
            timeout_seconds=args.timeout_seconds,
        )

        public = client.request(
            4,
            "device/key/public",
            {"keyId": created["keyId"]},
            timeout_seconds=args.timeout_seconds,
        )

        summary = {
            "binary": str(binary),
            "codexHome": str(codex_home),
            "initialize": {
                "platformOs": init_result.get("platformOs"),
                "userAgent": init_result.get("userAgent"),
            },
            "create": {
                "keyId": created.get("keyId"),
                "algorithm": created.get("algorithm"),
                "protectionClass": created.get("protectionClass"),
                "publicKeySpkiDerBase64Length": len(
                    created.get("publicKeySpkiDerBase64", "")
                ),
            },
            "sign": {
                "algorithm": signed.get("algorithm"),
                "signatureDerBase64Length": len(signed.get("signatureDerBase64", "")),
                "signedPayloadBase64Length": len(signed.get("signedPayloadBase64", "")),
            },
            "public": {
                "keyIdMatchesCreate": public.get("keyId") == created.get("keyId"),
                "algorithm": public.get("algorithm"),
                "protectionClass": public.get("protectionClass"),
                "publicKeyMatchesCreate": public.get("publicKeySpkiDerBase64")
                == created.get("publicKeySpkiDerBase64"),
            },
        }
        print(json.dumps(summary, indent=2, sort_keys=True))

        if created.get("protectionClass") != "hardware_secure_enclave":
            print(
                "device/key/create did not return hardware_secure_enclave",
                file=sys.stderr,
            )
            return 3
        return 0
    except DeviceKeyRpcError as exc:
        print(
            json.dumps(
                {
                    "binary": str(binary),
                    "codexHome": str(codex_home),
                    "failedMethod": exc.method,
                    "error": exc.error,
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 1
    finally:
        client.close()
        if temp_home is not None and not args.keep_codex_home:
            temp_home.cleanup()
        elif temp_home is not None:
            print(f"kept CODEX_HOME at {codex_home}", file=sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
