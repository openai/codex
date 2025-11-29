package ai.solace.coder.utils.git

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.toKString
import kotlinx.serialization.Serializable
import platform.posix.ENOENT
import platform.posix.S_IFDIR
import platform.posix.S_IFMT
import platform.posix.errno
import platform.posix.getenv
import platform.posix.stat

/**
 * Default commit message used for ghost commits when none is provided.
 */
private const val DEFAULT_COMMIT_MESSAGE = "codex snapshot"

/**
 * Default threshold that triggers a warning about large untracked directories.
 */
private const val LARGE_UNTRACKED_WARNING_THRESHOLD = 200

/**
 * Options to control ghost commit creation.
 */
data class CreateGhostCommitOptions(
    val repoPath: String,
    val message: String? = null,
    val forceInclude: List<String> = emptyList()
) {
    companion object {
        fun new(repoPath: String): CreateGhostCommitOptions {
            return CreateGhostCommitOptions(repoPath)
        }
    }

    fun withMessage(message: String): CreateGhostCommitOptions {
        return copy(message = message)
    }

    fun withForceInclude(paths: List<String>): CreateGhostCommitOptions {
        return copy(forceInclude = paths)
    }
}

/**
 * A ghost commit capturing the state of the repository's working tree.
 */
@Serializable
data class GhostCommit(
    val id: String,
    val parent: String?,
    val preexistingUntrackedFiles: List<String>,
    val preexistingUntrackedDirs: List<String>
)

/**
 * Summary produced alongside a ghost snapshot.
 */
@Serializable
data class GhostSnapshotReport(
    val largeUntrackedDirs: List<LargeUntrackedDir> = emptyList()
)

/**
 * Directory containing a large amount of untracked content.
 */
@Serializable
data class LargeUntrackedDir(
    val path: String,
    val fileCount: Int
)

/**
 * Errors that can occur during git operations.
 */
sealed class GitToolingError : Exception() {
    data class NotAGitRepository(val path: String) : GitToolingError() {
        override val message: String get() = "Not a git repository: $path"
    }
    data class CommandFailed(override val message: String, val exitCode: Int) : GitToolingError()
    data class PathEscapesRepository(val path: String) : GitToolingError() {
        override val message: String get() = "Path escapes repository: $path"
    }
    data class IoError(override val message: String) : GitToolingError()
}

/**
 * Interface for git operations - allows for testing with mocks.
 */
interface GitOperations {
    /**
     * Create a ghost commit capturing the current state of the working tree.
     */
    suspend fun createGhostCommit(options: CreateGhostCommitOptions): Result<GhostCommit>

    /**
     * Create a ghost commit and return both the commit and a snapshot report.
     */
    suspend fun createGhostCommitWithReport(
        options: CreateGhostCommitOptions
    ): Result<Pair<GhostCommit, GhostSnapshotReport>>

    /**
     * Restore the working tree to match the provided ghost commit.
     */
    suspend fun restoreGhostCommit(repoPath: String, commit: GhostCommit): Result<Unit>

    /**
     * Compute a report describing the working tree without creating a commit.
     */
    suspend fun captureGhostSnapshotReport(options: CreateGhostCommitOptions): Result<GhostSnapshotReport>
}

/**
 * Shell wrapper implementation of GitOperations.
 * Executes git commands via subprocess calls.
 */
class ShellGitOperations : GitOperations {

    override suspend fun createGhostCommit(options: CreateGhostCommitOptions): Result<GhostCommit> {
        return createGhostCommitWithReport(options).map { (commit, _) -> commit }
    }

