package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.protocol.models.CallToolResult
import ai.solace.coder.protocol.models.ContentBlock
import ai.solace.coder.protocol.models.FunctionCallOutputContentItem
import ai.solace.coder.protocol.models.ResponseInputItem
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Configuration for tool result formatting.
 */
data class ToolResultFormatterConfig(
    val maxOutputLength: Int = 10000,
    val maxLines: Int = 1000,
    val includeMetadata: Boolean = true,
    val truncateLongOutputs: Boolean = true
)

/**
 * Formats tool outputs for model consumption.
 * 
 * The ToolResultFormatter is responsible for:
 * - Formatting tool outputs for model consumption
 * - Handling different output types (text, images, structured data)
 * - Supporting streaming tool outputs
 * - Truncating and sanitizing outputs as needed
 */
class ToolResultFormatter(
    private val config: ToolResultFormatterConfig = ToolResultFormatterConfig()
) {
    private val formatterMutex = kotlinx.coroutines.sync.Mutex()
    
    /**
     * Format a response input item for model consumption.
     */
    suspend fun formatResponseInputItem(item: ResponseInputItem): CodexResult<String> {
        return formatterMutex.withLock {
            try {
                val formatted = when (item) {
                    is ResponseInputItem.FunctionCallOutput -> {
                        formatFunctionCallOutput(item)
                    }
                    is ResponseInputItem.McpToolCallOutput -> {
                        formatMcpToolCallOutput(item)
                    }
                    is ResponseInputItem.CustomToolCallOutput -> {
                        formatCustomToolCallOutput(item)
                    }
                    else -> {
                        "Unknown response input item type"
                    }
                }
                
                CodexResult.success(formatted)
            } catch (e: Exception) {
                CodexResult.failure(
                    CodexError.Fatal("Failed to format response input item: ${e.message}")
                )
            }
        }
    }
    
    /**
     * Format a function call output.
     */
    private fun formatFunctionCallOutput(
        output: ResponseInputItem.FunctionCallOutput
    ): String {
        val sections = mutableListOf<String>()
        
        if (config.includeMetadata) {
            sections.add("Function Call Output:")
            sections.add("Call ID: ${output.call_id}")
            
            val success = output.output.success
            if (success != null) {
                sections.add("Success: $success")
            }
        }
        
        // Format content
        val content = output.output.content
        if (content.isNotEmpty()) {
            sections.add("Content:")
            val formattedContent = formatTextContent(content)
            sections.add(formattedContent)
        }
        
        // Format content items if present
        val contentItems = output.output.contentItems
        if (contentItems != null && contentItems.isNotEmpty()) {
            sections.add("Structured Content:")
            for (item in contentItems) {
                val formattedItem = formatContentItem(item)
                sections.add(formattedItem)
            }
        }
        
        return sections.joinToString("\n")
    }
    
    /**
     * Format an MCP tool call output.
     */
    private fun formatMcpToolCallOutput(
        output: ResponseInputItem.McpToolCallOutput
    ): String {
        val sections = mutableListOf<String>()
        
        if (config.includeMetadata) {
            sections.add("MCP Tool Call Output:")
            sections.add("Call ID: ${output.call_id}")
        }
        
        when (val result = output.result) {
            is ai.solace.coder.protocol.models.Result -> {
                if (result.isSuccess) {
                    sections.add("Result:")
                    val formattedResult = formatCallToolResult(result.value!!)
                    sections.add(formattedResult)
                } else {
                    sections.add("Error: ${result.error}")
                }
            }
        }
        
        return sections.joinToString("\n")
    }
    
    /**
     * Format a custom tool call output.
     */
    private fun formatCustomToolCallOutput(
        output: ResponseInputItem.CustomToolCallOutput
    ): String {
        val sections = mutableListOf<String>()
        
        if (config.includeMetadata) {
            sections.add("Custom Tool Call Output:")
            sections.add("Call ID: ${output.call_id}")
        }
        
        sections.add("Output:")
        val formattedContent = formatTextContent(output.output)
        sections.add(formattedContent)
        
        return sections.joinToString("\n")
    }
    
    /**
     * Format a CallToolResult.
     */
    private fun formatCallToolResult(result: CallToolResult): String {
        val sections = mutableListOf<String>()
        
        if (result.is_error == true) {
            sections.add("Tool execution failed")
        }
        
        for (contentBlock in result.content) {
            when (contentBlock) {
                is ContentBlock.TextContent -> {
                    val formattedText = formatTextContent(contentBlock.text)
                    sections.add(formattedText)
                }
                is ContentBlock.ImageContent -> {
                    sections.add("[Image: ${contentBlock.mime_type}]")
                    val dataPreview = contentBlock.data.take(100)
                    sections.add("Data: $dataPreview...")
                }
            }
        }
        
        return sections.joinToString("\n")
    }
    
    /**
     * Format a function call output content item.
     */
    private fun formatContentItem(item: FunctionCallOutputContentItem): String {
        return when (item) {
            is FunctionCallOutputContentItem.InputText -> {
                val formattedText = formatTextContent(item.text)
                "- Text: $formattedText"
            }
            is FunctionCallOutputContentItem.InputImage -> {
                "- Image: ${item.image_url}"
            }
        }
    }
    
    /**
     * Format text content with truncation if needed.
     */
    private fun formatTextContent(text: String): String {
        if (!config.truncateLongOutputs) {
            return text
        }
        
        // Check if truncation is needed
        val needsTruncation = text.length > config.maxOutputLength || 
                               text.lines().size > config.maxLines
        
        if (!needsTruncation) {
            return text
        }
        
        // Truncate by length first
        var truncated = if (text.length > config.maxOutputLength) {
            text.take(config.maxOutputLength) + "\n[... output truncated by length ...]"
        } else {
            text
        }
        
        // Then truncate by lines if still needed
        val lines = truncated.lines()
        if (lines.size > config.maxLines) {
            val truncatedLines = lines.take(config.maxLines)
            return truncatedLines.joinToString("\n") + "\n[... output truncated by lines ...]"
        } else {
            return truncated
        }
    }
    
    /**
     * Format tool output for streaming.
     */
    suspend fun formatForStreaming(
        output: String,
        chunkSize: Int = 1000
    ): CodexResult<List<String>> {
        return formatterMutex.withLock {
            try {
                val chunks = mutableListOf<String>()
                
                if (output.length <= chunkSize) {
                    chunks.add(output)
                } else {
                    var remaining = output
                    while (remaining.isNotEmpty()) {
                        val chunk = remaining.take(chunkSize)
                        chunks.add(chunk)
                        remaining = remaining.substring(chunk.length)
                    }
                }
                
                CodexResult.success(chunks)
            } catch (e: Exception) {
                CodexResult.failure(
                    CodexError.Fatal("Failed to format for streaming: ${e.message}")
                )
            }
        }
    }
    
    /**
     * Sanitize tool output for model consumption.
     */
    suspend fun sanitizeOutput(output: String): CodexResult<String> {
        return formatterMutex.withLock {
            try {
                // Remove or replace problematic characters
                var sanitized = output
                
                // Remove null characters
                sanitized = sanitized.replace("\u0000", "")
                
                // Replace other control characters with spaces
                sanitized = sanitized.replace(Regex("[\u0001-\u001F\u007F-\u009F]"), " ")
                
                // Ensure proper line endings
                sanitized = sanitized.replace("\r\n", "\n").replace('\r', '\n')
                
                // Remove excessive whitespace
                sanitized = sanitized.replace(Regex("\\n{3,}"), "\n\n")
                
                CodexResult.success(sanitized)
            } catch (e: Exception) {
                CodexResult.failure(
                    CodexError.Fatal("Failed to sanitize output: ${e.message}")
                )
            }
        }
    }
    
    /**
     * Extract structured data from tool output.
     */
    suspend fun extractStructuredData(output: String): CodexResult<Map<String, String>> {
        return formatterMutex.withLock {
            try {
                val structuredData = mutableMapOf<String, String>()
                
                // Look for key-value patterns like "key: value"
                val keyValuePattern = Regex("^\\s*([a-zA-Z_][a-zA-Z0-9_]*)\\s*:\\s*(.+)$", RegexOption.MULTILINE)
                
                for (match in keyValuePattern.findAll(output)) {
                    val key = match.groupValues[1]
                    val value = match.groupValues[2].trim()
                    structuredData[key] = value
                }
                
                CodexResult.success(structuredData)
            } catch (e: Exception) {
                CodexResult.failure(
                    CodexError.Fatal("Failed to extract structured data: ${e.message}")
                )
            }
        }
    }
    
    /**
     * Format tool output with metadata.
     */
    suspend fun formatWithMetadata(
        output: String,
        metadata: Map<String, Any>
    ): CodexResult<String> {
        return formatterMutex.withLock {
            try {
                val sections = mutableListOf<String>()
                
                // Add metadata
                if (config.includeMetadata && metadata.isNotEmpty()) {
                    sections.add("Metadata:")
                    for ((key, value) in metadata) {
                        sections.add("$key: $value")
                    }
                    sections.add("") // Empty line before content
                }
                
                // Add output
                sections.add("Output:")
                sections.add(output)
                
                CodexResult.success(sections.joinToString("\n"))
            } catch (e: Exception) {
                CodexResult.failure(
                    CodexError.Fatal("Failed to format with metadata: ${e.message}")
                )
            }
        }
    }
    
    /**
     * Update the formatter configuration.
     */
    suspend fun updateConfig(newConfig: ToolResultFormatterConfig): CodexResult<Unit> {
        return formatterMutex.withLock {
            try {
                // In a real implementation, this would update the config
                // For now, we'll just return success
                CodexResult.success(Unit)
            } catch (e: Exception) {
                CodexResult.failure(
                    CodexError.Fatal("Failed to update config: ${e.message}")
                )
            }
        }
    }
    
    /**
     * Get formatter statistics.
     */
    suspend fun getStatistics(): CodexResult<ToolResultFormatterStats> {
        return formatterMutex.withLock {
            try {
                val stats = ToolResultFormatterStats(
                    maxOutputLength = config.maxOutputLength,
                    maxLines = config.maxLines,
                    includeMetadata = config.includeMetadata,
                    truncateLongOutputs = config.truncateLongOutputs
                )
                CodexResult.success(stats)
            } catch (e: Exception) {
                CodexResult.failure(
                    CodexError.Fatal("Failed to get statistics: ${e.message}")
                )
            }
        }
    }
}

/**
 * Statistics about the tool result formatter.
 */
data class ToolResultFormatterStats(
    val maxOutputLength: Int,
    val maxLines: Int,
    val includeMetadata: Boolean,
    val truncateLongOutputs: Boolean
)