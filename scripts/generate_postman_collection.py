#!/usr/bin/env python3
"""Generate a Postman collection from the SmallWallets OpenAPI spec."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Dict, List

DEFAULT_SPEC = Path("smallwallets-openapi.json")
DEFAULT_OUTPUT = Path("postman.collection.json")
DEFAULT_BASE_URL = "https://sandbox.smallwallets.dev/v1"


def method_to_pm(method: str) -> str:
    return method.upper()


def path_to_variables(path: str) -> List[str]:
    vars_: List[str] = []
    idx = 0
    while True:
        start = path.find("{", idx)
        if start == -1:
            break
        end = path.find("}", start)
        if end == -1:
            break
        vars_.append(path[start + 1 : end])
        idx = end + 1
    return vars_


def path_to_segments(path: str) -> List[str]:
    stripped = path.strip("/")
    if not stripped:
        return []
    return stripped.split("/")


def parameter_to_query(param: Dict[str, Any]) -> Dict[str, Any]:
    return {
        "key": param.get("name", ""),
        "value": "",
        "description": param.get("description", ""),
    }


def build_collection(spec: Dict[str, Any]) -> Dict[str, Any]:
    servers = spec.get("servers") or []
    if len(servers) > 1:
        base_url = servers[1].get("url", DEFAULT_BASE_URL)
    elif servers:
        base_url = servers[0].get("url", DEFAULT_BASE_URL)
    else:
        base_url = DEFAULT_BASE_URL

    collection: Dict[str, Any] = {
        "info": {
            "name": "SmallWallets API (Collection)",
            "_postman_id": "f6f9d5f6-aaaa-bbbb-cccc-000000000001",
            "description": "Auto-generated requests from the SmallWallets OpenAPI spec.",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json",
        },
        "item": [],
        "variable": [
            {"key": "baseUrl", "value": base_url},
            {"key": "token", "value": "REPLACE_ME_JWT"},
        ],
    }

    for path, operations in spec.get("paths", {}).items():
        folder = {"name": path, "item": []}
        for method, op in operations.items():
            if method.lower() not in {"get", "post", "put", "patch", "delete"}:
                continue
            name = op.get("summary") or f"{method.upper()} {path}"
            pm_request: Dict[str, Any] = {
                "name": name,
                "request": {
                    "auth": {
                        "type": "bearer",
                        "bearer": [
                            {"key": "token", "value": "{{token}}", "type": "string"}
                        ],
                    },
                    "method": method_to_pm(method),
                    "header": [
                        {"key": "Authorization", "value": "Bearer {{token}}"},
                        {"key": "Content-Type", "value": "application/json"},
                    ],
                    "url": {
                        "raw": "{{baseUrl}}" + path,
                        "host": ["{{baseUrl}}"],
                        "path": path_to_segments(path),
                    },
                },
                "response": [],
            }

            params = op.get("parameters", [])
            if operations.get("parameters"):
                params = operations["parameters"] + params
            query_params = [
                parameter_to_query(param)
                for param in params
                if param.get("in") == "query"
            ]
            if query_params:
                pm_request["request"]["url"]["query"] = query_params

            vars_ = path_to_variables(path)
            if vars_:
                pm_request["request"]["url"]["variable"] = [
                    {"key": var, "value": f"<{var}>"}
                    for var in vars_
                ]

            if method.lower() != "get":
                pm_request["request"]["body"] = {
                    "mode": "raw",
                    "raw": "{}",
                    "options": {"raw": {"language": "json"}},
                }

            folder["item"].append(pm_request)
        if folder["item"]:
            collection["item"].append(folder)

    return collection


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate the Postman collection for SmallWallets")
    parser.add_argument("--spec", type=Path, default=DEFAULT_SPEC)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="Only succeed when the generated output matches the on-disk file.",
    )
    args = parser.parse_args()

    spec_path = args.spec
    output_path = args.output

    spec = json.loads(spec_path.read_text(encoding="utf-8"))
    collection = build_collection(spec)
    rendered = json.dumps(collection, indent=2) + "\n"

    if args.check and output_path.exists():
        current = output_path.read_text(encoding="utf-8")
        if current != rendered:
            print("Postman collection is out of date. Run without --check to regenerate.")
            return 1
        return 0

    output_path.write_text(rendered, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