    override suspend fun createGhostCommitWithReport(
        options: CreateGhostCommitOptions
    ): Result<Pair<GhostCommit, GhostSnapshotReport>> {
        return runCatching {
            ensureGitRepository(options.repoPath)

            val repoRoot = resolveRepositoryRoot(options.repoPath)
            val repoPrefix = repoSubdir(repoRoot, options.repoPath)
            val parent = resolveHead(repoRoot)
            val existingUntracked = captureExistingUntracked(repoRoot, repoPrefix)

            val warningFiles = existingUntracked.files.map { path ->
                toSessionRelativePath(path, repoPrefix)
            }
            val warningDirs = existingUntracked.dirs.map { path ->
                toSessionRelativePath(path, repoPrefix)
            }
            val largeUntrackedDirs = detectLargeUntrackedDirs(warningFiles, warningDirs)

            // Validate force_include paths don't escape repository
            for (path in options.forceInclude) {
                if (path.contains("..")) {
                    throw GitToolingError.PathEscapesRepository(path)
                }
            }

            // Create temporary index file
            val tempIndexPath = createTempIndexPath()
            try {
                val baseEnv = mapOf("GIT_INDEX_FILE" to tempIndexPath)

                // Pre-populate temporary index with HEAD (if exists)
                parent?.let { parentSha ->
                    runGitCommand(
                        repoRoot,
                        listOf("read-tree", parentSha),
                        baseEnv
                    )
                }

                // Add all files to temporary index
                val addArgs = mutableListOf("add", "--all")
                repoPrefix?.let { prefix ->
                    addArgs.addAll(listOf("--", prefix))
                }
                runGitCommand(repoRoot, addArgs, baseEnv)

                // Force-add any specified paths (e.g., ignored files)
                if (options.forceInclude.isNotEmpty()) {
                    val forceArgs = mutableListOf("add", "--force")
                    forceArgs.addAll(options.forceInclude.map { path ->
                        repoPrefix?.let { "$it/$path" } ?: path
                    })
                    runGitCommand(repoRoot, forceArgs, baseEnv)
                }

                // Write tree from temporary index
                val treeId = runGitCommandForOutput(repoRoot, listOf("write-tree"), baseEnv).trim()

                // Create commit
                val commitEnv = baseEnv + defaultCommitIdentity()
                val message = options.message ?: DEFAULT_COMMIT_MESSAGE
                val commitArgs = mutableListOf("commit-tree", treeId)
                parent?.let { parentSha ->
                    commitArgs.addAll(listOf("-p", parentSha))
                }
                commitArgs.addAll(listOf("-m", message))

                val commitId = runGitCommandForOutput(repoRoot, commitArgs, commitEnv).trim()

                val ghostCommit = GhostCommit(
                    id = commitId,
                    parent = parent,
                    preexistingUntrackedFiles = existingUntracked.files,
                    preexistingUntrackedDirs = existingUntracked.dirs
                )

                val report = GhostSnapshotReport(largeUntrackedDirs = largeUntrackedDirs)
                Pair(ghostCommit, report)
            } finally {
                // Clean up temporary index file
                deleteTempFile(tempIndexPath)
            }
        }
    }

    override suspend fun restoreGhostCommit(repoPath: String, commit: GhostCommit): Result<Unit> {
        return runCatching {
            ensureGitRepository(repoPath)

            val repoRoot = resolveRepositoryRoot(repoPath)
            val repoPrefix = repoSubdir(repoRoot, repoPath)
            val currentUntracked = captureExistingUntracked(repoRoot, repoPrefix)

            // Restore working tree and index from ghost commit
            restoreToCommitInner(repoRoot, repoPrefix, commit.id)

            // Remove untracked files that were created after the snapshot
            removeNewUntracked(
                repoRoot,
                commit.preexistingUntrackedFiles,
                commit.preexistingUntrackedDirs,
                currentUntracked
            )
        }
    }

    override suspend fun captureGhostSnapshotReport(options: CreateGhostCommitOptions): Result<GhostSnapshotReport> {
        return runCatching {
            ensureGitRepository(options.repoPath)

            val repoRoot = resolveRepositoryRoot(options.repoPath)
            val repoPrefix = repoSubdir(repoRoot, options.repoPath)
            val existingUntracked = captureExistingUntracked(repoRoot, repoPrefix)

            val warningFiles = existingUntracked.files.map { path ->
                toSessionRelativePath(path, repoPrefix)
            }
            val warningDirs = existingUntracked.dirs.map { path ->
                toSessionRelativePath(path, repoPrefix)
            }

            GhostSnapshotReport(
                largeUntrackedDirs = detectLargeUntrackedDirs(warningFiles, warningDirs)
            )
        }
    }

    // ---- Internal helper functions ----

    private fun ensureGitRepository(path: String) {
        val result = runGitCommandForExitCode(path, listOf("rev-parse", "--git-dir"))
        if (result != 0) {
            throw GitToolingError.NotAGitRepository(path)
        }
    }

    private fun resolveRepositoryRoot(path: String): String {
        return runGitCommandForOutput(path, listOf("rev-parse", "--show-toplevel")).trim()
    }

