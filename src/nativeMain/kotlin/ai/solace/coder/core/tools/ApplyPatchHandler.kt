package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okio.FileSystem
import okio.Path.Companion.toPath
import okio.buffer
import okio.use

/**
 * Handler for the apply_patch tool.
 * Applies unified diff patches to files.
 *
 * Ported from Rust codex-rs/core/src/tools/handlers/apply_patch.rs
 *
 * TODO: Full implementation requires:
 * - [ ] Lark grammar parser for patch format (tool_apply_patch.lark)
 * - [ ] codex_apply_patch::maybe_parse_apply_patch_verified() equivalent
 * - [ ] InternalApplyPatchInvocation for output vs delegate decision
 * - [ ] ApplyPatchRuntime for actual file operations
 * - [ ] ToolOrchestrator integration for approval workflow
 * - [ ] Support for Add File, Delete File, Update File, Move To operations
 * - [ ] Hunk application with context matching
 * - [ ] Freeform tool format support (grammar-based)
 */
class ApplyPatchHandler : ToolHandler {

    override val kind: ToolKind = ToolKind.Function

    override fun matchesKind(payload: ToolPayload): Boolean {
        return payload is ToolPayload.Function || payload is ToolPayload.Custom
    }

    override fun isMutating(invocation: ToolInvocation): Boolean = true

    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val patchInput = when (val payload = invocation.payload) {
            is ToolPayload.Function -> {
                val args = try {
                    json.decodeFromString<ApplyPatchArgs>(payload.arguments)
                } catch (e: Exception) {
                    return CodexResult.failure(
                        CodexError.Fatal("failed to parse function arguments: ${e.message}")
                    )
                }
                args.input
            }
            is ToolPayload.Custom -> payload.input
            else -> {
                return CodexResult.failure(
                    CodexError.Fatal("apply_patch handler received unsupported payload")
                )
            }
        }

        val cwd = invocation.turn.cwd

