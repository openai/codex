package ai.solace.coder.core

import kotlinx.cinterop.CPointer
import kotlinx.cinterop.alloc
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.ptr
import kotlinx.cinterop.toKString
import kotlinx.cinterop.value
import platform.posix.FILE
import platform.posix.SIGKILL
import platform.posix.access
import platform.posix.kill
import platform.posix.waitpid
import platform.posix.WEXITSTATUS
import platform.posix.WIFEXITED
import platform.posix.F_OK
import platform.posix.X_OK

/**
 * Platform-specific process handle implementation for Linux
 */
actual class ProcessHandle(
    actual val pid: Int,
    private val stdoutFd: Int,
    private val stderrFd: Int
) {
    actual val stdout: ByteArray? = null
    actual val stderr: ByteArray? = null

    actual suspend fun onAwait(): Int {
        return memScoped {
            val status = alloc<kotlinx.cinterop.IntVar>()
            waitpid(pid, status.ptr, 0)
            if (WIFEXITED(status.value) != 0) {
                WEXITSTATUS(status.value)
            } else {
                -1
            }
        }
    }

    actual fun readStdout(buffer: ByteArray): Int = -1
    actual fun readStderr(buffer: ByteArray): Int = -1
    actual fun close() {}
    actual fun kill() { kill(pid, SIGKILL) }
    actual fun isAlive(): Boolean = kill(pid, 0) == 0
}

actual fun createPlatformProcess(
    program: String,
    args: List<String>,
    cwd: String,
    env: Map<String, String>
): ProcessHandle {
    return ProcessHandle(0, -1, -1)
}

actual fun killPlatformChildProcessGroup(process: ProcessHandle) {
    kill(process.pid, SIGKILL)
}

actual fun platformGetUserShellPath(): String? {
    return platform.posix.getenv("SHELL")?.toKString() ?: "/bin/bash"
}

actual fun platformFileExists(path: String): Boolean {
    return access(path, F_OK) == 0
}

actual fun platformFindInPath(binaryName: String): String? {
    val pathEnv = platform.posix.getenv("PATH")?.toKString() ?: return null
    val paths = pathEnv.split(":")
    for (dir in paths) {
        val fullPath = "$dir/$binaryName"
        if (access(fullPath, X_OK) == 0) {
            return fullPath
        }
    }
    return null
}

actual fun platformIsWindows(): Boolean = false
actual fun platformIsMacOS(): Boolean = false

actual fun platformGetSandbox(): SandboxType? {
    return SandboxType.None
}

actual fun platformGetMacosDirParams(): List<Pair<String, String>> {
    return emptyList()
}
