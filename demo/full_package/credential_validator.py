"""Lightweight credential validator used by the demo Docker compose stack.

The module provides two entry points:
- A simple CLI that validates a JSON credential payload and writes a log file.
- A FastAPI application (started with --serve) for HTTP validation checks.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Dict


DEFAULT_CREDENTIAL = {
    "credentialId": "urn:stagecred:demo-001",
    "signatureValue": "a1b2c3d4",
    "revoked": False,
    "issuer": "stageport-demo",
    "credentialSubject": {
        "id": "urn:hash:demo1234",
        "name": "Demo: AI Choreography for Ballet",
    },
    "proof": {
        "type": "Ed25519Signature2018",
        "created": "2025-08-01T12:00:00Z",
        "proofPurpose": "assertionMethod",
        "verificationMethod": "https://stageport.studio/.well-known/stageport-public-key.json",
        "signatureValue": "a1b2c3d4",
    },
}


def _signature_value(payload: Dict[str, Any]) -> str:
    proof = payload.get("proof", {})
    return (payload.get("signatureValue") or proof.get("signatureValue") or "").strip()


def _write_log(report: Dict[str, Any], log_path: Path) -> None:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    existing = []
    if log_path.exists():
        existing = json.loads(log_path.read_text(encoding="utf-8"))
        if not isinstance(existing, list):
            existing = [existing]
    existing.append(report)
    log_path.write_text(json.dumps(existing, indent=2), encoding="utf-8")


def validate_credential(payload: Dict[str, Any]) -> Dict[str, Any]:
    """Return a structured validation report for the provided credential payload."""

    credential_id = str(payload.get("credentialId", "")).strip()
    signature_value = _signature_value(payload)

    errors = []
    if not credential_id:
        errors.append("Missing credentialId")
    if not signature_value:
        errors.append("Missing signatureValue")

    verified = not errors
    report = {
        "credentialId": credential_id,
        "verified": verified,
        "revoked": bool(payload.get("revoked", False)),
        "issuer": payload.get("issuer", "stageport-demo"),
        "signatureValue": signature_value,
        "credentialSubject": payload.get("credentialSubject", {}),
        "errors": errors,
        "proof": payload.get("proof", {}),
    }
    return report


def _run_cli(args: argparse.Namespace) -> None:
    payload = DEFAULT_CREDENTIAL if args.input is None else json.loads(Path(args.input).read_text(encoding="utf-8"))
    report = validate_credential(payload)
    _write_log(report, args.log)
    print(json.dumps(report, indent=2))


def _start_api(args: argparse.Namespace) -> None:
    from fastapi import FastAPI
    from fastapi.responses import JSONResponse
    from pydantic import BaseModel
    import uvicorn

    class CredentialRequest(BaseModel):
        credentialId: str
        signatureValue: str | None = None
        revoked: bool | None = False
        issuer: str | None = None
        credentialSubject: Dict[str, Any] | None = None
        proof: Dict[str, Any] | None = None

    app = FastAPI(title="StagePort Credential Validator", version="0.1.0")

    @app.post("/validate")
    def validate(request: CredentialRequest) -> JSONResponse:
        report = validate_credential(request.model_dump())
        _write_log(report, args.log)
        status_code = 200 if report["verified"] else 400
        return JSONResponse(content=report, status_code=status_code)

    uvicorn.run(app, host="0.0.0.0", port=args.port)


def main() -> None:
    parser = argparse.ArgumentParser(description="Validate StagePort credential payloads.")
    parser.add_argument("--serve", action="store_true", help="Start the FastAPI server instead of running a single check.")
    parser.add_argument("--port", type=int, default=8000, help="Port to bind when running the FastAPI server.")
    parser.add_argument("--input", type=str, help="Path to a JSON credential payload to validate once and exit.")
    parser.add_argument("--log", type=Path, default=Path("credentials_log.json"), help="Path to the JSON log file.")
    args = parser.parse_args()

    if args.serve:
        _start_api(args)
    else:
        _run_cli(args)


if __name__ == "__main__":
    main()
