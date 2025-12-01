// port-lint: source exec/src/lib.rs
package ai.solace.coder.exec.process

/**
 * Platform-specific process handle
 */
expect class ProcessHandle {
    val stdout: ByteArray?
    val stderr: ByteArray?

    suspend fun onAwait(): Int
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