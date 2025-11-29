package ai.solace.coder.core.session

import ai.solace.coder.protocol.FileChange
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Tracks sets of changes to files and exposes the overall unified diff.
 *
 * Internally:
 * 1. Maintains an in-memory baseline snapshot of files when they are first seen.
 *    For new additions, does not create a baseline so diffs are shown as proper additions.
 * 2. Keeps a stable internal filename (uuid) per external path for rename tracking.
 * 3. To compute the aggregated unified diff, compares each baseline snapshot to the
 *    current file on disk.
 *
 * Ported from Rust codex-rs/core/src/turn_diff_tracker.rs
 */
class TurnDiffTracker {
    private val mutex = Mutex()

    /** Map external path -> internal filename (uuid). */
    private val externalToTempName = mutableMapOf<String, String>()

    /** Internal filename -> baseline file info. */
    private val baselineFileInfo = mutableMapOf<String, BaselineFileInfo>()

    /** Internal filename -> external path as of current accumulated state. */
    private val tempNameToCurrentPath = mutableMapOf<String, String>()

    /** Cache of known git worktree roots to avoid repeated filesystem walks. */
    private val gitRootCache = mutableListOf<String>()

    /**
     * Front-run apply patch calls to track the starting contents of any modified files.
     *
     * - Creates an in-memory baseline snapshot for files that already exist on disk when first seen.
     * - For additions, intentionally does not create a baseline so diffs are proper additions.
     * - Also updates internal mappings for move/rename events.
     */
    suspend fun onPatchBegin(changes: Map<String, FileChange>) {
        mutex.withLock {
            for (entry in changes.entries) {
                val path = entry.key
                val change = entry.value

                // Ensure a stable internal filename exists for this external path
                if (!externalToTempName.containsKey(path)) {
                    val internal = generateUuid()
                    externalToTempName[path] = internal
                    tempNameToCurrentPath[internal] = path

                    // Snapshot baseline if file exists
                    val baseline = createBaseline(path)
                    if (baseline != null) {
                        baselineFileInfo[internal] = baseline
                    }
                }

                // Track rename/move in current mapping if provided in an Update
                if (change is FileChange.Update && change.movePath != null) {
                    val destPath = change.movePath
                    val internal = externalToTempName[path]
                    if (internal != null) {
                        tempNameToCurrentPath[internal] = destPath
                        externalToTempName.remove(path)
                        externalToTempName[destPath] = internal
                    }
                }
            }
        }
    }

    /**
     * Compute the aggregated unified diff for all tracked changes.
     */
    suspend fun computeUnifiedDiff(): String {
        return mutex.withLock {
            val diffs = mutableListOf<String>()

            for ((internal, baseline) in baselineFileInfo) {
                val currentPath = tempNameToCurrentPath[internal] ?: continue
                val currentContent = readFileContent(currentPath)

                if (baseline.content != currentContent) {
                    val diff = computeDiff(
                        oldPath = baseline.path,
                        newPath = currentPath,
                        oldContent = baseline.content,
                        newContent = currentContent
                    )
                    if (diff.isNotEmpty()) {
                        diffs.add(diff)
                    }
                }
            }

            diffs.joinToString("\n")
        }
    }

    /**
     * Get the list of changed files.
     */
    suspend fun getChangedFiles(): List<ChangedFile> {
        return mutex.withLock {
            val result = mutableListOf<ChangedFile>()

            for ((internal, baseline) in baselineFileInfo) {
                val currentPath = tempNameToCurrentPath[internal] ?: continue
                val currentContent = readFileContent(currentPath)

                val changeType = when {
                    baseline.content.isEmpty() && currentContent.isNotEmpty() -> ChangeType.Added
                    baseline.content.isNotEmpty() && currentContent.isEmpty() -> ChangeType.Deleted
                    baseline.path != currentPath -> ChangeType.Renamed
                    baseline.content != currentContent -> ChangeType.Modified
                    else -> continue
                }

                result.add(ChangedFile(
                    path = currentPath,
                    originalPath = if (baseline.path != currentPath) baseline.path else null,
                    changeType = changeType
                ))
            }

            result
        }
    }

    /**
     * Clear all tracked changes.
     */
    suspend fun clear() {
        mutex.withLock {
            externalToTempName.clear()
            baselineFileInfo.clear()
            tempNameToCurrentPath.clear()
        }
    }

    /**
     * Check if there are any tracked changes.
     */
    suspend fun hasChanges(): Boolean {
        return mutex.withLock {
            baselineFileInfo.isNotEmpty()
        }
    }

    private fun createBaseline(path: String): BaselineFileInfo? {
        val content = readFileContent(path)
        val oid = computeGitBlobOid(content)
        return BaselineFileInfo(
            path = path,
            content = content,
            mode = FileMode.Regular,
            oid = oid
        )
    }

    private fun readFileContent(path: String): String {
        return try {
            // Platform-specific file reading would go here
            // For now, return empty string as placeholder
            ""
        } catch (e: Exception) {
            ""
        }
    }

    private fun computeDiff(
        oldPath: String,
        newPath: String,
        oldContent: String,
        newContent: String
    ): String {
        // Simple unified diff implementation
        if (oldContent == newContent) return ""

        val oldLines = oldContent.lines()
        val newLines = newContent.lines()

        return buildString {
            appendLine("--- a/$oldPath")
            appendLine("+++ b/$newPath")
            appendLine("@@ -1,${oldLines.size} +1,${newLines.size} @@")

            // Simple diff: show all old lines as removed, all new lines as added
            // A real implementation would use Myers diff algorithm
            for (line in oldLines) {
                appendLine("-$line")
            }
            for (line in newLines) {
                appendLine("+$line")
            }
        }
    }

    private fun computeGitBlobOid(content: String): String {
        // Git blob OID is SHA-1 of "blob <size>\0<content>"
        // For now, return a placeholder
        return "0000000000000000000000000000000000000000"
    }

    private fun generateUuid(): String {
        // Simple UUID generation
        val chars = "0123456789abcdef"
        return buildString {
            repeat(32) {
                append(chars.random())
                if (it == 7 || it == 11 || it == 15 || it == 19) append("-")
            }
        }
    }

    companion object {
        private const val ZERO_OID = "0000000000000000000000000000000000000000"
    }
}

/**
 * Baseline information for a file.
 */
private data class BaselineFileInfo(
    val path: String,
    val content: String,
    val mode: FileMode,
    val oid: String
)

/**
 * File mode.
 */
private enum class FileMode {
    Regular,
    Executable,
    Symlink
}

/**
 * Information about a changed file.
 */
data class ChangedFile(
    val path: String,
    val originalPath: String?,
    val changeType: ChangeType
)

/**
 * Type of file change.
 */
enum class ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed
}

/**
 * Thread-safe wrapper for TurnDiffTracker that can be shared across tasks.
 *
 * Ported from Rust codex-rs/core/src/tools/context.rs SharedTurnDiffTracker
 */
class SharedTurnDiffTracker {
    private val tracker = TurnDiffTracker()

    suspend fun onPatchBegin(changes: Map<String, FileChange>) {
        tracker.onPatchBegin(changes)
    }

    suspend fun computeUnifiedDiff(): String {
        return tracker.computeUnifiedDiff()
    }

    suspend fun getChangedFiles(): List<ChangedFile> {
        return tracker.getChangedFiles()
    }

    suspend fun clear() {
        tracker.clear()
    }

    suspend fun hasChanges(): Boolean {
        return tracker.hasChanges()
    }
}

// FileChange is imported from ai.solace.coder.protocol.FileChange
