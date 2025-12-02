// port-lint: source core/src/spawn.rs
package ai.solace.coder.core

/**
 * Platform-specific process handle
 */
expect class ProcessHandle {
    val pid: Int
    val stdout: ByteArray? // Keep for backward compat or remove? Exec.kt uses it. I'll keep it but it might be empty if streamed.
    val stderr: ByteArray?

    suspend fun onAwait(): Int
    
    fun readStdout(buffer: ByteArray): Int
    fun readStderr(buffer: ByteArray): Int
    fun close()
    fun kill()
    fun isAlive(): Boolean
}

/**
 * Platform-specific process creation
 */
expect fun createPlatformProcess(
    program: String,
    args: List<String>,
    cwd: String,
    env: Map<String, String>
): ProcessHandle

/**
 * Platform-specific process group killing
 */
expect fun killPlatformChildProcessGroup(process: ProcessHandle)

/**
 * Platform-specific shell detection
 */
expect fun platformGetUserShellPath(): String?
expect fun platformFileExists(path: String): Boolean
expect fun platformFindInPath(binaryName: String): String?
expect fun platformIsWindows(): Boolean
expect fun platformIsMacOS(): Boolean

/**
 * Platform-specific sandbox detection
 */
expect fun platformGetSandbox(): SandboxType?

/**
 * Platform-specific macOS directory parameters
 */
expect fun platformGetMacosDirParams(): List<Pair<String, String>>