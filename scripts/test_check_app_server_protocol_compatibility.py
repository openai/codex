import json
import unittest

from check_app_server_protocol_compatibility import compare_protocol_trees


def schema_file(
    *,
    properties=None,
    required=None,
    enum=None,
    one_of=None,
):
    schema = {"type": "object"}
    if properties is not None:
        schema["properties"] = properties
    if required is not None:
        schema["required"] = required
    if enum is not None:
        schema = {"type": "string", "enum": enum}
    if one_of is not None:
        schema = {"oneOf": one_of}
    return json.dumps(schema)


def method(method_name):
    return {
        "type": "object",
        "properties": {
            "method": {"type": "string", "enum": [method_name]},
        },
        "required": ["method"],
    }


class AppServerProtocolCompatibilityTest(unittest.TestCase):
    def test_removed_output_field_is_breaking(self):
        path = "json/v2/PluginReadResponse.json"
        base = {
            path: json.dumps(
                {
                    "definitions": {
                        "AppSummary": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string"},
                                "needsAuth": {"type": "boolean"},
                            },
                            "required": ["id", "needsAuth"],
                        }
                    },
                    "type": "object",
                    "properties": {
                        "app": {"$ref": "#/definitions/AppSummary"},
                    },
                    "required": ["app"],
                },
            )
        }
        head = {
            path: json.dumps(
                {
                    "definitions": {
                        "AppSummary": {
                            "type": "object",
                            "properties": {"id": {"type": "string"}},
                            "required": ["id"],
                        }
                    },
                    "type": "object",
                    "properties": {
                        "app": {"$ref": "#/definitions/AppSummary"},
                    },
                    "required": ["app"],
                },
            )
        }

        violations = compare_protocol_trees(base, head)

        self.assertIn("property_removed", {violation.code for violation in violations})
        self.assertTrue(any("needsAuth" in violation.path for violation in violations))

    def test_new_required_input_field_is_breaking(self):
        path = "json/ClientRequest.json"
        base = {path: schema_file(properties={"method": {"type": "string"}})}
        head = {
            path: schema_file(
                properties={
                    "method": {"type": "string"},
                    "newField": {"type": "string"},
                },
                required=["newField"],
            )
        }

        violations = compare_protocol_trees(base, head)

        self.assertEqual(
            {violation.code for violation in violations}, {"required_changed"}
        )

    def test_optional_input_field_and_enum_value_are_additive(self):
        path = "json/ClientRequest.json"
        base = {
            path: schema_file(properties={"mode": {"type": "string", "enum": ["one"]}})
        }
        head = {
            path: schema_file(
                properties={
                    "mode": {"type": "string", "enum": ["one", "two"]},
                    "optional": {"type": ["string", "null"]},
                }
            )
        }

        self.assertEqual(compare_protocol_trees(base, head), [])

    def test_new_output_enum_value_is_breaking(self):
        path = "json/v2/ExampleResponse.json"
        base = {path: schema_file(enum=["one"])}
        head = {path: schema_file(enum=["one", "two"])}

        violations = compare_protocol_trees(base, head)

        self.assertEqual({violation.code for violation in violations}, {"enum_changed"})

    def test_removing_output_enum_constraint_is_breaking(self):
        path = "json/v2/ExampleResponse.json"
        base = {path: schema_file(enum=["one"])}
        head = {path: json.dumps({"type": "string"})}

        violations = compare_protocol_trees(base, head)

        self.assertEqual({violation.code for violation in violations}, {"enum_changed"})

    def test_required_output_field_becoming_optional_is_breaking(self):
        path = "json/v2/ExampleResponse.json"
        properties = {"value": {"type": "string"}}
        base = {path: schema_file(properties=properties, required=["value"])}
        head = {path: schema_file(properties=properties)}

        violations = compare_protocol_trees(base, head)

        self.assertEqual(
            {violation.code for violation in violations},
            {"required_changed"},
        )

    def test_input_type_narrowing_is_breaking(self):
        path = "json/ClientRequest.json"
        base = {path: schema_file(properties={"value": {"type": ["string", "null"]}})}
        head = {path: schema_file(properties={"value": {"type": "string"}})}

        violations = compare_protocol_trees(base, head)

        self.assertEqual({violation.code for violation in violations}, {"type_changed"})

    def test_new_property_on_closed_output_is_breaking(self):
        path = "json/v2/ExampleResponse.json"
        base_schema = {
            "type": "object",
            "additionalProperties": False,
            "properties": {"old": {"type": "string"}},
        }
        head_schema = {
            **base_schema,
            "properties": {
                **base_schema["properties"],
                "new": {"type": "string"},
            },
        }

        violations = compare_protocol_trees(
            {path: json.dumps(base_schema)},
            {path: json.dumps(head_schema)},
        )

        self.assertEqual(
            {violation.code for violation in violations},
            {"property_added_to_closed_output"},
        )

    def test_new_client_method_is_additive(self):
        path = "json/ClientRequest.json"
        base = {path: schema_file(one_of=[method("thread/read")])}
        head = {
            path: schema_file(one_of=[method("thread/read"), method("thread/archive")])
        }

        self.assertEqual(compare_protocol_trees(base, head), [])

    def test_new_server_notification_is_breaking(self):
        path = "json/ServerNotification.json"
        base = {path: schema_file(one_of=[method("thread/started")])}
        head = {
            path: schema_file(
                one_of=[method("thread/started"), method("thread/replaced")]
            )
        }

        violations = compare_protocol_trees(base, head)

        self.assertEqual(
            {violation.code for violation in violations},
            {"union_variant_changed"},
        )

    def test_removed_typescript_export_is_breaking(self):
        path = "typescript/v2/index.ts"
        base = {path: 'export type { OldName } from "./OldName";\n'}
        head = {path: 'export type { NewName } from "./NewName";\n'}

        violations = compare_protocol_trees(base, head)

        self.assertEqual(
            {violation.code for violation in violations},
            {"typescript_export_removed"},
        )


if __name__ == "__main__":
    unittest.main()
