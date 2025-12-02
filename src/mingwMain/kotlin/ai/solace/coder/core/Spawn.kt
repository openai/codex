package ai.solace.coder.core

/**
 * Platform-specific process handle implementation for Windows
 */
actual class ProcessHandle(actual val pid: Int) {
    actual val stdout: ByteArray? = null
    actual val stderr: ByteArray? = null

    actual suspend fun onAwait(): Int {
        return -1
    }

    actual fun readStdout(buffer: ByteArray): Int = -1
    actual fun readStderr(buffer: ByteArray): Int = -1
    actual fun close() {}
    actual fun kill() {}
    actual fun isAlive(): Boolean = false
}

actual fun createPlatformProcess(
    program: String,
    args: List<String>,
    cwd: String,
    env: Map<String, String>
): ProcessHandle {
    return ProcessHandle(0)
}

actual fun killPlatformChildProcessGroup(process: ProcessHandle) {
}

actual fun platformGetUserShellPath(): String? {
    return "cmd.exe"
}

actual fun platformFileExists(path: String): Boolean {
    return false
}

actual fun platformFindInPath(binaryName: String): String? {
    return null
}

actual fun platformIsWindows(): Boolean = true
actual fun platformIsMacOS(): Boolean = false

actual fun platformGetSandbox(): SandboxType? {
    return SandboxType.None
}

actual fun platformGetMacosDirParams(): List<Pair<String, String>> {
    return emptyList()
}
