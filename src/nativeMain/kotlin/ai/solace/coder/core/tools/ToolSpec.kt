package ai.solace.coder.core.tools

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

/**
 * Shell tool type configuration.
 *
 * Ported from Rust codex-rs/core/src/tools/spec.rs
 */
enum class ConfigShellToolType {
    Default,
    Local,
    UnifiedExec,
    /** Do not include a shell tool by default. Useful when using Codex
     * with tools provided exclusively by MCP servers. */
    Disabled,
    /** Takes a command as a single string to be run in the user's default shell. */
    ShellCommand
}

/**
 * Apply patch tool type.
 */
enum class ApplyPatchToolType {
    Freeform,
    Function
}

/**
 * Configuration for the tools subsystem.
 */
data class ToolsConfig(
    val shellType: ConfigShellToolType,
    val applyPatchToolType: ApplyPatchToolType?,
    val webSearchRequest: Boolean,
    val includeViewImageTool: Boolean,
    val experimentalSupportedTools: List<String>
)

/**
 * Generic JSON Schema subset needed for our tool definitions.
 *
 * Ported from Rust codex-rs/core/src/tools/spec.rs
 */
@Serializable
sealed class JsonSchema {
    @Serializable
    @SerialName("boolean")
    data class Boolean(
        val description: String? = null
    ) : JsonSchema()

    @Serializable
    @SerialName("string")
    data class StringType(
        val description: String? = null
    ) : JsonSchema()

    @Serializable
    @SerialName("number")
    data class Number(
        val description: String? = null
    ) : JsonSchema()

    @Serializable
    @SerialName("array")
    data class Array(
        val items: JsonSchema,
        val description: String? = null
    ) : JsonSchema()

    @Serializable
    @SerialName("object")
    data class Object(
        val properties: Map<String, JsonSchema>,
        val required: List<String>? = null,
        @SerialName("additionalProperties")
        val additionalProperties: AdditionalProperties? = null
    ) : JsonSchema()
}

/**
 * Whether additional properties are allowed, and if so, any required schema.
 */
@Serializable
sealed class AdditionalProperties {
    @Serializable
    data class BooleanValue(val value: kotlin.Boolean) : AdditionalProperties()

    @Serializable
    data class Schema(val schema: JsonSchema) : AdditionalProperties()

    companion object {
        fun fromBoolean(b: kotlin.Boolean): AdditionalProperties = BooleanValue(b)
        fun fromSchema(s: JsonSchema): AdditionalProperties = Schema(s)
    }
}

/**
 * Tool specification for the Responses API.
 */
@Serializable
data class ResponsesApiTool(
    val name: String,
    val description: String,
    val strict: kotlin.Boolean = false,
    val parameters: JsonSchema
)

/**
 * Freeform tool specification.
 */
@Serializable
data class FreeformTool(
    val name: String,
    val description: String,
    val parameters: JsonSchema
)

/**
 * Tool specification types.
 */
@Serializable
sealed class ToolSpec {
    @Serializable
    @SerialName("function")
    data class Function(val tool: ResponsesApiTool) : ToolSpec()

    @Serializable
    @SerialName("local_shell")
    data object LocalShell : ToolSpec()

    @Serializable
    @SerialName("web_search")
    data object WebSearch : ToolSpec()

    @Serializable
    @SerialName("freeform")
    data class Freeform(val tool: FreeformTool) : ToolSpec()

    fun name(): String = when (this) {
        is Function -> tool.name
        is LocalShell -> "local_shell"
        is WebSearch -> "web_search"
        is Freeform -> tool.name
    }
}

/**
 * Configured tool spec with parallel support flag.
 */
data class ConfiguredToolSpec(
    val spec: ToolSpec,
    val supportsParallelToolCalls: kotlin.Boolean = false
)

// ============================================================================
// Tool creation functions
// ============================================================================

