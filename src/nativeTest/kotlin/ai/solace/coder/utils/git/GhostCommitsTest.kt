package ai.solace.coder.utils.git

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.test.assertIs

/**
 * Tests for GhostCommits data structures and GitOperations interface.
 * Note: Integration tests requiring actual git repos are marked separately.
 */
class GhostCommitTest {

    @Test
    fun testGhostCommitCreation() {
        val commit = GhostCommit(
            id = "abc123",
            parent = "def456",
            preexistingUntrackedFiles = listOf("file1.txt", "file2.txt"),
            preexistingUntrackedDirs = listOf("dir1", "dir2")
        )

        assertEquals("abc123", commit.id)
        assertEquals("def456", commit.parent)
        assertEquals(2, commit.preexistingUntrackedFiles.size)
        assertEquals(2, commit.preexistingUntrackedDirs.size)
    }

    @Test
    fun testGhostCommitWithNullParent() {
        val commit = GhostCommit(
            id = "first-commit",
            parent = null,
            preexistingUntrackedFiles = emptyList(),
            preexistingUntrackedDirs = emptyList()
        )

        assertEquals("first-commit", commit.id)
        assertNull(commit.parent)
        assertTrue(commit.preexistingUntrackedFiles.isEmpty())
        assertTrue(commit.preexistingUntrackedDirs.isEmpty())
    }
}

class GhostSnapshotReportTest {

    @Test
    fun testEmptyReport() {
        val report = GhostSnapshotReport()
        assertTrue(report.largeUntrackedDirs.isEmpty())
    }

    @Test
    fun testReportWithLargeDirs() {
        val report = GhostSnapshotReport(
            largeUntrackedDirs = listOf(
                LargeUntrackedDir(path = "node_modules", fileCount = 5000),
                LargeUntrackedDir(path = "build", fileCount = 250)
            )
        )

        assertEquals(2, report.largeUntrackedDirs.size)
        assertEquals("node_modules", report.largeUntrackedDirs[0].path)
        assertEquals(5000, report.largeUntrackedDirs[0].fileCount)
    }
}

class LargeUntrackedDirTest {

    @Test
    fun testLargeUntrackedDir() {
        val dir = LargeUntrackedDir(
            path = "vendor/cache",
            fileCount = 300
        )

        assertEquals("vendor/cache", dir.path)
        assertEquals(300, dir.fileCount)
    }
}

class CreateGhostCommitOptionsTest {

    @Test
    fun testDefaultOptions() {
        val options = CreateGhostCommitOptions.new("/repo/path")

        assertEquals("/repo/path", options.repoPath)
        assertNull(options.message)
        assertTrue(options.forceInclude.isEmpty())
    }

    @Test
    fun testOptionsWithMessage() {
        val options = CreateGhostCommitOptions.new("/repo/path")
            .withMessage("custom snapshot message")

        assertEquals("custom snapshot message", options.message)
    }

    @Test
    fun testOptionsWithForceInclude() {
        val options = CreateGhostCommitOptions.new("/repo/path")
            .withForceInclude(listOf(".env", "secrets.json"))

        assertEquals(2, options.forceInclude.size)
        assertTrue(options.forceInclude.contains(".env"))
        assertTrue(options.forceInclude.contains("secrets.json"))
    }

    @Test
    fun testOptionsChaining() {
        val options = CreateGhostCommitOptions.new("/repo/path")
            .withMessage("my snapshot")
            .withForceInclude(listOf("ignored.txt"))

        assertEquals("/repo/path", options.repoPath)
        assertEquals("my snapshot", options.message)
        assertEquals(1, options.forceInclude.size)
    }
}

class GitToolingErrorTest {

    @Test
    fun testNotAGitRepositoryError() {
        val error = GitToolingError.NotAGitRepository("/some/path")
        assertEquals("Not a git repository: /some/path", error.message)
        assertIs<GitToolingError.NotAGitRepository>(error)
    }

    @Test
    fun testCommandFailedError() {
        val error = GitToolingError.CommandFailed("git add failed", 128)
        assertEquals("git add failed", error.message)
        assertEquals(128, error.exitCode)
        assertIs<GitToolingError.CommandFailed>(error)
    }

    @Test
    fun testPathEscapesRepositoryError() {
        val error = GitToolingError.PathEscapesRepository("../outside.txt")
        assertEquals("Path escapes repository: ../outside.txt", error.message)
        assertIs<GitToolingError.PathEscapesRepository>(error)
    }

    @Test
    fun testIoError() {
        val error = GitToolingError.IoError("File not found")
        assertEquals("File not found", error.message)
        assertIs<GitToolingError.IoError>(error)
    }
}

class ShellGitOperationsTest {

    @Test
    fun testShellGitOperationsCanBeInstantiated() {
        val ops = ShellGitOperations()
        assertNotNull(ops)
    }

    // Note: Integration tests that actually create git repos and run commands
    // would go in a separate integration test file, as they require:
    // - A real git installation
    // - Ability to create temp directories
    // - Actual filesystem operations
}
