package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okio.FileSystem
import okio.Path.Companion.toPath
import okio.buffer
import okio.use

/**
 * Handler for the read_file tool.
 * Reads file contents with support for slice mode and indentation-aware block mode.
 *
 * Ported from Rust codex-rs/core/src/tools/handlers/read_file.rs
 */
class ReadFileHandler : ToolHandler {

    override val kind: ToolKind = ToolKind.Function

    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val payload = invocation.payload
        if (payload !is ToolPayload.Function) {
            return CodexResult.failure(
                CodexError.Fatal("read_file handler received unsupported payload")
            )
        }

        val args = try {
            json.decodeFromString<ReadFileArgs>(payload.arguments)
        } catch (e: Exception) {
            return CodexResult.failure(
                CodexError.Fatal("failed to parse function arguments: ${e.message}")
            )
        }

        // Validate arguments
        if (args.offset == 0) {
            return CodexResult.failure(
                CodexError.Fatal("offset must be a 1-indexed line number")
            )
        }

        if (args.limit == 0) {
            return CodexResult.failure(
                CodexError.Fatal("limit must be greater than zero")
            )
        }

        val filePath = args.filePath
        if (!filePath.startsWith("/") && !filePath.matches(Regex("^[A-Za-z]:.*"))) {
            return CodexResult.failure(
                CodexError.Fatal("file_path must be an absolute path")
            )
        }

        return try {
            val content = when (args.mode) {
                ReadMode.slice -> readSlice(filePath, args.offset, args.limit)
                ReadMode.indentation -> {
                    val indentArgs = args.indentation ?: IndentationArgs()
                    readIndentationBlock(filePath, args.offset, args.limit, indentArgs)
                }
            }
            CodexResult.success(
                ToolOutput.Function(
                    content = content.joinToString("\n"),
                    success = true
                )
            )
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("failed to read file: ${e.message}")
            )
        }
    }

    companion object {
        private const val MAX_LINE_LENGTH = 500
        private const val TAB_WIDTH = 4

        private val json = Json {
            ignoreUnknownKeys = true
            isLenient = true
        }

        /**
         * Read a simple slice of lines from a file.
         */
        private fun readSlice(filePath: String, offset: Int, limit: Int): List<String> {
            val path = filePath.toPath()
            val collected = mutableListOf<String>()
            var lineNumber = 0

            FileSystem.SYSTEM.source(path).buffer().use { source ->
                while (true) {
                    val line = source.readUtf8Line() ?: break
                    lineNumber++

                    if (lineNumber < offset) continue
                    if (collected.size >= limit) break

                    val formatted = formatLine(line)
                    collected.add("L$lineNumber: $formatted")
                }
            }

            if (lineNumber < offset) {
                throw IllegalArgumentException("offset exceeds file length")
            }

            return collected
        }

        /**
         * Read an indentation-aware block from a file.
         */
        private fun readIndentationBlock(
            filePath: String,
            offset: Int,
            limit: Int,
            options: IndentationArgs
        ): List<String> {
            val anchorLine = options.anchor_line ?: offset
            if (anchorLine == 0) {
                throw IllegalArgumentException("anchor_line must be a 1-indexed line number")
            }

            val guardLimit = options.max_lines ?: limit
            if (guardLimit == 0) {
                throw IllegalArgumentException("max_lines must be greater than zero")
            }

            // Collect all lines from file
            val allLines = collectFileLines(filePath)
            if (allLines.isEmpty() || anchorLine > allLines.size) {
                throw IllegalArgumentException("anchor_line exceeds file length")
            }

            val anchorIndex = anchorLine - 1
            val effectiveIndents = computeEffectiveIndents(allLines)
            val anchorIndent = effectiveIndents[anchorIndex]

            // Compute min indent based on maxLevels
            val minIndent = if (options.maxLevels == 0) {
                0
            } else {
                maxOf(0, anchorIndent - options.maxLevels * TAB_WIDTH)
            }

            // Cap requested lines
            val finalLimit = minOf(limit, guardLimit, allLines.size)

            if (finalLimit == 1) {
                return listOf("L${allLines[anchorIndex].number}: ${allLines[anchorIndex].display}")
            }

            // Build output using bidirectional expansion
            val out = ArrayDeque<LineRecord>()
            out.addLast(allLines[anchorIndex])

            var i = anchorIndex - 1  // up cursor
            var j = anchorIndex + 1  // down cursor
            var iCounterMinIndent = 0
            var jCounterMinIndent = 0

            while (out.size < finalLimit) {
                var progressed = 0

                // Expand upward
                if (i >= 0) {
                    if (effectiveIndents[i] >= minIndent) {
                        out.addFirst(allLines[i])
                        progressed++

                        // Check sibling handling
                        if (effectiveIndents[i] == minIndent && !options.include_siblings) {
                            val allowHeaderComment = options.include_header && allLines[i].isComment()
                            val canTakeLine = allowHeaderComment || iCounterMinIndent == 0

                            if (canTakeLine) {
                                iCounterMinIndent++
                            } else {
                                out.removeFirst()
                                progressed--
                                i = -1
                            }
                        }

                        i--

                        if (out.size >= finalLimit) break
                    } else {
                        i = -1
                    }
                }

                // Expand downward
                if (j < allLines.size) {
                    if (effectiveIndents[j] >= minIndent) {
                        out.addLast(allLines[j])
                        progressed++

                        // Check sibling handling
                        if (effectiveIndents[j] == minIndent && !options.include_siblings) {
                            if (jCounterMinIndent > 0) {
                                out.removeLast()
                                progressed--
                                j = allLines.size
                            }
                            jCounterMinIndent++
                        }

                        j++
                    } else {
                        j = allLines.size
                    }
                }

                if (progressed == 0) break
            }

            // Trim empty lines from both ends
            while (out.isNotEmpty() && out.first().isBlank()) {
                out.removeFirst()
            }
            while (out.isNotEmpty() && out.last().isBlank()) {
                out.removeLast()
            }

            return out.map { "L${it.number}: ${it.display}" }
        }

        /**
         * Collect all lines from a file into LineRecord objects.
         */
        private fun collectFileLines(filePath: String): List<LineRecord> {
            val path = filePath.toPath()
            val lines = mutableListOf<LineRecord>()
            var number = 0

            FileSystem.SYSTEM.source(path).buffer().use { source ->
                while (true) {
                    val raw = source.readUtf8Line() ?: break
                    number++
                    val indent = measureIndent(raw)
                    val display = formatLine(raw)
                    lines.add(LineRecord(number, raw, display, indent))
                }
            }

            return lines
        }

        /**
         * Compute effective indentation for each line.
         * Blank lines inherit the previous line's indentation.
         */
        private fun computeEffectiveIndents(records: List<LineRecord>): List<Int> {
            val effective = mutableListOf<Int>()
            var previousIndent = 0
            for (record in records) {
                if (record.isBlank()) {
                    effective.add(previousIndent)
                } else {
                    previousIndent = record.indent
                    effective.add(previousIndent)
                }
            }
            return effective
        }

        /**
         * Measure the indentation of a line in spaces (tabs count as TAB_WIDTH).
         */
        private fun measureIndent(line: String): Int {
            var indent = 0
            for (char in line) {
                when (char) {
                    ' ' -> indent++
                    '\t' -> indent += TAB_WIDTH
                    else -> break
                }
            }
            return indent
        }

        /**
         * Format a line for output, truncating if necessary.
         */
        private fun formatLine(line: String): String {
            // Remove trailing CR if present (for CRLF files)
            val trimmed = if (line.endsWith('\r')) line.dropLast(1) else line

            return if (trimmed.length > MAX_LINE_LENGTH) {
                trimmed.take(MAX_LINE_LENGTH)
            } else {
                trimmed
            }
        }
    }
}