    private fun resolveHead(repoRoot: String): String? {
        val exitCode = runGitCommandForExitCode(repoRoot, listOf("rev-parse", "HEAD"))
        if (exitCode != 0) {
            return null
        }
        return runGitCommandForOutput(repoRoot, listOf("rev-parse", "HEAD")).trim()
    }

    private fun repoSubdir(repoRoot: String, sessionPath: String): String? {
        val normalizedRoot = normalizePath(repoRoot)
        val normalizedSession = normalizePath(sessionPath)
        if (normalizedRoot == normalizedSession) {
            return null
        }
        if (normalizedSession.startsWith(normalizedRoot)) {
            return normalizedSession.removePrefix(normalizedRoot).trimStart('/')
        }
        return null
    }

    private fun normalizePath(path: String): String {
        // Simple normalization - remove trailing slashes and resolve . and ..
        return path.trimEnd('/')
    }

    private fun toSessionRelativePath(path: String, repoPrefix: String?): String {
        return repoPrefix?.let { prefix ->
            if (path.startsWith(prefix)) {
                path.removePrefix(prefix).trimStart('/')
            } else {
                path
            }
        } ?: path
    }

    private data class UntrackedSnapshot(
        val files: List<String>,
        val dirs: List<String>
    )

    private fun captureExistingUntracked(repoRoot: String, repoPrefix: String?): UntrackedSnapshot {
        val args = mutableListOf(
            "status",
            "--porcelain=2",
            "-z",
            "--ignored=matching",
            "--untracked-files=all"
        )
        repoPrefix?.let { prefix ->
            args.addAll(listOf("--", prefix))
        }

        val output = runGitCommandForOutput(repoRoot, args)
        if (output.isEmpty()) {
            return UntrackedSnapshot(emptyList(), emptyList())
        }

        val files = mutableListOf<String>()
        val dirs = mutableListOf<String>()

        // Parse porcelain v2 output (null-delimited)
        for (entry in output.split('\u0000')) {
            if (entry.isEmpty()) continue

            val parts = entry.split(' ', limit = 2)
            if (parts.size < 2) continue

            val code = parts[0]
            val pathPart = parts[1]

            // Only interested in untracked (?) or ignored (!) entries
            if (code != "?" && code != "!") continue
            if (pathPart.isEmpty()) continue

            val absolutePath = "$repoRoot/$pathPart"
            val isDir = isDirectory(absolutePath)

            if (isDir) {
                dirs.add(pathPart)
            } else {
                files.add(pathPart)
            }
        }

        return UntrackedSnapshot(files, dirs)
    }

    private fun detectLargeUntrackedDirs(files: List<String>, dirs: List<String>): List<LargeUntrackedDir> {
        val counts = mutableMapOf<String, Int>()

        // Sort directories by depth (deepest first)
        val sortedDirs = dirs.sortedByDescending { it.count { c -> c == '/' } }

        for (file in files) {
            var key: String? = null
            for (dir in sortedDirs) {
                if (file.startsWith("$dir/") || file == dir) {
                    key = dir
                    break
                }
            }
            if (key == null) {
                // Use parent directory of file
                key = file.substringBeforeLast('/', ".")
            }
            counts[key] = (counts[key] ?: 0) + 1
        }

        return counts
            .filter { (_, count) -> count >= LARGE_UNTRACKED_WARNING_THRESHOLD }
            .map { (path, count) -> LargeUntrackedDir(path, count) }
            .sortedByDescending { it.fileCount }
    }

    private fun restoreToCommitInner(repoRoot: String, repoPrefix: String?, commitId: String) {
        val args = mutableListOf(
            "restore",
            "--source", commitId,
            "--worktree",
            "--staged",
            "--"
        )
        args.add(repoPrefix ?: ".")

        runGitCommand(repoRoot, args)
    }

    private fun removeNewUntracked(
        repoRoot: String,
        preservedFiles: List<String>,
        preservedDirs: List<String>,
        current: UntrackedSnapshot
    ) {
        if (current.files.isEmpty() && current.dirs.isEmpty()) return

        val preservedFileSet = preservedFiles.toSet()

        for (path in current.files) {
            if (shouldPreserve(path, preservedFileSet, preservedDirs)) continue
            deletePath("$repoRoot/$path")
        }

        for (dir in current.dirs) {
            if (shouldPreserve(dir, preservedFileSet, preservedDirs)) continue
            deletePath("$repoRoot/$dir")
        }
    }

