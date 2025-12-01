// port-lint: source core/src/tools/spec.rs
package ai.solace.coder.core.tools

import ai.solace.coder.client.common.tools.ResponsesApiTool
import ai.solace.coder.client.common.tools.ToolSpec
import ai.solace.coder.core.features.Feature
import ai.solace.coder.core.features.Features
import ai.solace.coder.core.model_family.ModelFamily
import ai.solace.coder.core.tools.handlers.PLAN_TOOL
import ai.solace.coder.core.tools.handlers.apply_patch.ApplyPatchToolType
import ai.solace.coder.core.tools.handlers.apply_patch.createApplyPatchFreeformTool
import ai.solace.coder.core.tools.handlers.apply_patch.createApplyPatchJsonTool
import ai.solace.coder.core.tools.handlers.ApplyPatchHandler
import ai.solace.coder.core.tools.handlers.GrepFilesHandler
import ai.solace.coder.core.tools.handlers.ListDirHandler
import ai.solace.coder.core.tools.handlers.McpHandler
import ai.solace.coder.core.tools.handlers.McpResourceHandler
import ai.solace.coder.core.tools.handlers.PlanHandler
import ai.solace.coder.core.tools.handlers.ReadFileHandler
import ai.solace.coder.core.tools.handlers.ShellCommandHandler
import ai.solace.coder.core.tools.handlers.ShellHandler
import ai.solace.coder.core.tools.handlers.TestSyncHandler
import ai.solace.coder.core.tools.handlers.UnifiedExecHandler
import ai.solace.coder.core.tools.handlers.ViewImageHandler
import ai.solace.coder.protocol.McpTool
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.contentOrNull

enum class ConfigShellToolType {
    Default,
    Local,
    UnifiedExec,
    Disabled,
    ShellCommand
}

data class ToolsConfig(
    val shellType: ConfigShellToolType,
    val applyPatchToolType: ApplyPatchToolType?,
    val webSearchRequest: Boolean,
    val includeViewImageTool: Boolean,
    val experimentalSupportedTools: List<String>
) {
    companion object {
        fun new(params: ToolsConfigParams): ToolsConfig {
            val modelFamily = params.modelFamily
            val features = params.features

            val includeApplyPatchTool = features.enabled(Feature.ApplyPatchFreeform)
            val includeWebSearchRequest = features.enabled(Feature.WebSearchRequest)
            val includeViewImageTool = features.enabled(Feature.ViewImageTool)

            val shellType = if (!features.enabled(Feature.ShellTool)) {
                ConfigShellToolType.Disabled
            } else if (features.enabled(Feature.UnifiedExec)) {
                ConfigShellToolType.UnifiedExec
            } else {
                modelFamily.shellType
            }

            val applyPatchToolType = when (modelFamily.applyPatchToolType) {
                ApplyPatchToolType.Freeform -> ApplyPatchToolType.Freeform
                ApplyPatchToolType.Function -> ApplyPatchToolType.Function
                null -> if (includeApplyPatchTool) ApplyPatchToolType.Freeform else null
            }

            return ToolsConfig(
                shellType = shellType,
                applyPatchToolType = applyPatchToolType,
                webSearchRequest = includeWebSearchRequest,
                includeViewImageTool = includeViewImageTool,
                experimentalSupportedTools = modelFamily.experimentalSupportedTools
            )
        }
    }
}

data class ToolsConfigParams(
    val modelFamily: ModelFamily,
    val features: Features
)

@Serializable
sealed class JsonSchema {
    @Serializable
    data class Boolean(val description: String? = null) : JsonSchema()
    
    @Serializable
    data class String(val description: String? = null) : JsonSchema()
    
    @Serializable
    data class Number(val description: String? = null) : JsonSchema()
    
    @Serializable
    data class Array(
        val items: JsonSchema,
        val description: kotlin.String? = null
    ) : JsonSchema()
    