/**
 * Arguments for the read_file tool.
 */
@Serializable
private data class ReadFileArgs(
    @SerialName("file_path")
    val filePath: String,
    val offset: Int = 1,
    val limit: Int = 2000,
    val mode: ReadMode = ReadMode.slice,
    val indentation: IndentationArgs? = null
)

/**
 * Read mode for the read_file tool.
 */
@Serializable
private enum class ReadMode {
    @SerialName("slice")
    slice,
    @SerialName("indentation")
    indentation
}

/**
 * Arguments for indentation-aware reading.
 */
@Serializable
private data class IndentationArgs(
    @SerialName("anchor_line")
    val anchorLine: Int? = null,
    @SerialName("max_levels")
    val maxLevels: Int = 0,
    @SerialName("include_siblings")
    val includeSiblings: Boolean = false,
    @SerialName("include_header")
    val includeHeader: Boolean = true,
    @SerialName("max_lines")
    val maxLines: Int? = null
)

/**
 * Internal representation of a line with metadata.
 */
private data class LineRecord(
    val number: Int,
    val raw: String,
    val display: String,
    val indent: Int
) {
    fun trimmed(): String = raw.trimStart()

    fun isBlank(): Boolean = trimmed().isEmpty()

    fun isComment(): Boolean {
        val trimmed = raw.trim()
        return COMMENT_PREFIXES.any { trimmed.startsWith(it) }
    }

    companion object {
        private val COMMENT_PREFIXES = listOf("#", "//", "--")
    }
}
