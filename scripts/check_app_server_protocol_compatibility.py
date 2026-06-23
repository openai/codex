#!/usr/bin/env python3

import argparse
import io
import json
import subprocess
import sys
import tarfile
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Any


REPO_SCHEMA_ROOT = "codex-rs/app-server-protocol/schema"
ROOT_SCHEMAS = {
    "json/ClientNotification.json": "input",
    "json/ClientRequest.json": "input",
    "json/ServerNotification.json": "output",
    "json/ServerRequest.json": "output",
}
OUTBOUND_ROOT_RESPONSES = {"json/FuzzyFileSearchResponse.json"}
TYPESCRIPT_INDEXES = ("typescript/index.ts", "typescript/v2/index.ts")


class Direction(Enum):
    INPUT = "client -> server"
    OUTPUT = "server -> client"


@dataclass(frozen=True, order=True)
class Violation:
    code: str
    path: str
    detail: str


def load_git_tree(revision: str) -> dict[str, str]:
    prefix = f"{REPO_SCHEMA_ROOT}/"
    archive = subprocess.run(
        ["git", "archive", "--format=tar", revision, REPO_SCHEMA_ROOT],
        check=True,
        capture_output=True,
    ).stdout
    files = {}
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as contents:
        for member in contents.getmembers():
            if not member.isfile() or not member.name.startswith(prefix):
                continue
            extracted = contents.extractfile(member)
            if extracted is not None:
                files[member.name.removeprefix(prefix)] = extracted.read().decode(
                    "utf-8"
                )
    return files


def load_schema_tree(root: Path) -> dict[str, str]:
    return {
        path.relative_to(root).as_posix(): path.read_text(encoding="utf-8")
        for path in root.rglob("*")
        if path.is_file()
    }


def schema_direction(path: str) -> Direction | None:
    root_direction = ROOT_SCHEMAS.get(path)
    if root_direction is not None:
        return Direction.INPUT if root_direction == "input" else Direction.OUTPUT

    schema_path = Path(path)
    if schema_path.suffix != ".json" or not schema_path.name.endswith("Response.json"):
        return None
    if schema_path.parts[:2] == ("json", "v2"):
        return Direction.OUTPUT
    if schema_path.parent == Path("json"):
        if path in OUTBOUND_ROOT_RESPONSES:
            return Direction.OUTPUT
        if schema_path.name.startswith("JSONRPC"):
            return None
        return Direction.INPUT
    return None


def parse_json_schema(raw: str, path: str) -> Any:
    try:
        return json.loads(raw)
    except json.JSONDecodeError as error:
        raise ValueError(f"invalid JSON schema {path}: {error}") from error


def resolve_ref(schema: Any, document: Any) -> Any:
    seen: set[str] = set()
    while isinstance(schema, dict) and isinstance(schema.get("$ref"), str):
        reference = schema["$ref"]
        if not reference.startswith("#/") or reference in seen:
            break
        seen.add(reference)
        target = document
        for component in reference[2:].split("/"):
            key = component.replace("~1", "/").replace("~0", "~")
            if not isinstance(target, dict) or key not in target:
                return schema
            target = target[key]
        schema = target
    return schema


def schema_types(schema: dict[str, Any]) -> set[str] | None:
    value = schema.get("type")
    if isinstance(value, str):
        return {value}
    if isinstance(value, list) and all(isinstance(item, str) for item in value):
        return set(value)
    return None


def variant_key(schema: Any, index: int) -> str:
    if not isinstance(schema, dict):
        return f"index:{index}"
    if isinstance(schema.get("$ref"), str):
        return f"ref:{schema['$ref'].rsplit('/', 1)[-1]}"
    for property_name in ("method", "type"):
        property_schema = schema.get("properties", {}).get(property_name, {})
        values = (
            property_schema.get("enum") if isinstance(property_schema, dict) else None
        )
        if isinstance(values, list) and len(values) == 1:
            return f"{property_name}:{values[0]}"
    if isinstance(schema.get("title"), str):
        return f"title:{schema['title']}"
    if schema.get("type") == "null":
        return "type:null"
    if "enum" in schema:
        return "enum"
    return f"index:{index}"


