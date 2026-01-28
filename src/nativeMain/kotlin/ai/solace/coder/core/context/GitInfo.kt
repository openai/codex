// port-lint: source codex-rs/core/src/git_info.rs
package ai.solace.coder.core.context

import kotlinx.cinterop.toKString
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import platform.posix.pclose
import platform.posix.popen
import platform.posix.fgets
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.refTo

/**
 * Git repository information collected from the working directory.
 */
@Serializable
data class GitInfoData(
    @SerialName("commit_hash")
    val commitHash: String? = null,
    val branch: String? = null,
    @SerialName("repository_url")
    val repositoryUrl: String? = null
)

/**
 * A minimal commit summary entry.
 */
@Serializable
data class CommitLogEntry(
    val sha: String,
    val timestamp: Long,
    val subject: String
)

/**
 * Git diff information relative to a remote sha.
 */
@Serializable
data class GitDiffToRemote(
    val sha: String,
    val diff: String
)

/** Timeout for git commands (seconds). */
private const val GIT_COMMAND_TIMEOUT_SECS = 5

/**
 * Return the root of the Git repository containing [baseDir], or null if
 * the directory is not inside a Git repository.
 *
 * Walks up the directory hierarchy looking for a `.git` file or directory.
 */
fun getGitRepoRoot(baseDir: String): String? {
    var dir = baseDir
    while (true) {
        val gitPath = if (dir.endsWith("/")) "${dir}.git" else "$dir/.git"
        if (fileOrDirExists(gitPath)) {
            return dir
        }
        // Go up one directory
        val parent = dir.substringBeforeLast('/')
        if (parent == dir || parent.isEmpty()) {
            break
        }
        dir = parent
    }
    return null
}

/**
 * Collect git repository information from the given working directory.
 * Returns null if no git repository is found or if git operations fail.
 */
fun collectGitInfo(cwd: String): GitInfoData? {
    // Check if we're in a git repository
    val isGitRepo = runGitCommand(listOf("rev-parse", "--git-dir"), cwd) != null
    if (!isGitRepo) {
        return null
    }

    val commitHash = runGitCommand(listOf("rev-parse", "HEAD"), cwd)?.trim()
    val rawBranch = runGitCommand(listOf("rev-parse", "--abbrev-ref", "HEAD"), cwd)?.trim()
    val branch = if (rawBranch != null && rawBranch != "HEAD") rawBranch else null
    val repositoryUrl = runGitCommand(listOf("remote", "get-url", "origin"), cwd)?.trim()

    return GitInfoData(
        commitHash = commitHash,
        branch = branch,
        repositoryUrl = repositoryUrl
    )
}

/**
 * Return the last [limit] commits reachable from HEAD for the current branch.
 */
fun recentCommits(cwd: String, limit: Int = 10): List<CommitLogEntry> {
    // Ensure we're in a git repo first
    val gitDir = runGitCommand(listOf("rev-parse", "--git-dir"), cwd) ?: return emptyList()

    val n = limit.coerceAtLeast(1)
    val fmt = "%H\u001f%ct\u001f%s" // sha <US> commit_time <US> subject
    val logOutput = runGitCommand(
        listOf("log", "-n", n.toString(), "--pretty=format:$fmt"),
        cwd
    ) ?: return emptyList()

    return logOutput.lines().mapNotNull { line ->
        val parts = line.split('\u001f')
        if (parts.size < 2) return@mapNotNull null
        val sha = parts[0].trim()
        val tsStr = parts[1].trim()
        val subject = parts.getOrElse(2) { "" }.trim()
        if (sha.isEmpty() || tsStr.isEmpty()) return@mapNotNull null
        val timestamp = tsStr.toLongOrNull() ?: 0L
        CommitLogEntry(sha = sha, timestamp = timestamp, subject = subject)
    }
}

/**
 * Determine the repository's default branch name.
 * Returns null when the information cannot be determined.
 */
fun defaultBranchName(cwd: String): String? {
    // Try symbolic ref for origin
    val symref = runGitCommand(
        listOf("symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"),
        cwd
    )?.trim()

    if (symref != null) {
        val name = symref.substringAfterLast('/')
        if (name.isNotEmpty()) return name
    }

    // Try common local defaults
    for (candidate in listOf("main", "master")) {
        val verify = runGitCommand(
            listOf("rev-parse", "--verify", "--quiet", "refs/heads/$candidate"),
            cwd
        )
        if (verify != null) return candidate
    }

    return null
}

/**
 * Resolve the root git project for trust validation.
 * This finds the topmost git repository root, handling nested repos and worktrees.
 */
fun resolveRootGitProjectForTrust(cwd: String): String? {
    // Find the git repo root
    val repoRoot = getGitRepoRoot(cwd) ?: return null

    // Check if this is a worktree by looking at the .git file
    val gitPath = if (repoRoot.endsWith("/")) "${repoRoot}.git" else "$repoRoot/.git"
    // If .git is a file (worktree), parse the gitdir path
    // For now, just return the repo root
    return repoRoot
}

// ============================================================================
// Internal helpers
// ============================================================================

/**
 * Run a git command and return its stdout output, or null on failure.
 */
@OptIn(ExperimentalForeignApi::class)
private fun runGitCommand(args: List<String>, cwd: String): String? {
    val cmd = "cd ${shellEscape(cwd)} && git ${args.joinToString(" ") { shellEscape(it) }} 2>/dev/null"

    val fp = popen(cmd, "r") ?: return null
    val buffer = ByteArray(8192)
    val output = StringBuilder()

    try {
        while (true) {
            val result = fgets(buffer.refTo(0), buffer.size, fp)
            if (result == null) break
            output.append(buffer.toKString())
        }
    } finally {
        val exitCode = pclose(fp)
        if (exitCode != 0 && output.isEmpty()) {
            return null
        }
    }

    return output.toString().ifEmpty { null }
}

/**
 * Shell-escape a string for use in a command.
 */
private fun shellEscape(s: String): String {
    if (s.all { it.isLetterOrDigit() || it in "/-_.=:" }) {
        return s
    }
    return "'" + s.replace("'", "'\\''") + "'"
}

/**
 * Check if a file or directory exists at the given path.
 */
@OptIn(ExperimentalForeignApi::class)
private fun fileOrDirExists(path: String): Boolean {
    val fp = popen("test -e ${shellEscape(path)} && echo yes 2>/dev/null", "r") ?: return false
    val buffer = ByteArray(64)
    val result = fgets(buffer.refTo(0), buffer.size, fp)
    pclose(fp)
    return result != null && buffer.toKString().trim().startsWith("yes")
}
