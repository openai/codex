import { sanitizeToolName } from "./sanitizeToolName";
import { type Tool as MCPTool } from "@modelcontextprotocol/sdk/types.js";
import { type FunctionTool as OpenAIFunctionTool } from "openai/resources/responses/responses.mjs";

function removeUnsupportedKeysFromJsonSchemaParameters(
  parameters: Record<string, unknown>,
  keys: Array<string>,
): Record<string, unknown> {
  // Create a deep copy of the parameters to avoid modifying the original
  const paramsCopy = JSON.parse(JSON.stringify(parameters));

  // Remove specified keys from the top level
  for (const key of keys) {
    delete paramsCopy[key];
  }

  // If there are properties, recursively process them
  if (paramsCopy.properties && typeof paramsCopy.properties === "object") {
    for (const propName in paramsCopy.properties) {
      if (
        Object.prototype.hasOwnProperty.call(paramsCopy.properties, propName)
      ) {
        const property = paramsCopy.properties[propName];
        if (property && typeof property === "object") {
          // Remove specified keys from each property
          for (const key of keys) {
            delete property[key];
          }
        }
      }
    }
  }

  return paramsCopy;
}

const unsupportedKeys = ["default", "minimum", "maximum"];

// Recursively remove unsupported keys from JSON schema
function removeUnsupportedKeysFromJsonSchemaParametersRecursive(
  parameters: Record<string, unknown>,
): Record<string, unknown> {
  return removeUnsupportedKeysFromJsonSchemaParameters(
    parameters,
    unsupportedKeys,
  );
}

/* 
  Error: 400 Invalid schema for function 'xx': In context=(), 'additionalProperties' is required to be supplied and to be false.
  // Recursively set "additionalProperties" to false, where "type" is "array, "object" or "string"
*/

// Recursively set additionalProperties: false for all object schemas
function setAdditionalPropertiesFalse(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  const schemaCopy = JSON.parse(JSON.stringify(schema));

  // Set additionalProperties: false for the current schema if it's an object type
  if (schemaCopy.type === "object" || !schemaCopy.type) {
    schemaCopy.additionalProperties = false;
  }

  // Process properties recursively
  if (schemaCopy.properties && typeof schemaCopy.properties === "object") {
    for (const propName in schemaCopy.properties) {
      if (
        Object.prototype.hasOwnProperty.call(schemaCopy.properties, propName)
      ) {
        const property = schemaCopy.properties[propName];
        if (property && typeof property === "object") {
          schemaCopy.properties[propName] = setAdditionalPropertiesFalse(
            property as Record<string, unknown>,
          );
        }
      }
    }
  }

  // Handle items in arrays
  if (
    schemaCopy.type === "array" &&
    schemaCopy.items &&
    typeof schemaCopy.items === "object"
  ) {
    schemaCopy.items = setAdditionalPropertiesFalse(
      schemaCopy.items as Record<string, unknown>,
    );
  }

  return schemaCopy;
}

// Helper function 1 for ensureBaseSchema: Ensures 'properties' field is a valid object.
function processSchemaProperties(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  const currentProperties = schema["properties"];
  const newProperties =
    currentProperties &&
    typeof currentProperties === "object" &&
    !Array.isArray(currentProperties)
      ? currentProperties
      : {};
  return {
    ...schema,
    properties: newProperties as Record<string, unknown>,
  };
}

// Helper function 2 for ensureBaseSchema: Determines the 'required' array.
// Reason: OpenAI expects all properties to be required, so we need to include all property keys in the required array
function determineSchemaRequired(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  // Assumes schema.properties is correctly set by a previous step (e.g., processSchemaProperties)
  const properties = (schema["properties"] as Record<string, unknown>) || {};

  // Use all property keys for the required array
  const finalRequired = Object.keys(properties);

  return {
    ...schema,
    required: finalRequired,
  };
}