        return try {
            val result = applyPatch(patchInput, cwd)
            result.map { message ->
                ToolOutput.Function(
                    content = message,
                    success = true
                )
            }
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("apply_patch failed: ${e.message}")
            )
        }
    }

    companion object {
        private val json = Json {
            ignoreUnknownKeys = true
            isLenient = true
        }

        private const val BEGIN_PATCH = "*** Begin Patch"
        private const val END_PATCH = "*** End Patch"
        private const val ADD_FILE_PREFIX = "*** Add File: "
        private const val DELETE_FILE_PREFIX = "*** Delete File: "
        private const val UPDATE_FILE_PREFIX = "*** Update File: "
        private const val MOVE_TO_PREFIX = "*** Move to: "
        private const val HUNK_MARKER = "@@"
        private const val END_OF_FILE_MARKER = "*** End of File"

        /**
         * Apply a patch to files in the working directory.
         */
        private fun applyPatch(patchInput: String, cwd: String): CodexResult<String> {
            val lines = patchInput.lines()
            val operations = parsePatch(lines)
                ?: return CodexResult.failure(
                    CodexError.Fatal("Failed to parse patch: invalid format")
                )

            val results = mutableListOf<String>()

            for (op in operations) {
                when (op) {
                    is FileOperation.AddFile -> {
                        val result = addFile(op.path, op.content, cwd)
                        if (result.isFailure()) return result.map { "" }
                        results.add("Added file: ${op.path}")
                    }
                    is FileOperation.DeleteFile -> {
                        val result = deleteFile(op.path, cwd)
                        if (result.isFailure()) return result.map { "" }
                        results.add("Deleted file: ${op.path}")
                    }
                    is FileOperation.UpdateFile -> {
                        val result = updateFile(op.path, op.newPath, op.hunks, cwd)
                        if (result.isFailure()) return result.map { "" }
                        val msg = if (op.newPath != null) {
                            "Updated and moved file: ${op.path} -> ${op.newPath}"
                        } else {
                            "Updated file: ${op.path}"
                        }
                        results.add(msg)
                    }
                }
            }

            return CodexResult.success(results.joinToString("\n"))
        }

        /**
         * Parse patch text into file operations.
         */
        private fun parsePatch(lines: List<String>): List<FileOperation>? {
            val trimmedLines = lines.map { it }
            val startIdx = trimmedLines.indexOfFirst { it.trim() == BEGIN_PATCH }
            val endIdx = trimmedLines.indexOfLast { it.trim() == END_PATCH }

            if (startIdx == -1 || endIdx == -1 || startIdx >= endIdx) {
                return null
            }

            val patchLines = trimmedLines.subList(startIdx + 1, endIdx)
            val operations = mutableListOf<FileOperation>()
            var i = 0

            while (i < patchLines.size) {
                val line = patchLines[i]

                when {
                    line.startsWith(ADD_FILE_PREFIX) -> {
                        val path = line.removePrefix(ADD_FILE_PREFIX).trim()
                        val content = StringBuilder()
                        i++
                        while (i < patchLines.size && patchLines[i].startsWith("+")) {
                            content.appendLine(patchLines[i].removePrefix("+"))
                            i++
                        }
                        operations.add(FileOperation.AddFile(path, content.toString().trimEnd()))
                    }
                    line.startsWith(DELETE_FILE_PREFIX) -> {
                        val path = line.removePrefix(DELETE_FILE_PREFIX).trim()
                        operations.add(FileOperation.DeleteFile(path))
                        i++
                    }
                    line.startsWith(UPDATE_FILE_PREFIX) -> {
                        val path = line.removePrefix(UPDATE_FILE_PREFIX).trim()
                        i++
                        var newPath: String? = null
                        if (i < patchLines.size && patchLines[i].startsWith(MOVE_TO_PREFIX)) {
                            newPath = patchLines[i].removePrefix(MOVE_TO_PREFIX).trim()
                            i++
                        }
                        val hunks = mutableListOf<Hunk>()
                        while (i < patchLines.size && !patchLines[i].startsWith("*** ")) {
                            if (patchLines[i].startsWith(HUNK_MARKER)) {
                                val header = patchLines[i].removePrefix(HUNK_MARKER).trim()
                                i++
                                val hunkLines = mutableListOf<HunkLine>()
                                while (i < patchLines.size &&
                                       !patchLines[i].startsWith(HUNK_MARKER) &&
                                       !patchLines[i].startsWith("*** ")) {
                                    val hunkLine = patchLines[i]
                                    when {
                                        hunkLine.startsWith(" ") ->
                                            hunkLines.add(HunkLine.Context(hunkLine.drop(1)))
                                        hunkLine.startsWith("-") ->
                                            hunkLines.add(HunkLine.Remove(hunkLine.drop(1)))
                                        hunkLine.startsWith("+") ->
                                            hunkLines.add(HunkLine.Add(hunkLine.drop(1)))
                                        hunkLine == END_OF_FILE_MARKER -> {
                                            // Skip end of file marker
                                        }
                                        else -> {
                                            // Treat as context line without prefix
                                            hunkLines.add(HunkLine.Context(hunkLine))
                                        }
                                    }
                                    i++
                                }
                                hunks.add(Hunk(header.takeIf { it.isNotEmpty() }, hunkLines))
                            } else {
                                i++
                            }
                        }
                        operations.add(FileOperation.UpdateFile(path, newPath, hunks))
                    }
                    else -> i++
                }
            }

            return operations
        }

        /**
         * Add a new file.
         */
        private fun addFile(path: String, content: String, cwd: String): CodexResult<Unit> {
            return try {
                val fullPath = resolvePath(path, cwd).toPath()

                // Create parent directories if needed
                fullPath.parent?.let { parent ->
                    if (!FileSystem.SYSTEM.exists(parent)) {
                        FileSystem.SYSTEM.createDirectories(parent)
                    }
                }

                FileSystem.SYSTEM.sink(fullPath).buffer().use { sink ->
                    sink.writeUtf8(content)
                }
                CodexResult.success(Unit)
            } catch (e: Exception) {
                CodexResult.failure(CodexError.Fatal("Failed to add file $path: ${e.message}"))
            }
        }

        /**
         * Delete a file.
         */
        private fun deleteFile(path: String, cwd: String): CodexResult<Unit> {
            return try {
                val fullPath = resolvePath(path, cwd).toPath()
                FileSystem.SYSTEM.delete(fullPath)
                CodexResult.success(Unit)
            } catch (e: Exception) {
                CodexResult.failure(CodexError.Fatal("Failed to delete file $path: ${e.message}"))
            }
        }

        /**
         * Update a file by applying hunks.
         */
        private fun updateFile(
            path: String,
            newPath: String?,
            hunks: List<Hunk>,
            cwd: String
        ): CodexResult<Unit> {
            return try {
                val fullPath = resolvePath(path, cwd).toPath()

                // Read existing content
                val existingContent = FileSystem.SYSTEM.source(fullPath).buffer().use { source ->
                    source.readUtf8()
                }
                val existingLines = existingContent.lines().toMutableList()

                // Apply hunks
                for (hunk in hunks) {
                    applyHunk(existingLines, hunk)
                }

                val newContent = existingLines.joinToString("\n")

                // Handle move if specified
                val targetPath = if (newPath != null) {
                    FileSystem.SYSTEM.delete(fullPath)
                    resolvePath(newPath, cwd).toPath()
                } else {
                    fullPath
                }

                // Create parent directories if needed (for move)
                targetPath.parent?.let { parent ->
                    if (!FileSystem.SYSTEM.exists(parent)) {
                        FileSystem.SYSTEM.createDirectories(parent)
                    }
                }

                FileSystem.SYSTEM.sink(targetPath).buffer().use { sink ->
                    sink.writeUtf8(newContent)
                }

                CodexResult.success(Unit)
            } catch (e: Exception) {
                CodexResult.failure(CodexError.Fatal("Failed to update file $path: ${e.message}"))
            }
        }

        /**
         * Apply a single hunk to file lines.
         * Uses context lines to find the correct location.
         */
        private fun applyHunk(lines: MutableList<String>, hunk: Hunk) {
            // Find the location to apply the hunk based on context
            val contextLines = hunk.lines.filterIsInstance<HunkLine.Context>()
            val removeLines = hunk.lines.filterIsInstance<HunkLine.Remove>()

            // Try to find the location by matching context/remove lines
            val searchPattern = (contextLines.take(3) + removeLines).map {
                when (it) {
                    is HunkLine.Context -> it.text
                    is HunkLine.Remove -> it.text
                    else -> ""
                }
            }

            var matchIndex = -1
            if (searchPattern.isNotEmpty()) {
                outer@ for (i in lines.indices) {
                    for ((j, pattern) in searchPattern.withIndex()) {
                        if (i + j >= lines.size || lines[i + j].trim() != pattern.trim()) {
                            continue@outer
                        }
                    }
                    matchIndex = i
                    break
                }
            }

            if (matchIndex == -1) {
                // Fallback: apply at end of file
                matchIndex = lines.size
            }

            // Apply changes at matchIndex
            var currentIndex = matchIndex
            for (line in hunk.lines) {
                when (line) {
                    is HunkLine.Context -> currentIndex++
                    is HunkLine.Remove -> {
                        if (currentIndex < lines.size) {
                            lines.removeAt(currentIndex)
                        }
                    }
                    is HunkLine.Add -> {
                        lines.add(currentIndex, line.text)
                        currentIndex++
                    }
                }
            }
        }

        /**
         * Resolve a relative path against the working directory.
         */
        private fun resolvePath(path: String, cwd: String): String {
            return if (path.startsWith("/") || path.matches(Regex("^[A-Za-z]:.*"))) {
                path
            } else {
                if (cwd.endsWith("/") || cwd.endsWith("\\")) {
                    "$cwd$path"
                } else {
                    "$cwd/$path"
                }
            }
        }
    }
}

/**
 * Arguments for the apply_patch tool.
 */
@Serializable
private data class ApplyPatchArgs(
    val input: String
)

/**
 * File operations parsed from a patch.
 */
private sealed class FileOperation {
    data class AddFile(val path: String, val content: String) : FileOperation()
    data class DeleteFile(val path: String) : FileOperation()
    data class UpdateFile(val path: String, val newPath: String?, val hunks: List<Hunk>) : FileOperation()
}

/**
 * A hunk within an update operation.
 */
private data class Hunk(
    val header: String?,
    val lines: List<HunkLine>
)

/**
 * Individual lines within a hunk.
 */
private sealed class HunkLine {
    data class Context(val text: String) : HunkLine()
    data class Remove(val text: String) : HunkLine()
    data class Add(val text: String) : HunkLine()
}