    @Serializable
    data class Object(
        val properties: Map<kotlin.String, JsonSchema>,
        val required: List<kotlin.String>? = null,
        val additionalProperties: AdditionalProperties? = null
    ) : JsonSchema()
}

@Serializable
sealed class AdditionalProperties {
    @Serializable
    data class Boolean(val value: kotlin.Boolean) : AdditionalProperties()
    
    @Serializable
    data class Schema(val schema: JsonSchema) : AdditionalProperties()
    
    companion object {
        fun from(b: kotlin.Boolean) = Boolean(b)
        fun from(s: JsonSchema) = Schema(s)
    }
}

fun createExecCommandTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["cmd"] = JsonSchema.String(description = "Shell command to execute.")
    properties["workdir"] = JsonSchema.String(description = "Optional working directory to run the command in; defaults to the turn cwd.")
    properties["shell"] = JsonSchema.String(description = "Shell binary to launch. Defaults to /bin/bash.")
    properties["login"] = JsonSchema.Boolean(description = "Whether to run the shell with -l/-i semantics. Defaults to true.")
    properties["yield_time_ms"] = JsonSchema.Number(description = "How long to wait (in milliseconds) for output before yielding.")
    properties["max_output_tokens"] = JsonSchema.Number(description = "Maximum number of tokens to return. Excess output will be truncated.")
    properties["with_escalated_permissions"] = JsonSchema.Boolean(description = "Whether to request escalated permissions. Set to true if command needs to be run without sandbox restrictions")
    properties["justification"] = JsonSchema.String(description = "Only set if with_escalated_permissions is true. 1-sentence explanation of why we want to run this command.")

    return ToolSpec.Function(ResponsesApiTool(
        name = "exec_command",
        description = "Runs a command in a PTY, returning output or a session ID for ongoing interaction.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = listOf("cmd"),
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createWriteStdinTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["session_id"] = JsonSchema.Number(description = "Identifier of the running unified exec session.")
    properties["chars"] = JsonSchema.String(description = "Bytes to write to stdin (may be empty to poll).")
    properties["yield_time_ms"] = JsonSchema.Number(description = "How long to wait (in milliseconds) for output before yielding.")
    properties["max_output_tokens"] = JsonSchema.Number(description = "Maximum number of tokens to return. Excess output will be truncated.")

    return ToolSpec.Function(ResponsesApiTool(
        name = "write_stdin",
        description = "Writes characters to an existing unified exec session and returns recent output.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = listOf("session_id"),
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createShellTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["command"] = JsonSchema.Array(
        items = JsonSchema.String(),
        description = "The command to execute"
    )
    properties["workdir"] = JsonSchema.String(description = "The working directory to execute the command in")
    properties["timeout_ms"] = JsonSchema.Number(description = "The timeout for the command in milliseconds")
    properties["with_escalated_permissions"] = JsonSchema.Boolean(description = "Whether to request escalated permissions. Set to true if command needs to be run without sandbox restrictions")
    properties["justification"] = JsonSchema.String(description = "Only set if with_escalated_permissions is true. 1-sentence explanation of why we want to run this command.")

    // Assuming non-Windows for now as Kotlin Native target is likely macOS/Linux based on context
    val description = """Runs a shell command and returns its output.
- The arguments to `shell` will be passed to execvp(). Most terminal commands should be prefixed with ["bash", "-lc"].
- Always set the `workdir` param when using the shell function. Do not use `cd` unless absolutely necessary."""

    return ToolSpec.Function(ResponsesApiTool(
        name = "shell",
        description = description,
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = listOf("command"),
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createShellCommandTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["command"] = JsonSchema.String(description = "The shell script to execute in the user's default shell")
    properties["workdir"] = JsonSchema.String(description = "The working directory to execute the command in")
    properties["timeout_ms"] = JsonSchema.Number(description = "The timeout for the command in milliseconds")
    properties["with_escalated_permissions"] = JsonSchema.Boolean(description = "Whether to request escalated permissions. Set to true if command needs to be run without sandbox restrictions")
    properties["justification"] = JsonSchema.String(description = "Only set if with_escalated_permissions is true. 1-sentence explanation of why we want to run this command.")

    val description = """Runs a shell command and returns its output.
- Always set the `workdir` param when using the shell_command function. Do not use `cd` unless absolutely necessary."""

    return ToolSpec.Function(ResponsesApiTool(
        name = "shell_command",
        description = description,
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = listOf("command"),
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createViewImageTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["path"] = JsonSchema.String(description = "Local filesystem path to an image file")

    return ToolSpec.Function(ResponsesApiTool(
        name = "view_image",
        description = "Attach a local image (by filesystem path) to the conversation context for this turn.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = listOf("path"),
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createTestSyncTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["sleep_before_ms"] = JsonSchema.Number(description = "Optional delay in milliseconds before any other action")
    properties["sleep_after_ms"] = JsonSchema.Number(description = "Optional delay in milliseconds after completing the barrier")

    val barrierProperties = mutableMapOf<kotlin.String, JsonSchema>()
    barrierProperties["id"] = JsonSchema.String(description = "Identifier shared by concurrent calls that should rendezvous")
    barrierProperties["participants"] = JsonSchema.Number(description = "Number of tool calls that must arrive before the barrier opens")
    barrierProperties["timeout_ms"] = JsonSchema.Number(description = "Maximum time in milliseconds to wait at the barrier")

    properties["barrier"] = JsonSchema.Object(
        properties = barrierProperties,
        required = listOf("id", "participants"),
        additionalProperties = AdditionalProperties.from(false)
    )

    return ToolSpec.Function(ResponsesApiTool(
        name = "test_sync_tool",
        description = "Internal synchronization helper used by Codex integration tests.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = null,
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createGrepFilesTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["pattern"] = JsonSchema.String(description = "Regular expression pattern to search for.")
    properties["include"] = JsonSchema.String(description = "Optional glob that limits which files are searched (e.g. \"*.rs\" or \"*.{ts,tsx}\").")
    properties["path"] = JsonSchema.String(description = "Directory or file path to search. Defaults to the session's working directory.")
    properties["limit"] = JsonSchema.Number(description = "Maximum number of file paths to return (defaults to 100).")

    return ToolSpec.Function(ResponsesApiTool(
        name = "grep_files",
        description = "Finds files whose contents match the pattern and lists them by modification time.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = listOf("pattern"),
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createReadFileTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["file_path"] = JsonSchema.String(description = "Absolute path to the file")
    properties["offset"] = JsonSchema.Number(description = "The line number to start reading from. Must be 1 or greater.")
    properties["limit"] = JsonSchema.Number(description = "The maximum number of lines to return.")
    properties["mode"] = JsonSchema.String(description = "Optional mode selector: \"slice\" for simple ranges (default) or \"indentation\" to expand around an anchor line.")

    val indentationProperties = mutableMapOf<kotlin.String, JsonSchema>()
    indentationProperties["anchor_line"] = JsonSchema.Number(description = "Anchor line to center the indentation lookup on (defaults to offset).")
    indentationProperties["max_levels"] = JsonSchema.Number(description = "How many parent indentation levels (smaller indents) to include.")
    indentationProperties["include_siblings"] = JsonSchema.Boolean(description = "When true, include additional blocks that share the anchor indentation.")
    indentationProperties["include_header"] = JsonSchema.Boolean(description = "Include doc comments or attributes directly above the selected block.")
    indentationProperties["max_lines"] = JsonSchema.Number(description = "Hard cap on the number of lines returned when using indentation mode.")

    properties["indentation"] = JsonSchema.Object(
        properties = indentationProperties,
        required = null,
        additionalProperties = AdditionalProperties.from(false)
    )

    return ToolSpec.Function(ResponsesApiTool(
        name = "read_file",
        description = "Reads a local file with 1-indexed line numbers, supporting slice and indentation-aware block modes.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = listOf("file_path"),
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createListDirTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["dir_path"] = JsonSchema.String(description = "Absolute path to the directory to list.")
    properties["offset"] = JsonSchema.Number(description = "The entry number to start listing from. Must be 1 or greater.")
    properties["limit"] = JsonSchema.Number(description = "The maximum number of entries to return.")
    properties["depth"] = JsonSchema.Number(description = "The maximum directory depth to traverse. Must be 1 or greater.")

    return ToolSpec.Function(ResponsesApiTool(
        name = "list_dir",
        description = "Lists entries in a local directory with 1-indexed entry numbers and simple type labels.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = listOf("dir_path"),
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createListMcpResourcesTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["server"] = JsonSchema.String(description = "Optional MCP server name. When omitted, lists resources from every configured server.")
    properties["cursor"] = JsonSchema.String(description = "Opaque cursor returned by a previous list_mcp_resources call for the same server.")

    return ToolSpec.Function(ResponsesApiTool(
        name = "list_mcp_resources",
        description = "Lists resources provided by MCP servers. Resources allow servers to share data that provides context to language models, such as files, database schemas, or application-specific information. Prefer resources over web search when possible.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = null,
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createListMcpResourceTemplatesTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["server"] = JsonSchema.String(description = "Optional MCP server name. When omitted, lists resource templates from all configured servers.")
    properties["cursor"] = JsonSchema.String(description = "Opaque cursor returned by a previous list_mcp_resource_templates call for the same server.")

    return ToolSpec.Function(ResponsesApiTool(
        name = "list_mcp_resource_templates",
        description = "Lists resource templates provided by MCP servers. Parameterized resource templates allow servers to share data that takes parameters and provides context to language models, such as files, database schemas, or application-specific information. Prefer resource templates over web search when possible.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = null,
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun createReadMcpResourceTool(): ToolSpec {
    val properties = mutableMapOf<kotlin.String, JsonSchema>()
    properties["server"] = JsonSchema.String(description = "MCP server name exactly as configured. Must match the 'server' field returned by list_mcp_resources.")
    properties["uri"] = JsonSchema.String(description = "Resource URI to read. Must be one of the URIs returned by list_mcp_resources.")

    return ToolSpec.Function(ResponsesApiTool(
        name = "read_mcp_resource",
        description = "Read a specific resource from an MCP server given the server name and resource URI.",
        strict = false,
        parameters = JsonSchema.Object(
            properties = properties,
            required = listOf("server", "uri"),
            additionalProperties = AdditionalProperties.from(false)
        )
    ))
}

fun buildSpecs(
    config: ToolsConfig,
    mcpTools: Map<String, McpTool>?
): ToolRegistryBuilder {
    val builder = ToolRegistryBuilder()

    val shellHandler = ShellHandler()
    val unifiedExecHandler = UnifiedExecHandler()
    val planHandler = PlanHandler()
    val applyPatchHandler = ApplyPatchHandler()
    val viewImageHandler = ViewImageHandler()
    val mcpHandler = McpHandler()
    val mcpResourceHandler = McpResourceHandler()
    val shellCommandHandler = ShellCommandHandler()

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
            builder.registerHandler("exec_command", unifiedExecHandler)
            builder.registerHandler("write_stdin", unifiedExecHandler)
        }
        ConfigShellToolType.Disabled -> {
            // Do nothing.
        }
        ConfigShellToolType.ShellCommand -> {
            builder.pushSpec(createShellCommandTool())
        }
    }

    if (config.shellType != ConfigShellToolType.Disabled) {
        // Always register shell aliases so older prompts remain compatible.
        builder.registerHandler("shell", shellHandler)
        builder.registerHandler("container.exec", shellHandler)
        builder.registerHandler("local_shell", shellHandler)
        builder.registerHandler("shell_command", shellCommandHandler)
    }

    builder.pushSpecWithParallelSupport(createListMcpResourcesTool(), true)
    builder.pushSpecWithParallelSupport(createListMcpResourceTemplatesTool(), true)
    builder.pushSpecWithParallelSupport(createReadMcpResourceTool(), true)
    builder.registerHandler("list_mcp_resources", mcpResourceHandler)
    builder.registerHandler("list_mcp_resource_templates", mcpResourceHandler)
    builder.registerHandler("read_mcp_resource", mcpResourceHandler)

    builder.pushSpec(PLAN_TOOL)
    builder.registerHandler("update_plan", planHandler)

    if (config.applyPatchToolType != null) {
        when (config.applyPatchToolType) {
            ApplyPatchToolType.Freeform -> {
                builder.pushSpec(createApplyPatchFreeformTool())
            }
            ApplyPatchToolType.Function -> {
                builder.pushSpec(createApplyPatchJsonTool())
            }
        }
        builder.registerHandler("apply_patch", applyPatchHandler)
    }

    if (config.experimentalSupportedTools.contains("grep_files")) {
        val grepFilesHandler = GrepFilesHandler()
        builder.pushSpecWithParallelSupport(createGrepFilesTool(), true)
        builder.registerHandler("grep_files", grepFilesHandler)
    }

    if (config.experimentalSupportedTools.contains("read_file")) {
        val readFileHandler = ReadFileHandler()
        builder.pushSpecWithParallelSupport(createReadFileTool(), true)
        builder.registerHandler("read_file", readFileHandler)
    }

    if (config.experimentalSupportedTools.contains("list_dir")) {
        val listDirHandler = ListDirHandler()
        builder.pushSpecWithParallelSupport(createListDirTool(), true)
        builder.registerHandler("list_dir", listDirHandler)
    }

    if (config.experimentalSupportedTools.contains("test_sync_tool")) {
        val testSyncHandler = TestSyncHandler()
        builder.pushSpecWithParallelSupport(createTestSyncTool(), true)
        builder.registerHandler("test_sync_tool", testSyncHandler)
    }

    if (config.webSearchRequest) {
        builder.pushSpec(ToolSpec.WebSearch)
    }

    if (config.includeViewImageTool) {
        builder.pushSpecWithParallelSupport(createViewImageTool(), true)
        builder.registerHandler("view_image", viewImageHandler)
    }

    if (mcpTools != null) {
        val entries = mcpTools.entries.sortedBy { it.key }
        for ((name, tool) in entries) {
            try {
                val convertedTool = mcpToolToOpenAiTool(name, tool)
                builder.pushSpec(ToolSpec.Function(convertedTool))
                builder.registerHandler(name, mcpHandler)
            } catch (e: Exception) {
                // tracing::error!("Failed to convert {name:?} MCP tool to OpenAI tool: {e:?}");
                println("Failed to convert $name MCP tool to OpenAI tool: $e")
            }
        }
    }

    return builder
}

fun mcpToolToOpenAiTool(
    fullyQualifiedName: String,
    tool: McpTool
): ResponsesApiTool {
    val description = tool.description ?: ""
    var inputSchema = tool.inputSchema

    // OpenAI models mandate the "properties" field in the schema.
    // We'll handle this by ensuring the schema has properties.
    // Note: This logic is simplified compared to Rust's direct JSON manipulation
    // because we are working with typed objects or need to parse/modify JSON.
    // Assuming McpTool.inputSchema is a JsonElement or similar.
    
    // TODO: Implement full schema sanitization and property injection logic
    // similar to Rust's sanitize_json_schema.
    // For now, we assume inputSchema can be mapped to JsonSchema.
    
    // This part requires a proper JSON to JsonSchema converter which is complex.
    // I will use a placeholder or simplified conversion if possible.
    
    val parameters = convertJsonElementToJsonSchema(inputSchema.toJsonElement())

    return ResponsesApiTool(
        name = fullyQualifiedName,
        description = description,
        strict = false,
        parameters = parameters
    )
}

// Placeholder for converting generic JSON to our JsonSchema type
fun convertJsonElementToJsonSchema(element: JsonElement): JsonSchema {
    // Implementation needed
    return JsonSchema.Object(emptyMap()) // Stub
}
