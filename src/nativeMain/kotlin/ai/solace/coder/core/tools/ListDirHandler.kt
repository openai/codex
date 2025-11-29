package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okio.FileSystem
import okio.Path
import okio.Path.Companion.toPath

/**
 * Handler for the list_dir tool.
 * Lists directory contents with support for depth, pagination, and entry formatting.
 *
 * Ported from Rust codex-rs/core/src/tools/handlers/list_dir.rs
 */
class ListDirHandler : ToolHandler {

    override val kind: ToolKind = ToolKind.Function

    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val payload = invocation.payload
        if (payload !is ToolPayload.Function) {
            return CodexResult.failure(
                CodexError.Fatal("list_dir handler received unsupported payload")
            )
        }

        val args = try {
            json.decodeFromString<ListDirArgs>(payload.arguments)
        } catch (e: Exception) {
            return CodexResult.failure(
                CodexError.Fatal("failed to parse function arguments: ${e.message}")
            )
        }

        // Validate arguments
        if (args.offset == 0) {
            return CodexResult.failure(
                CodexError.Fatal("offset must be a 1-indexed entry number")
            )
        }

        if (args.limit == 0) {
            return CodexResult.failure(
                CodexError.Fatal("limit must be greater than zero")
            )
        }

        if (args.depth == 0) {
            return CodexResult.failure(
                CodexError.Fatal("depth must be greater than zero")
            )
        }

        val dirPath = args.dirPath
        if (!dirPath.startsWith("/") && !dirPath.matches(Regex("^[A-Za-z]:.*"))) {
            return CodexResult.failure(
                CodexError.Fatal("dir_path must be an absolute path")
            )
        }

        return try {
            val entries = listDirSlice(dirPath.toPath(), args.offset, args.limit, args.depth)
            val output = mutableListOf<String>()
            output.add("Absolute path: $dirPath")
            output.addAll(entries)

            CodexResult.success(
                ToolOutput.Function(
                    content = output.joinToString("\n"),
                    success = true
                )
            )
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("failed to read directory: ${e.message}")
            )
        }
    }

    companion object {
        private const val MAX_ENTRY_LENGTH = 500
        private const val INDENTATION_SPACES = 2

        private val json = Json {
            ignoreUnknownKeys = true
            isLenient = true
        }

        /**
         * List directory entries with pagination.
         */
        private fun listDirSlice(path: Path, offset: Int, limit: Int, depth: Int): List<String> {
            val entries = mutableListOf<DirEntry>()
            collectEntries(path, "".toPath(), depth, entries)

            if (entries.isEmpty()) {
                return emptyList()
            }

            val startIndex = offset - 1
            if (startIndex >= entries.size) {
                throw IllegalArgumentException("offset exceeds directory entry count")
            }

            val remainingEntries = entries.size - startIndex
            val cappedLimit = minOf(limit, remainingEntries)
            val endIndex = startIndex + cappedLimit

            val selectedEntries = entries.subList(startIndex, endIndex).sortedBy { it.name }
            val formatted = selectedEntries.map { formatEntryLine(it) }.toMutableList()

            if (endIndex < entries.size) {
                formatted.add("More than $cappedLimit entries found")
            }

            return formatted
        }

        /**
         * Collect directory entries recursively using BFS.
         */
        private fun collectEntries(
            dirPath: Path,
            relativePrefix: Path,
            depth: Int,
            entries: MutableList<DirEntry>
        ) {
            val queue = ArrayDeque<Triple<Path, Path, Int>>()
            queue.addLast(Triple(dirPath, relativePrefix, depth))

            while (queue.isNotEmpty()) {
                val (currentDir, prefix, remainingDepth) = queue.removeFirst()

                val dirEntries = mutableListOf<Pair<Path, DirEntry>>()

                try {
                    FileSystem.SYSTEM.list(currentDir).forEach { entryPath ->
                        val metadata = FileSystem.SYSTEM.metadataOrNull(entryPath)
                        val fileName = entryPath.name
                        val relativePath = if (prefix.toString().isEmpty()) {
                            fileName.toPath()
                        } else {
                            (prefix.toString() + "/" + fileName).toPath()
                        }

                        val displayName = formatEntryComponent(fileName)
                        val displayDepth = prefix.toString().split("/").filter { it.isNotEmpty() }.size
                        val sortKey = formatEntryName(relativePath)
                        val kind = when {
                            metadata?.symlinkTarget != null -> DirEntryKind.Symlink
                            metadata?.isDirectory == true -> DirEntryKind.Directory
                            metadata?.isRegularFile == true -> DirEntryKind.File
                            else -> DirEntryKind.Other
                        }

                        val entry = DirEntry(
                            name = sortKey,
                            displayName = displayName,
                            depth = displayDepth,
                            kind = kind
                        )

                        dirEntries.add(Pair(entryPath, entry))

                        // Queue subdirectory for BFS traversal
                        if (kind == DirEntryKind.Directory && remainingDepth > 1) {
                            queue.addLast(Triple(entryPath, relativePath, remainingDepth - 1))
                        }
                    }
                } catch (e: Exception) {
                    throw IllegalArgumentException("failed to read directory: ${e.message}")
                }

                // Sort entries and add to results
                dirEntries.sortBy { it.second.name }
                entries.addAll(dirEntries.map { it.second })
            }
        }

        /**
         * Format an entry name, normalizing path separators and truncating if needed.
         */
        private fun formatEntryName(path: Path): String {
            val normalized = path.toString().replace("\\", "/")
            return if (normalized.length > MAX_ENTRY_LENGTH) {
                normalized.take(MAX_ENTRY_LENGTH)
            } else {
                normalized
            }
        }

        /**
         * Format a single entry component (file/dir name).
         */
        private fun formatEntryComponent(name: String): String {
            return if (name.length > MAX_ENTRY_LENGTH) {
                name.take(MAX_ENTRY_LENGTH)
            } else {
                name
            }
        }

        /**
         * Format an entry line with indentation and type suffix.
         */
        private fun formatEntryLine(entry: DirEntry): String {
            val indent = " ".repeat(entry.depth * INDENTATION_SPACES)
            val suffix = when (entry.kind) {
                DirEntryKind.Directory -> "/"
                DirEntryKind.Symlink -> "@"
                DirEntryKind.Other -> "?"
                DirEntryKind.File -> ""
            }
            return "$indent${entry.displayName}$suffix"
        }
    }
}

/**
 * Arguments for the list_dir tool.
 */
@Serializable
private data class ListDirArgs(
    @kotlinx.serialization.SerialName("dir_path")
    val dirPath: String,
    val offset: Int = 1,
    val limit: Int = 25,
    val depth: Int = 2
)

/**
 * Internal representation of a directory entry.
 */
private data class DirEntry(
    val name: String,
    val displayName: String,
    val depth: Int,
    val kind: DirEntryKind
)

/**
 * Kind of directory entry.
 */
private enum class DirEntryKind {
    Directory,
    File,
    Symlink,
    Other
}