    private fun shouldPreserve(
        path: String,
        preservedFiles: Set<String>,
        preservedDirs: List<String>
    ): Boolean {
        if (preservedFiles.contains(path)) return true
        return preservedDirs.any { dir -> path.startsWith("$dir/") || path == dir }
    }

    private fun defaultCommitIdentity(): Map<String, String> {
        return mapOf(
            "GIT_AUTHOR_NAME" to "Codex Snapshot",
            "GIT_AUTHOR_EMAIL" to "snapshot@codex.local",
            "GIT_COMMITTER_NAME" to "Codex Snapshot",
            "GIT_COMMITTER_EMAIL" to "snapshot@codex.local"
        )
    }

    // ---- Process execution helpers ----

    private fun runGitCommand(
        cwd: String,
        args: List<String>,
        extraEnv: Map<String, String> = emptyMap()
    ) {
        val exitCode = runGitCommandForExitCode(cwd, args, extraEnv)
        if (exitCode != 0) {
            throw GitToolingError.CommandFailed("git ${args.joinToString(" ")} failed", exitCode)
        }
    }

    private fun runGitCommandForOutput(
        cwd: String,
        args: List<String>,
        extraEnv: Map<String, String> = emptyMap()
    ): String {
        return executeGitCommand(cwd, args, extraEnv).first
    }

    private fun runGitCommandForExitCode(
        cwd: String,
        args: List<String>,
        extraEnv: Map<String, String> = emptyMap()
    ): Int {
        return executeGitCommand(cwd, args, extraEnv).second
    }

    /**
     * Execute git command and return (stdout, exitCode) pair.
     * This is the core execution function that calls the actual git binary.
     */
    private fun executeGitCommand(
        cwd: String,
        args: List<String>,
        extraEnv: Map<String, String>
    ): Pair<String, Int> {
        // Use platform-specific process execution
        return platformExecuteGit(cwd, args, extraEnv)
    }

    // ---- Platform/file helpers (expect/actual pattern for full implementation) ----

    @OptIn(ExperimentalForeignApi::class)
    private fun createTempIndexPath(): String {
        // Create a unique temp file path for the git index
        val tempDir = getenv("TMPDIR")?.toKString() ?: "/tmp"
        return "$tempDir/codex-git-index-${kotlin.random.Random.nextLong()}"
    }

    @OptIn(ExperimentalForeignApi::class)
    private fun isDirectory(path: String): Boolean {
        // Ported from Rust ghost_commits.rs symlink_metadata behavior:
        // If stat fails with ENOENT, treat as "not a directory" (return false)
        // Log other errors but still return false for safety
        return try {
            platformIsDirectory(path)
        } catch (e: Exception) {
            // Log unexpected errors (not "file not found")
            if (!e.message.orEmpty().contains("ENOENT") &&
                !e.message.orEmpty().contains("No such file")) {
                println("WARN: isDirectory check failed for '$path': ${e.message}")
            }
            false
        }
    }

    @OptIn(ExperimentalForeignApi::class)
    private fun deleteTempFile(path: String) {
        // Ported from Rust ghost_commits.rs remove_path behavior:
        // Only ignore ENOENT (file not found), log other errors
        val result = platform.posix.remove(path)
        if (result != 0) {
            val err = errno
            if (err != ENOENT) {
                println("WARN: failed to delete temp file '$path': errno=$err")
            }
        }
    }

    private fun deletePath(path: String) {
        // Ported from Rust ghost_commits.rs remove_path behavior:
        // Only ignore NotFound errors, log other errors
        // Use rm -rf for recursive directory deletion
        val exitCode = platformExecuteCommand(listOf("rm", "-rf", path))
        if (exitCode != 0) {
            // rm -rf typically succeeds even if path doesn't exist,
            // so a non-zero exit indicates a real error
            println("WARN: failed to delete path '$path': exit code $exitCode")
        }
    }
}

// ---- Platform-specific execution (expect/actual pattern) ----

/**
 * Execute git command using platform-specific process APIs.
 * Returns (stdout, exitCode) pair.
 */
internal expect fun platformExecuteGit(
    cwd: String,
    args: List<String>,
    extraEnv: Map<String, String>
): Pair<String, Int>

/**
 * Execute a general command using platform-specific process APIs.
 */
internal expect fun platformExecuteCommand(args: List<String>): Int

/**
 * Check if a path is a directory using platform-specific APIs.
 */
internal expect fun platformIsDirectory(path: String): Boolean
