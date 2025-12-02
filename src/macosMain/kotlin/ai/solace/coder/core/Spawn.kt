package ai.solace.coder.core

import kotlinx.cinterop.*
import platform.posix.*
import platform.Foundation.*

@OptIn(ExperimentalForeignApi::class)
private fun WIFEXITED(status: Int): Boolean = (status and 0x7f) == 0

@OptIn(ExperimentalForeignApi::class)
private fun WEXITSTATUS(status: Int): Int = (status shr 8) and 0xff

/**
 * Platform-specific process handle implementation for macOS using NSTask
 */
@OptIn(ExperimentalForeignApi::class)
actual class ProcessHandle(
    actual val pid: Int,
    private val stdoutFd: Int,
    private val stderrFd: Int,
    private val task: NSTask?
) {
    actual val stdout: ByteArray? = null // Not used when streaming
    actual val stderr: ByteArray? = null // Not used when streaming

    actual suspend fun onAwait(): Int {
        return memScoped {
            val status = alloc<IntVar>()
            waitpid(pid, status.ptr, 0)
            if (WIFEXITED(status.value)) {
                WEXITSTATUS(status.value)
            } else {
                -1
            }
        }
    }

    actual fun readStdout(buffer: ByteArray): Int {
        if (stdoutFd == -1) return -1
        return buffer.usePinned { pinned ->
            read(stdoutFd, pinned.addressOf(0), buffer.size.toULong()).toInt()
        }
    }

    actual fun readStderr(buffer: ByteArray): Int {
        if (stderrFd == -1) return -1
        return buffer.usePinned { pinned ->
            read(stderrFd, pinned.addressOf(0), buffer.size.toULong()).toInt()
        }
    }

    actual fun close() {
        if (stdoutFd != -1) close(stdoutFd)
        if (stderrFd != -1) close(stderrFd)
    }

    actual fun kill() {
        task?.terminate()
    }

    actual fun isAlive(): Boolean {
        return task?.isRunning ?: false
    }
}

/**
 * Platform-specific process creation for macOS using NSTask
 */
@OptIn(ExperimentalForeignApi::class)
actual fun createPlatformProcess(
    program: String,
    args: List<String>,
    cwd: String,
    env: Map<String, String>
): ProcessHandle {
    val task = NSTask()
    
    // Use executableURL for newer macOS versions if possible, but launchPath is still available
    // task.executableURL = NSURL.fileURLWithPath(program)
    task.launchPath = program
    task.arguments = args
    task.currentDirectoryPath = cwd
    task.environment = env as Map<Any?, *>
    
    val stdoutPipe = NSPipe()
    val stderrPipe = NSPipe()
    task.standardOutput = stdoutPipe
    task.standardError = stderrPipe
    
    task.launch()
    
    val pid = task.processIdentifier
    val stdoutFd = stdoutPipe.fileHandleForReading.fileDescriptor
    val stderrFd = stderrPipe.fileHandleForReading.fileDescriptor
    
    return ProcessHandle(pid, stdoutFd, stderrFd, task)
}

/**
 * Platform-specific process group killing for macOS
 */
@OptIn(ExperimentalForeignApi::class)
actual fun killPlatformChildProcessGroup(process: ProcessHandle) {
    process.kill()
}

/**
 * Platform-specific shell detection for macOS
 */
@OptIn(ExperimentalForeignApi::class)
actual fun platformGetUserShellPath(): String? {
    return getenv("SHELL")?.toKString() ?: "/bin/zsh"
}

@OptIn(ExperimentalForeignApi::class)
actual fun platformFileExists(path: String): Boolean {
    return access(path, F_OK) == 0
}

@OptIn(ExperimentalForeignApi::class)
actual fun platformFindInPath(binaryName: String): String? {
    val pathEnv = getenv("PATH")?.toKString() ?: return null
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
actual fun platformIsMacOS(): Boolean = true

/**
 * Platform-specific sandbox detection for macOS
 */
actual fun platformGetSandbox(): SandboxType? {
    return SandboxType.None
}

/**
 * Platform-specific macOS directory parameters
 */
@OptIn(ExperimentalForeignApi::class)
actual fun platformGetMacosDirParams(): List<Pair<String, String>> {
    return listOf(
        "HOME" to (getenv("HOME")?.toKString() ?: "/Users/unknown"),
        "DARWIN_USER_CACHE_DIR" to (getenv("DARWIN_USER_CACHE_DIR")?.toKString() ?: "/tmp")
    )
}
