#!/usr/bin/env python3
"""Generate lightweight typed models from app-server v2 JSON schemas.

The generator intentionally focuses on a high-value subset used by the Python SDK,
including core thread/turn responses and common server notifications.

It emits dataclasses and TypedDict stubs into `src/codex_app_server/schema_types.py`.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

TARGET_SCHEMAS = {
    "ThreadStartResponse": "ThreadStartResponse.json",
    "ThreadResumeResponse": "ThreadResumeResponse.json",
    "ThreadReadResponse": "ThreadReadResponse.json",
    "ThreadListResponse": "ThreadListResponse.json",
    "TurnStartResponse": "TurnStartResponse.json",
    "ModelListResponse": "ModelListResponse.json",
    "ThreadStartedNotificationPayload": "ThreadStartedNotification.json",
    "TurnStartedNotificationPayload": "TurnStartedNotification.json",
    "TurnCompletedNotificationPayload": "TurnCompletedNotification.json",
    "AgentMessageDeltaNotificationPayload": "AgentMessageDeltaNotification.json",
    "ErrorNotificationPayload": "ErrorNotification.json",
}


@dataclass(slots=True)
class FieldSpec:
    name: str
    annotation: str
    required: bool
    source_expr: str


@dataclass(slots=True)
class ClassSpec:
    name: str
    fields: list[FieldSpec]


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _schema_dir(repo_root: Path) -> Path:
    return repo_root / "codex-rs" / "app-server-protocol" / "schema" / "json" / "v2"


def _python_type_for_schema(schema: dict[str, Any], defs: dict[str, Any], nested: set[str]) -> tuple[str, str]:
    if "$ref" in schema:
        ref_name = schema["$ref"].split("/")[-1]
        ref_def = defs.get(ref_name, {})
        if ref_name in nested:
            return ref_name, "object"
        if ref_def.get("type") == "string":
            return "str", "scalar"
        if ref_def.get("type") == "integer":
            return "int", "scalar"
        if ref_def.get("type") == "boolean":
            return "bool", "scalar"
        return "Any", "scalar"

    if "allOf" in schema and schema["allOf"]:
        return _python_type_for_schema(schema["allOf"][0], defs, nested)

    if "anyOf" in schema:
        non_null = [x for x in schema["anyOf"] if x.get("type") != "null"]
        has_null = len(non_null) != len(schema["anyOf"])
        if len(non_null) == 1:
            inner_t, inner_k = _python_type_for_schema(non_null[0], defs, nested)
            if has_null:
                return f"{inner_t} | None", inner_k
            return inner_t, inner_k
        return "Any", "scalar"

    t = schema.get("type")
    if isinstance(t, list):
        non_null = [x for x in t if x != "null"]
        if len(non_null) == 1:
            inner_t, _ = _python_type_for_schema({"type": non_null[0]}, defs, nested)
            return f"{inner_t} | None", "scalar"
        return "Any", "scalar"

    if t == "string":
        return "str", "scalar"
    if t == "integer":
        return "int", "scalar"
    if t == "boolean":
        return "bool", "scalar"
    if t == "array":
        items = schema.get("items", {})
        item_t, _ = _python_type_for_schema(items, defs, nested)
        return f"list[{item_t}]", "array"
    if t == "object":
        return "dict[str, Any]", "object"

    return "Any", "scalar"


def _field_source_expr(field_name: str, py_type: str, kind: str, required: bool, default_fallback: str) -> str:
    getter = f'payload.get("{field_name}")'
    if required:
        getter = f'payload.get("{field_name}", {default_fallback})'

    if py_type == "str":
        return f"str({getter} or \"\")"
    if py_type == "int":
        return f"int({getter} or 0)"
    if py_type == "bool":
        return f"bool({getter})"
    if py_type.endswith(" | None"):
        base = py_type[:-7]
        if base in {"str", "int", "bool"}:
            return f"None if {getter} is None else {base}({getter})"
        return getter
    if kind == "array":
        return f"list({getter} or [])"
    return getter


def _class_from_schema(name: str, schema: dict[str, Any], defs: dict[str, Any], nested: set[str]) -> ClassSpec:
    props = schema.get("properties", {})
    required_set = set(schema.get("required", []))
    fields: list[FieldSpec] = []
    for field_name, field_schema in props.items():
        py_type, kind = _python_type_for_schema(field_schema, defs, nested)
        required = field_name in required_set
        fallback = "[]" if kind == "array" else "{}" if kind == "object" else "None"
        source_expr = _field_source_expr(field_name, py_type, kind, required, fallback)
        fields.append(FieldSpec(field_name, py_type, required, source_expr))
    return ClassSpec(name=name, fields=fields)


def _build_specs() -> list[ClassSpec]:
    repo = _repo_root()
    sdir = _schema_dir(repo)

    raw_roots: dict[str, dict[str, Any]] = {}
    defs_merged: dict[str, Any] = {}
    for class_name, fname in TARGET_SCHEMAS.items():
        data = json.loads((sdir / fname).read_text())
        raw_roots[class_name] = data
        defs_merged.update(data.get("definitions", {}))

    nested = {"Thread", "Turn"}
    specs: list[ClassSpec] = []

    for nested_name in sorted(nested):
        nested_schema = defs_merged.get(nested_name, {})
        if nested_schema:
            specs.append(_class_from_schema(nested_name, nested_schema, defs_merged, nested))

    for name, root_schema in raw_roots.items():
        specs.append(_class_from_schema(name, root_schema, defs_merged, nested))

    return specs


def _render(specs: list[ClassSpec]) -> str:
    out: list[str] = []
    out.append("# Auto-generated by scripts/generate_types_from_schema.py\n")
    out.append("# DO NOT EDIT MANUALLY.\n\n")
    out.append("from __future__ import annotations\n\n")
    out.append("from dataclasses import dataclass\n")
    out.append("from typing import Any, TypedDict\n\n")

    for spec in specs:
        out.append(f"class {spec.name}Dict(TypedDict, total=False):\n")
        if spec.fields:
            for f in spec.fields:
                out.append(f"    {f.name}: {f.annotation}\n")
        else:
            out.append("    pass\n")
        out.append("\n")

        out.append("@dataclass(slots=True, kw_only=True)\n")
        out.append(f"class {spec.name}:\n")
        if spec.fields:
            for f in spec.fields:
                default = "" if f.required else " = None"
                if f.annotation.startswith("list["):
                    default = " = None"
                out.append(f"    {f.name}: {f.annotation}{default}\n")
        else:
            out.append("    pass\n")
        out.append("\n")
        out.append("    @classmethod\n")
        out.append(f"    def from_dict(cls, payload: dict[str, Any]) -> \"{spec.name}\":\n")
        out.append("        payload = payload or {}\n")
        if spec.fields:
            out.append("        return cls(\n")
            for f in spec.fields:
                expr = f.source_expr
                if f.annotation == "Thread" and f.name in {"thread"}:
                    expr = "Thread.from_dict(payload.get(\"thread\") or {})"
                elif f.annotation == "Turn" and f.name in {"turn"}:
                    expr = "Turn.from_dict(payload.get(\"turn\") or {})"
                elif f.annotation == "list[Thread]" and f.name == "data":
                    expr = "[Thread.from_dict(item or {}) for item in (payload.get(\"data\") or [])]"
                out.append(f"            {f.name}={expr},\n")
            out.append("        )\n")
        else:
            out.append("        return cls()\n")
        out.append("\n\n")

    names = [spec.name for spec in specs] + [f"{spec.name}Dict" for spec in specs]
    out.append("__all__ = [\n")
    for n in names:
        out.append(f"    \"{n}\",\n")
    out.append("]\n")
    return "".join(out)


def main() -> None:
    repo = _repo_root()
    target = repo / "sdk" / "python" / "src" / "codex_app_server" / "schema_types.py"
    specs = _build_specs()
    target.write_text(_render(specs))
    print(f"wrote {target}")


if __name__ == "__main__":
    main()