class SchemaComparator:
    def __init__(
        self,
        base_document: Any,
        head_document: Any,
        direction: Direction,
        root_path: str,
    ) -> None:
        self.base_document = base_document
        self.head_document = head_document
        self.direction = direction
        self.root_path = root_path
        self.violations: set[Violation] = set()
        self.seen: set[tuple[int, int, Direction]] = set()

    def compare(self) -> set[Violation]:
        self._compare(self.base_document, self.head_document, "#")
        return self.violations

    def _add(self, code: str, path: str, detail: str) -> None:
        self.violations.add(
            Violation(
                code, f"{self.root_path}{path}", f"{self.direction.value}: {detail}"
            )
        )

    def _compare(self, base: Any, head: Any, path: str) -> None:
        base = resolve_ref(base, self.base_document)
        head = resolve_ref(head, self.head_document)
        pair = (id(base), id(head), self.direction)
        if pair in self.seen:
            return
        self.seen.add(pair)

        if isinstance(base, bool) or isinstance(head, bool):
            if base is True and head is not True and self.direction is Direction.INPUT:
                self._add(
                    "schema_narrowed",
                    path,
                    "an unrestricted input schema became restricted",
                )
            elif (
                head is True and base is not True and self.direction is Direction.OUTPUT
            ):
                self._add(
                    "schema_widened", path, "an output schema became unrestricted"
                )
            return
        if not isinstance(base, dict) or not isinstance(head, dict):
            if base != head:
                self._add("schema_changed", path, "schema shape changed")
            return

        base_types = schema_types(base)
        head_types = schema_types(head)
        if base_types is not None and head_types is not None:
            incompatible = (
                base_types - head_types
                if self.direction is Direction.INPUT
                else head_types - base_types
            )
            if incompatible:
                self._add(
                    "type_changed",
                    path,
                    f"incompatible type(s) {sorted(incompatible)}; before={sorted(base_types)}, after={sorted(head_types)}",
                )

        self._compare_enum(base, head, path)
        self._compare_union(base, head, path)
        self._compare_all_of(base, head, path)
        self._compare_object(base, head, path)

        if "items" in base and "items" in head:
            self._compare(base["items"], head["items"], f"{path}/items")

    def _compare_enum(
        self, base: dict[str, Any], head: dict[str, Any], path: str
    ) -> None:
        base_values = base.get("enum")
        head_values = head.get("enum")
        if not isinstance(base_values, list) or not isinstance(head_values, list):
            narrowed_input = not isinstance(base_values, list) and isinstance(
                head_values, list
            )
            widened_output = isinstance(base_values, list) and not isinstance(
                head_values, list
            )
            if (self.direction is Direction.INPUT and narrowed_input) or (
                self.direction is Direction.OUTPUT and widened_output
            ):
                self._add("enum_changed", path, "enum constraint was added or removed")
            return
        base_set = {json.dumps(value, sort_keys=True) for value in base_values}
        head_set = {json.dumps(value, sort_keys=True) for value in head_values}
        incompatible = (
            base_set - head_set
            if self.direction is Direction.INPUT
            else head_set - base_set
        )
        if incompatible:
            self._add(
                "enum_changed",
                path,
                f"incompatible enum value(s) {sorted(incompatible)}",
            )

    def _compare_union(
        self, base: dict[str, Any], head: dict[str, Any], path: str
    ) -> None:
        for keyword in ("oneOf", "anyOf"):
            base_variants = base.get(keyword)
            head_variants = head.get(keyword)
            if not isinstance(base_variants, list) or not isinstance(
                head_variants, list
            ):
                narrowed_input = not isinstance(base_variants, list) and isinstance(
                    head_variants, list
                )
                widened_output = isinstance(base_variants, list) and not isinstance(
                    head_variants, list
                )
                if (self.direction is Direction.INPUT and narrowed_input) or (
                    self.direction is Direction.OUTPUT and widened_output
                ):
                    self._add(
                        "union_changed",
                        f"{path}/{keyword}",
                        "union constraint was added or removed",
                    )
                continue
            base_by_key = {
                variant_key(variant, index): variant
                for index, variant in enumerate(base_variants)
            }
            head_by_key = {
                variant_key(variant, index): variant
                for index, variant in enumerate(head_variants)
            }
            required_keys = (
                base_by_key.keys()
                if self.direction is Direction.INPUT
                else head_by_key.keys()
            )
            for key in required_keys:
                if key not in base_by_key or key not in head_by_key:
                    self._add(
                        "union_variant_changed",
                        f"{path}/{keyword}/{key}",
                        "union variant is not understood by both protocol versions",
                    )
                    continue
                self._compare(
                    base_by_key[key],
                    head_by_key[key],
                    f"{path}/{keyword}/{key}",
                )

            for key in base_by_key.keys() - head_by_key.keys():
                if key.startswith("method:"):
                    self._add(
                        "method_removed",
                        f"{path}/{keyword}/{key}",
                        "method was removed",
                    )

    def _compare_all_of(
        self, base: dict[str, Any], head: dict[str, Any], path: str
    ) -> None:
        base_parts = base.get("allOf")
        head_parts = head.get("allOf")
        if not isinstance(base_parts, list) or not isinstance(head_parts, list):
            return
        if len(base_parts) != len(head_parts):
            self._add("all_of_changed", f"{path}/allOf", "allOf shape changed")
            return
        for index, (base_part, head_part) in enumerate(zip(base_parts, head_parts)):
            self._compare(base_part, head_part, f"{path}/allOf/{index}")

    def _compare_object(
        self, base: dict[str, Any], head: dict[str, Any], path: str
    ) -> None:
        base_properties = base.get("properties")
        head_properties = head.get("properties")
        if not isinstance(base_properties, dict) and not isinstance(
            head_properties, dict
        ):
            return
        base_properties = base_properties if isinstance(base_properties, dict) else {}
        head_properties = head_properties if isinstance(head_properties, dict) else {}

        for name in base_properties.keys() - head_properties.keys():
            self._add(
                "property_removed", f"{path}/properties/{name}", "property was removed"
            )

        base_required = set(base.get("required", []))
        head_required = set(head.get("required", []))
        common_properties = base_properties.keys() & head_properties.keys()
        incompatible_required = (
            head_required - base_required
            if self.direction is Direction.INPUT
            else (base_required - head_required) & common_properties
        )
        for name in incompatible_required:
            detail = (
                "property became required"
                if self.direction is Direction.INPUT
                else "required output property became optional"
            )
            self._add("required_changed", f"{path}/properties/{name}", detail)

        for name in common_properties:
            self._compare(
                base_properties[name],
                head_properties[name],
                f"{path}/properties/{name}",
            )

        base_additional = base.get("additionalProperties", True)
        head_additional = head.get("additionalProperties", True)
        if self.direction is Direction.INPUT:
            if base_additional is not False and head_additional is False:
                self._add(
                    "additional_properties_narrowed",
                    path,
                    "input object stopped accepting additional properties",
                )
        elif base_additional is False:
            for name in head_properties.keys() - base_properties.keys():
                self._add(
                    "property_added_to_closed_output",
                    f"{path}/properties/{name}",
                    "output property was added to an object that rejected unknown fields",
                )
            if head_additional is not False:
                self._add(
                    "additional_properties_widened",
                    path,
                    "output object may now emit additional properties",
                )

        if isinstance(base_additional, dict) and isinstance(head_additional, dict):
            self._compare(
                base_additional,
                head_additional,
                f"{path}/additionalProperties",
            )


