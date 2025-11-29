package ai.solace.coder.core.tools

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement

/**
 * Builder for tool registry with specs collection.
 *
 * Ported from Rust codex-rs/core/src/tools/spec.rs build_specs()
 */
class ToolRegistryBuilder {
    private val specs = mutableListOf<ConfiguredToolSpec>()
    private val handlers = mutableMapOf<String, ToolHandler>()

    fun pushSpec(spec: ToolSpec) {
        specs.add(ConfiguredToolSpec(spec, supportsParallelToolCalls = false))
    }

    fun pushSpecWithParallelSupport(spec: ToolSpec, supportsParallel: Boolean) {
        specs.add(ConfiguredToolSpec(spec, supportsParallelToolCalls = supportsParallel))
    }

    fun registerHandler(name: String, handler: ToolHandler) {
        handlers[name] = handler
    }

    fun build(): Pair<List<ConfiguredToolSpec>, Map<String, ToolHandler>> {
        return Pair(specs.toList(), handlers.toMap())
    }
}

/**
 * Builds the tool registry builder while collecting tool specs for later serialization.
 *
 * @param config Tool configuration
 * @param mcpTools Optional map of MCP tool names to their definitions
 * @return ToolRegistryBuilder with configured tools
 */
fun buildSpecs(
    config: ToolsConfig,
    mcpTools: Map<String, McpToolDef>? = null
): ToolRegistryBuilder {
    val builder = ToolRegistryBuilder()

    // Shell tools based on configuration
    when (config.shellType) {
        ConfigShellToolType.Default -> {
            builder.pushSpec(createShellTool())
        }
        ConfigShellToolType.Local -> {
            builder.pushSpec(ToolSpec.LocalShell)
        }
        ConfigShellToolType.UnifiedExec -> {
            builder.pushSpec(createExecCommandTool())
            builder.pushSpec(createWriteStdinTool())
        }
        ConfigShellToolType.Disabled -> {
            // Do nothing
        }
        ConfigShellToolType.ShellCommand -> {
            builder.pushSpec(createShellCommandTool())
        }
    }

    // MCP resource tools - always included with parallel support
    builder.pushSpecWithParallelSupport(createListMcpResourcesTool(), true)
    builder.pushSpecWithParallelSupport(createListMcpResourceTemplatesTool(), true)
    builder.pushSpecWithParallelSupport(createReadMcpResourceTool(), true)

    // Plan tool - always included
    builder.pushSpec(PLAN_TOOL)

    // Apply patch tool based on configuration
    config.applyPatchToolType?.let { patchType ->
        when (patchType) {
            ApplyPatchToolType.Freeform -> {
                builder.pushSpec(createApplyPatchFreeformTool())
            }
            ApplyPatchToolType.Function -> {
                builder.pushSpec(createApplyPatchJsonTool())
            }
        }
    }

    // Experimental tools
    if ("grep_files" in config.experimentalSupportedTools) {
        builder.pushSpecWithParallelSupport(createGrepFilesTool(), true)
    }

    if ("read_file" in config.experimentalSupportedTools) {
        builder.pushSpecWithParallelSupport(createReadFileTool(), true)
    }

    if ("list_dir" in config.experimentalSupportedTools) {
        builder.pushSpecWithParallelSupport(createListDirTool(), true)
    }

    if ("test_sync_tool" in config.experimentalSupportedTools) {
        builder.pushSpecWithParallelSupport(createTestSyncTool(), true)
    }

    // Web search tool
    if (config.webSearchRequest) {
        builder.pushSpec(ToolSpec.WebSearch)
    }

    // View image tool
    if (config.includeViewImageTool) {
        builder.pushSpecWithParallelSupport(createViewImageTool(), true)
    }

    // MCP tools - sorted by name for deterministic ordering
    mcpTools?.let { tools ->
        val sortedEntries = tools.entries.sortedBy { it.key }
        for ((name, tool) in sortedEntries) {
            try {
                val convertedTool = mcpToolToOpenAiTool(name, tool)
                builder.pushSpec(ToolSpec.Function(convertedTool))
            } catch (e: Exception) {
                // Log error and skip this tool
                println("Failed to convert MCP tool '$name': ${e.message}")
            }
        }
    }

    return builder
}

/**
 * MCP tool definition.
 */
data class McpToolDef(
    val name: String,
    val description: String?,
    val inputSchema: McpToolInputSchema
)

/**
 * MCP tool input schema.
 */
data class McpToolInputSchema(
    val type: String,
    val properties: JsonElement?,
    val required: List<String>?
)

/**
 * Convert an MCP tool to an OpenAI-compatible ResponsesApiTool.
 */
fun mcpToolToOpenAiTool(
    fullyQualifiedName: String,
    tool: McpToolDef
): ResponsesApiTool {
    // OpenAI models mandate the "properties" field in the schema.
    // Insert an empty object for "properties" if not present.
    val properties = tool.inputSchema.properties
        ?: kotlinx.serialization.json.JsonObject(emptyMap())

    // Sanitize the schema
    val sanitizedSchema = sanitizeJsonSchema(
        kotlinx.serialization.json.buildJsonObject {
            put("type", kotlinx.serialization.json.JsonPrimitive(tool.inputSchema.type))
            put("properties", properties)
            tool.inputSchema.required?.let { required ->
                put("required", kotlinx.serialization.json.JsonArray(
                    required.map { kotlinx.serialization.json.JsonPrimitive(it) }
                ))
            }
        }
    )

    // Parse sanitized schema into JsonSchema
    val inputSchema = parseJsonSchemaFromElement(sanitizedSchema)

    return ResponsesApiTool(
        name = fullyQualifiedName,
        description = tool.description ?: "",
        strict = false,
        parameters = inputSchema
    )
}