/*
Helper function to fix required fields by ensuring they only include valid properties
and excluding fields of type "object"

Reason: Really weird behaviour with OpenAI's API, it expects all the properties to be required, but if it's supplied with a property of type "object", it throws an error.

  Example Error:
  sanitizedName mcp-atlassian__MCP__TOOL__jira_create_issue
  inputSchema {
    type: 'object',
    properties: {
      project_key: {
        description: "The JIRA project key (e.g. 'PROJ', 'DEV', 'SUPPORT'). This is the prefix of issue keys in your project. Never assume what it might be, always ask the user.",
        title: 'Project Key',
        type: 'string'
      },
      summary: {
        description: 'Summary/title of the issue',
        title: 'Summary',
        type: 'string'
      },
      issue_type: {
        description: "Issue type (e.g. 'Task', 'Bug', 'Story', 'Epic', 'Subtask'). The available types depend on your project configuration. For subtasks, use 'Subtask' (not 'Sub-task') and include parent in additional_fields.",
        title: 'Issue Type',
        type: 'string'
      },
      assignee: {
        description: "(Optional) Assignee's user identifier (string): Email, display name, or account ID (e.g., 'user@example.com', 'John Doe', 'accountid:...')",
        title: 'Assignee',
        type: 'string'
      },
      description: {
        description: 'Issue description',
        title: 'Description',
        type: 'string'
      },
      components: {
        description: "(Optional) Comma-separated list of component names to assign (e.g., 'Frontend,API')",
        title: 'Components',
        type: 'string'
      },
      additional_fields: {
        description: '(Optional) Dictionary of additional fields to set. Examples:\n' +
          "- Set priority: {'priority': {'name': 'High'}}\n" +
          "- Add labels: {'labels': ['frontend', 'urgent']}\n" +
          "- Link to parent (for any issue type): {'parent': 'PROJ-123'}\n" +
          "- Set Fix Version/s: {'fixVersions': [{'id': '10020'}]}\n" +
          "- Custom fields: {'customfield_10010': 'value'}",
        title: 'Additional Fields',
        type: 'object',
        additionalProperties: false
      }
    },
    required: [
      'project_key',
      'summary',
      'issue_type',
      'assignee',
      'description',
      'components',
      'additional_fields'
    ],
    additionalProperties: false,
    '$schema': 'http://json-schema.org/draft-07/schema#'
  }
 system
    ⚠️  OpenAI rejected the request (request ID: req_b5de6f4ca29a335dc1cb99298b88b4b4). Error details: Status: 400,
    Code: invalid_function_parameters, Type: invalid_request_error, Message: 400 Invalid schema for function
    'mcp-atlassian__MCP__TOOL__jira_create_issue': In context=(), 'required' is required to be supplied and to be an
    array including every key in properties. Extra required key 'additional_fields' supplied.. Please verify your
    settings and try again.
*/
function fixRequiredFields(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  const schemaCopy = { ...schema };

  if (Array.isArray(schemaCopy["required"]) && schemaCopy["properties"]) {
    const properties = schemaCopy["properties"] as Record<
      string,
      Record<string, unknown>
    >;

    schemaCopy["required"] = (schemaCopy["required"] as Array<unknown>).filter(
      (key: unknown) => {
        if (typeof key !== "string") {
          return false;
        }

        if (!Object.prototype.hasOwnProperty.call(properties, key)) {
          return false;
        }

        // Get the property definition
        const property = properties[key];

        // Exclude properties of type "object" from required
        return !(property && property["type"] === "object");
      },
    );
  }

  return schemaCopy;
}

// Helper function 3 for ensureBaseSchema: Applies the base schema attributes.
function applySchemaBaseStructure(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  // The 'properties' and 'required' fields from the input 'schema' (output of previous pipe stages)
  // are preserved because '...schema' is spread first.
  return {
    ...schema,
    type: "object",
    $schema: "http://json-schema.org/draft-07/schema#",
    additionalProperties: false,
  };
}

const fixInputSchema = pipe(
  removeUnsupportedKeysFromJsonSchemaParametersRecursive,
  setAdditionalPropertiesFalse,
  processSchemaProperties,
  determineSchemaRequired,
  fixRequiredFields,
  applySchemaBaseStructure,
);

// function piping
function pipe<T>(...fns: Array<(arg: T) => T>): (arg: T) => T {
  return (arg: T): T => {
    return fns.reduce((acc, fn) => fn(acc), arg);
  };
}

export function mcpToOpenaiTools(
  tools: Array<MCPTool>,
): Array<OpenAIFunctionTool> {
  return tools.map((tool: MCPTool): OpenAIFunctionTool => {
    const inputSchema = fixInputSchema(tool.inputSchema);

    // Sanitize the tool name to ensure it complies with OpenAI's pattern
    const sanitizedName = sanitizeToolName(tool.name);
    return {
      type: "function",
      name: sanitizedName,
      parameters: inputSchema,
      strict: true,
      description: tool.description,
    };
  });
}