def exported_types(raw: str) -> set[str]:
    exports = set()
    for line in raw.splitlines():
        prefix = "export type { "
        if line.startswith(prefix) and " }" in line:
            exports.add(line[len(prefix) : line.index(" }")])
    return exports


def compare_protocol_trees(
    base_tree: dict[str, str], head_tree: dict[str, str]
) -> list[Violation]:
    violations: set[Violation] = set()
    paths = sorted(set(base_tree) | set(head_tree))
    for path in paths:
        direction = schema_direction(path)
        if direction is None:
            continue
        if path not in head_tree:
            violations.add(Violation("schema_removed", path, "schema file was removed"))
            continue
        if path not in base_tree:
            continue
        base_document = parse_json_schema(base_tree[path], path)
        head_document = parse_json_schema(head_tree[path], path)
        violations.update(
            SchemaComparator(base_document, head_document, direction, path).compare()
        )

    for path in TYPESCRIPT_INDEXES:
        if path not in base_tree or path not in head_tree:
            continue
        for removed in exported_types(base_tree[path]) - exported_types(
            head_tree[path]
        ):
            violations.add(
                Violation(
                    "typescript_export_removed",
                    f"{path}#{removed}",
                    "generated TypeScript export was removed or renamed",
                )
            )
    return sorted(violations)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check the stable app-server protocol for backwards-incompatible changes."
    )
    parser.add_argument(
        "--base", required=True, help="Git revision containing the baseline schema."
    )
    head = parser.add_mutually_exclusive_group()
    head.add_argument(
        "--head", default="HEAD", help="Git revision containing the candidate schema."
    )
    head.add_argument(
        "--head-schema-root",
        type=Path,
        help="Generated candidate schema root containing json/ and typescript/.",
    )
    args = parser.parse_args()

    base_tree = load_git_tree(args.base)
    head_tree = (
        load_schema_tree(args.head_schema_root)
        if args.head_schema_root is not None
        else load_git_tree(args.head)
    )
    violations = compare_protocol_trees(base_tree, head_tree)
    if not violations:
        print("No backwards-incompatible stable app-server protocol changes detected.")
        return 0

    print(
        f"Detected {len(violations)} backwards-incompatible app-server protocol change(s):"
    )
    for violation in violations:
        print(f"- [{violation.code}] {violation.path}: {violation.detail}")
    print(
        "\nPreserve the old stable shape, introduce the change additively, or place unstable "
        "surface behind experimentalApi. Repository owners may bypass this required check for "
        "an emergency merge."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