fun createShellTool(): ToolSpec {
    val properties = mapOf(
        "command" to JsonSchema.Array(
            items = JsonSchema.StringType(description = null),
            description = "The command to execute"
        ),
        "workdir" to JsonSchema.StringType(
            description = "The working directory to execute the command in"
        ),
        "timeout_ms" to JsonSchema.Number(
            description = "The timeout for the command in milliseconds"
        ),
        "with_escalated_permissions" to JsonSchema.Boolean(
            description = "Whether to request escalated permissions. Set to true if command needs to be run without sandbox restrictions"
        ),
        "justification" to JsonSchema.StringType(
            description = "Only set if with_escalated_permissions is true. 1-sentence explanation of why we want to run this command."
        )
    )

    val description = when {
        Platform.isWindows -> """Runs a Powershell command (Windows) and returns its output. Arguments to `shell` will be passed to CreateProcessW(). Most commands should be prefixed with ["powershell.exe", "-Command"].

Examples of valid command strings:

- ls -a (show hidden): ["powershell.exe", "-Command", "Get-ChildItem -Force"]
- recursive find by name: ["powershell.exe", "-Command", "Get-ChildItem -Recurse -Filter *.py"]
- recursive grep: ["powershell.exe", "-Command", "Get-ChildItem -Path C:\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"]
- ps aux | grep python: ["powershell.exe", "-Command", "Get-Process | Where-Object { ${'$'}.ProcessName -like '*python*' }"]
- setting an env var: ["powershell.exe", "-Command", "${'$'}env:FOO='bar'; echo ${'$'}env:FOO"]
- running an inline Python script: ["powershell.exe", "-Command", "@'\nprint('Hello, world!')\n'@ | python -"]"""
        else -> """Runs a shell command and returns its output.
- The arguments to `shell` will be passed to execvp(). Most terminal commands should be prefixed with ["bash", "-lc"].
- Always set the `workdir` param when using the shell function. Do not use `cd` unless absolutely necessary."""
    }

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "shell",
            description = description,
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("command"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createShellCommandTool(): ToolSpec {
    val properties = mapOf(
        "command" to JsonSchema.StringType(
            description = "The shell script to execute in the user's default shell"
        ),
        "workdir" to JsonSchema.StringType(
            description = "The working directory to execute the command in"
        ),
        "timeout_ms" to JsonSchema.Number(
            description = "The timeout for the command in milliseconds"
        ),
        "with_escalated_permissions" to JsonSchema.Boolean(
            description = "Whether to request escalated permissions. Set to true if command needs to be run without sandbox restrictions"
        ),
        "justification" to JsonSchema.StringType(
            description = "Only set if with_escalated_permissions is true. 1-sentence explanation of why we want to run this command."
        )
    )

    val description = when {
        Platform.isWindows -> """Runs a Powershell command (Windows) and returns its output.

Examples of valid command strings:

- ls -a (show hidden): "Get-ChildItem -Force"
- recursive find by name: "Get-ChildItem -Recurse -Filter *.py"
- recursive grep: "Get-ChildItem -Path C:\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"
- ps aux | grep python: "Get-Process | Where-Object { ${'$'}.ProcessName -like '*python*' }"
- setting an env var: "${'$'}env:FOO='bar'; echo ${'$'}env:FOO"
- running an inline Python script: "@'\nprint('Hello, world!')\n'@ | python -"""
        else -> """Runs a shell command and returns its output.
- Always set the `workdir` param when using the shell_command function. Do not use `cd` unless absolutely necessary."""
    }

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "shell_command",
            description = description,
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("command"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createExecCommandTool(): ToolSpec {
    val properties = mapOf(
        "cmd" to JsonSchema.StringType(
            description = "Shell command to execute."
        ),
        "workdir" to JsonSchema.StringType(
            description = "Optional working directory to run the command in; defaults to the turn cwd."
        ),
        "shell" to JsonSchema.StringType(
            description = "Shell binary to launch. Defaults to /bin/bash."
        ),
        "login" to JsonSchema.Boolean(
            description = "Whether to run the shell with -l/-i semantics. Defaults to true."
        ),
        "yield_time_ms" to JsonSchema.Number(
            description = "How long to wait (in milliseconds) for output before yielding."
        ),
        "max_output_tokens" to JsonSchema.Number(
            description = "Maximum number of tokens to return. Excess output will be truncated."
        ),
        "with_escalated_permissions" to JsonSchema.Boolean(
            description = "Whether to request escalated permissions. Set to true if command needs to be run without sandbox restrictions"
        ),
        "justification" to JsonSchema.StringType(
            description = "Only set if with_escalated_permissions is true. 1-sentence explanation of why we want to run this command."
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "exec_command",
            description = "Runs a command in a PTY, returning output or a session ID for ongoing interaction.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("cmd"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createWriteStdinTool(): ToolSpec {
    val properties = mapOf(
        "session_id" to JsonSchema.Number(
            description = "Identifier of the running unified exec session."
        ),
        "chars" to JsonSchema.StringType(
            description = "Bytes to write to stdin (may be empty to poll)."
        ),
        "yield_time_ms" to JsonSchema.Number(
            description = "How long to wait (in milliseconds) for output before yielding."
        ),
        "max_output_tokens" to JsonSchema.Number(
            description = "Maximum number of tokens to return. Excess output will be truncated."
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "write_stdin",
            description = "Writes characters to an existing unified exec session and returns recent output.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("session_id"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createViewImageTool(): ToolSpec {
    val properties = mapOf(
        "path" to JsonSchema.StringType(
            description = "Local filesystem path to an image file"
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "view_image",
            description = "Attach a local image (by filesystem path) to the conversation context for this turn.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("path"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createGrepFilesTool(): ToolSpec {
    val properties = mapOf(
        "pattern" to JsonSchema.StringType(
            description = "Regular expression pattern to search for."
        ),
        "include" to JsonSchema.StringType(
            description = "Optional glob that limits which files are searched (e.g. \"*.rs\" or \"*.{ts,tsx}\")."
        ),
        "path" to JsonSchema.StringType(
            description = "Directory or file path to search. Defaults to the session's working directory."
        ),
        "limit" to JsonSchema.Number(
            description = "Maximum number of file paths to return (defaults to 100)."
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "grep_files",
            description = "Finds files whose contents match the pattern and lists them by modification time.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("pattern"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createReadFileTool(): ToolSpec {
    val indentationProperties = mapOf(
        "anchor_line" to JsonSchema.Number(
            description = "Anchor line to center the indentation lookup on (defaults to offset)."
        ),
        "max_levels" to JsonSchema.Number(
            description = "How many parent indentation levels (smaller indents) to include."
        ),
        "include_siblings" to JsonSchema.Boolean(
            description = "When true, include additional blocks that share the anchor indentation."
        ),
        "include_header" to JsonSchema.Boolean(
            description = "Include doc comments or attributes directly above the selected block."
        ),
        "max_lines" to JsonSchema.Number(
            description = "Hard cap on the number of lines returned when using indentation mode."
        )
    )

    val properties = mapOf(
        "file_path" to JsonSchema.StringType(
            description = "Absolute path to the file"
        ),
        "offset" to JsonSchema.Number(
            description = "The line number to start reading from. Must be 1 or greater."
        ),
        "limit" to JsonSchema.Number(
            description = "The maximum number of lines to return."
        ),
        "mode" to JsonSchema.StringType(
            description = "Optional mode selector: \"slice\" for simple ranges (default) or \"indentation\" to expand around an anchor line."
        ),
        "indentation" to JsonSchema.Object(
            properties = indentationProperties,
            required = null,
            additionalProperties = AdditionalProperties.fromBoolean(false)
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "read_file",
            description = "Reads a local file with 1-indexed line numbers, supporting slice and indentation-aware block modes.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("file_path"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createListDirTool(): ToolSpec {
    val properties = mapOf(
        "dir_path" to JsonSchema.StringType(
            description = "Absolute path to the directory to list."
        ),
        "offset" to JsonSchema.Number(
            description = "The entry number to start listing from. Must be 1 or greater."
        ),
        "limit" to JsonSchema.Number(
            description = "The maximum number of entries to return."
        ),
        "depth" to JsonSchema.Number(
            description = "The maximum directory depth to traverse. Must be 1 or greater."
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "list_dir",
            description = "Lists entries in a local directory with 1-indexed entry numbers and simple type labels.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("dir_path"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createListMcpResourcesTool(): ToolSpec {
    val properties = mapOf(
        "server" to JsonSchema.StringType(
            description = "Optional MCP server name. When omitted, lists resources from every configured server."
        ),
        "cursor" to JsonSchema.StringType(
            description = "Opaque cursor returned by a previous list_mcp_resources call for the same server."
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "list_mcp_resources",
            description = "Lists resources provided by MCP servers. Resources allow servers to share data that provides context to language models, such as files, database schemas, or application-specific information. Prefer resources over web search when possible.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = null,
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createListMcpResourceTemplatesTool(): ToolSpec {
    val properties = mapOf(
        "server" to JsonSchema.StringType(
            description = "Optional MCP server name. When omitted, lists resource templates from all configured servers."
        ),
        "cursor" to JsonSchema.StringType(
            description = "Opaque cursor returned by a previous list_mcp_resource_templates call for the same server."
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "list_mcp_resource_templates",
            description = "Lists resource templates provided by MCP servers. Parameterized resource templates allow servers to share data that takes parameters and provides context to language models, such as files, database schemas, or application-specific information. Prefer resource templates over web search when possible.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = null,
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createReadMcpResourceTool(): ToolSpec {
    val properties = mapOf(
        "server" to JsonSchema.StringType(
            description = "MCP server name exactly as configured. Must match the 'server' field returned by list_mcp_resources."
        ),
        "uri" to JsonSchema.StringType(
            description = "Resource URI to read. Must be one of the URIs returned by list_mcp_resources."
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "read_mcp_resource",
            description = "Read a specific resource from an MCP server given the server name and resource URI.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = listOf("server", "uri"),
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

fun createTestSyncTool(): ToolSpec {
    val barrierProperties = mapOf(
        "id" to JsonSchema.StringType(
            description = "Identifier shared by concurrent calls that should rendezvous"
        ),
        "participants" to JsonSchema.Number(
            description = "Number of tool calls that must arrive before the barrier opens"
        ),
        "timeout_ms" to JsonSchema.Number(
            description = "Maximum time in milliseconds to wait at the barrier"
        )
    )

    val properties = mapOf(
        "sleep_before_ms" to JsonSchema.Number(
            description = "Optional delay in milliseconds before any other action"
        ),
        "sleep_after_ms" to JsonSchema.Number(
            description = "Optional delay in milliseconds after completing the barrier"
        ),
        "barrier" to JsonSchema.Object(
            properties = barrierProperties,
            required = listOf("id", "participants"),
            additionalProperties = AdditionalProperties.fromBoolean(false)
        )
    )

    return ToolSpec.Function(
        ResponsesApiTool(
            name = "test_sync_tool",
            description = "Internal synchronization helper used by Codex integration tests.",
            strict = false,
            parameters = JsonSchema.Object(
                properties = properties,
                required = null,
                additionalProperties = AdditionalProperties.fromBoolean(false)
            )
        )
    )
}

// ============================================================================
// JSON Schema sanitization
// ============================================================================

/**
 * Sanitize a JSON Schema so it can fit our limited JsonSchema enum.
 * This function:
 * - Ensures every schema object has a "type". If missing, infers it from
 *   common keywords (properties => object, items => array, enum/const/format => string)
 *   and otherwise defaults to "string".
 * - Fills required child fields (e.g. array items, object properties) with
 *   permissive defaults when absent.
 */
fun sanitizeJsonSchema(value: JsonElement): JsonElement {
    return when (value) {
        is JsonPrimitive -> {
            // JSON Schema boolean form: true/false. Coerce to accept-all string.
            if (value.isString.not()) {
                buildJsonObject { put("type", "string") }
            } else {
                value
            }
        }
        is kotlinx.serialization.json.JsonArray -> {
            kotlinx.serialization.json.JsonArray(value.map { sanitizeJsonSchema(it) })
        }
        is JsonObject -> {
            val map = value.toMutableMap()

            // Recursively sanitize nested schema holders
            map["properties"]?.jsonObject?.let { props ->
                map["properties"] = JsonObject(props.mapValues { (_, v) -> sanitizeJsonSchema(v) })
            }
            map["items"]?.let { items ->
                map["items"] = sanitizeJsonSchema(items)
            }
            // Sanitize oneOf/anyOf/allOf/prefixItems
            listOf("oneOf", "anyOf", "allOf", "prefixItems").forEach { combiner ->
                map[combiner]?.let { v ->
                    map[combiner] = sanitizeJsonSchema(v)
                }
            }

            // Normalize/ensure type
            var ty = map["type"]?.jsonPrimitive?.content

            // If type is an array (union), pick first supported
            if (ty == null) {
                map["type"]?.jsonArray?.let { types ->
                    for (t in types) {
                        val tt = t.jsonPrimitive.content
                        if (tt in listOf("object", "array", "string", "number", "integer", "boolean")) {
                            ty = tt
                            break
                        }
                    }
                }
            }

            // Infer type if still missing
            if (ty == null) {
                ty = when {
                    map.containsKey("properties") ||
                    map.containsKey("required") ||
                    map.containsKey("additionalProperties") -> "object"
                    map.containsKey("items") || map.containsKey("prefixItems") -> "array"
                    map.containsKey("enum") ||
                    map.containsKey("const") ||
                    map.containsKey("format") -> "string"
                    map.containsKey("minimum") ||
                    map.containsKey("maximum") ||
                    map.containsKey("exclusiveMinimum") ||
                    map.containsKey("exclusiveMaximum") ||
                    map.containsKey("multipleOf") -> "number"
                    else -> "string"
                }
            }

            map["type"] = JsonPrimitive(ty)

            // Ensure object schemas have properties map
            if (ty == "object" && !map.containsKey("properties")) {
                map["properties"] = JsonObject(emptyMap())
            }

            // Sanitize additionalProperties if it's an object schema
            map["additionalProperties"]?.let { ap ->
                if (ap is JsonObject) {
                    map["additionalProperties"] = sanitizeJsonSchema(ap)
                }
            }

            // Ensure array schemas have items
            if (ty == "array" && !map.containsKey("items")) {
                map["items"] = buildJsonObject { put("type", "string") }
            }

            JsonObject(map)
        }
        else -> value
    }
}

// ============================================================================
// Platform detection helper
// ============================================================================

internal object Platform {
    @OptIn(kotlin.experimental.ExperimentalNativeApi::class)
    val isWindows: kotlin.Boolean
        get() = kotlin.native.Platform.osFamily == kotlin.native.OsFamily.WINDOWS

    @OptIn(kotlin.experimental.ExperimentalNativeApi::class)
    val isLinux: kotlin.Boolean
        get() = kotlin.native.Platform.osFamily == kotlin.native.OsFamily.LINUX

    @OptIn(kotlin.experimental.ExperimentalNativeApi::class)
    val isMacos: kotlin.Boolean
        get() = kotlin.native.Platform.osFamily == kotlin.native.OsFamily.MACOSX
}