/**
 * Parse a JsonElement into our JsonSchema type.
 */
private fun parseJsonSchemaFromElement(element: JsonElement): JsonSchema {
    val obj = element as? kotlinx.serialization.json.JsonObject
        ?: return JsonSchema.StringType()

    val type = obj["type"]?.let {
        (it as? kotlinx.serialization.json.JsonPrimitive)?.content
    } ?: "string"

    return when (type) {
        "boolean" -> JsonSchema.Boolean(
            description = obj["description"]?.let {
                (it as? kotlinx.serialization.json.JsonPrimitive)?.content
            }
        )
        "string" -> JsonSchema.StringType(
            description = obj["description"]?.let {
                (it as? kotlinx.serialization.json.JsonPrimitive)?.content
            }
        )
        "number", "integer" -> JsonSchema.Number(
            description = obj["description"]?.let {
                (it as? kotlinx.serialization.json.JsonPrimitive)?.content
            }
        )
        "array" -> {
            val items = obj["items"]?.let { parseJsonSchemaFromElement(it) }
                ?: JsonSchema.StringType()
            JsonSchema.Array(
                items = items,
                description = obj["description"]?.let {
                    (it as? kotlinx.serialization.json.JsonPrimitive)?.content
                }
            )
        }
        "object" -> {
            val propsObj = obj["properties"] as? kotlinx.serialization.json.JsonObject
            val properties = propsObj?.mapValues { (_, v) ->
                parseJsonSchemaFromElement(v)
            } ?: emptyMap()

            val required = obj["required"]?.let { req ->
                (req as? kotlinx.serialization.json.JsonArray)?.mapNotNull {
                    (it as? kotlinx.serialization.json.JsonPrimitive)?.content
                }
            }

            val additionalProperties = obj["additionalProperties"]?.let { ap ->
                when (ap) {
                    is kotlinx.serialization.json.JsonPrimitive -> {
                        if (ap.content == "false" || ap.content == "true") {
                            AdditionalProperties.fromBoolean(ap.content == "true")
                        } else {
                            null
                        }
                    }
                    is kotlinx.serialization.json.JsonObject -> {
                        AdditionalProperties.fromSchema(parseJsonSchemaFromElement(ap))
                    }
                    else -> null
                }
            }

            JsonSchema.Object(
                properties = properties,
                required = required,
                additionalProperties = additionalProperties
            )
        }
        else -> JsonSchema.StringType()
    }
}

// ============================================================================
// Plan tool constant
// ============================================================================

/**
 * The plan tool specification.
 */
val PLAN_TOOL: ToolSpec = run {
    val stepProperties = mapOf(
        "description" to JsonSchema.StringType(
            description = "Description of this step"
        ),
        "status" to JsonSchema.StringType(
            description = "Status of this step: pending, in_progress, or completed"
        )
    )

    val properties = mapOf(
        "plan" to JsonSchema.Array(
            items = JsonSchema.Object(
                properties = stepProperties,
                required = listOf("description", "status"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            ),
            description = "List of steps in the plan"
        )
    )

    ToolSpec.Function(
        ResponsesApiTool(
            name = "update_plan",
            description = "Update the current task plan. Use this to track progress on multi-step tasks.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("plan"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

// ============================================================================
// Apply patch tools
// ============================================================================

fun createApplyPatchFreeformTool(): ToolSpec {
    val properties = mapOf(
        "input" to JsonSchema.StringType(
            description = "The patch content in unified diff format"
        )
    )

    return ToolSpec.Freeform(
        FreeformTool(
            name = "apply_patch",
            description = "Apply a patch to modify files. The input should be a unified diff.",
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("input"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createApplyPatchJsonTool(): ToolSpec {
    val properties = mapOf(
        "input" to JsonSchema.StringType(
            description = "The patch content in unified diff format"
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "apply_patch",
            description = "Apply a patch to modify files. The input should be a unified diff.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("input"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

// ============================================================================
// JSON conversion utilities
// ============================================================================

/**
 * Convert tool specs to JSON values compatible with the Responses API.
 */
fun createToolsJsonForResponsesApi(tools: List<ToolSpec>): List<JsonElement> {
    val json = Json { encodeDefaults = true }
    return tools.map { tool ->
        json.encodeToJsonElement(ToolSpec.serializer(), tool)
    }
}

/**
 * Convert tool specs to JSON values compatible with the Chat Completions API.
 */
fun createToolsJsonForChatCompletionsApi(tools: List<ToolSpec>): List<JsonElement> {
    val responsesApiTools = createToolsJsonForResponsesApi(tools)
    return responsesApiTools.mapNotNull { tool ->
        val obj = tool as? kotlinx.serialization.json.JsonObject ?: return@mapNotNull null
        val type = obj["type"]?.let {
            (it as? kotlinx.serialization.json.JsonPrimitive)?.content
        }
        if (type != "function") return@mapNotNull null

        // Reformat for chat completions API
        val mutableMap = obj.toMutableMap()
        mutableMap.remove("type")
        kotlinx.serialization.json.buildJsonObject {
            put("type", kotlinx.serialization.json.JsonPrimitive("function"))
            put("function", kotlinx.serialization.json.JsonObject(mutableMap))
        }
    }
}
