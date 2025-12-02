@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)

// port-lint: source core/src/shell.rs
package ai.solace.coder.exec.shell

import kotlinx.cinterop.toKString
import platform.posix.getenv
import platform.posix.access
import platform.posix.F_OK

/**
 * Shell types supported by the system
 */
enum class ShellType {
    Zsh,
    Bash,
    PowerShell,
    Sh,
    Cmd
}

/**
 * Shell configuration with type and path
 */
data class Shell(
    val shellType: ShellType,
    val shellPath: String
) {
    /**
     * Get the shell name
     */
    fun name(): String = when (shellType) {
        ShellType.Zsh -> "zsh"
        ShellType.Bash -> "bash"
        ShellType.PowerShell -> "powershell"
        ShellType.Sh -> "sh"
        ShellType.Cmd -> "cmd"
    }

    /**
     * Derive execution arguments for running a command in this shell
     */
    fun deriveExecArgs(command: String, useLoginShell: Boolean): List<String> {
        return when (shellType) {
            ShellType.Zsh, ShellType.Bash, ShellType.Sh -> {
                val arg = if (useLoginShell) "-lc" else "-c"
                listOf(shellPath, arg, command)
            }
            ShellType.PowerShell -> {
                val args = mutableListOf(shellPath)
                if (!useLoginShell) {
                    args.add("-NoProfile")
                }
                args.add("-Command")
                args.add(command)
                args
            }
            ShellType.Cmd -> {
                listOf(shellPath, "/c", command)
            }
        }
    }
}

/**
 * Shell detector with platform-specific implementations
 */
class ShellDetector {
    companion object {
        private const val DEFAULT_ZSH_PATH = "/bin/zsh"
        private const val DEFAULT_BASH_PATH = "/bin/bash"
        private const val DEFAULT_SH_PATH = "/bin/sh"
        private const val DEFAULT_CMD_PATH = "cmd.exe"
        private const val DEFAULT_POWERSHELL_PATH = "powershell.exe"
        private const val DEFAULT_PWSH_PATH = "pwsh.exe"
    }

    /**
     * Get the default user shell
     */
    fun defaultUserShell(): Shell {
        val userShellPath = getUserShellPath()
        return defaultUserShellFromPath(userShellPath)
    }

    /**
     * Get shell by type with optional path
     */
    fun getShell(shellType: ShellType, path: String? = null): Shell? {
        return when (shellType) {
            ShellType.Zsh -> getZshShell(path)
            ShellType.Bash -> getBashShell(path)
            ShellType.PowerShell -> getPowerShellShell(path)
            ShellType.Sh -> getShShell(path)
            ShellType.Cmd -> getCmdShell(path)
        }
    }

    /**
     * Detect shell type from path
     */
    fun detectShellType(shellPath: String): ShellType? {
        val fileName = shellPath.substringAfterLast('/').substringAfterLast('\\')
            .removeSuffix(".exe")
            .lowercase()

        return when (fileName) {
            "zsh" -> ShellType.Zsh
            "bash" -> ShellType.Bash
            "pwsh", "powershell" -> ShellType.PowerShell
            "sh" -> ShellType.Sh
            "cmd" -> ShellType.Cmd
            else -> null
        }
    }

    /**
     * Get shell by model-provided path
     */
    fun getShellByModelProvidedPath(shellPath: String): Shell {
        val shellType = detectShellType(shellPath)
        return shellType?.let { getShell(it, shellPath) }
            ?: ultimateFallbackShell()
    }

    /**
     * Get user shell path from system
     */
    private fun getUserShellPath(): String? {
        return platformGetUserShellPath()
    }

    /**
     * Get default shell from path with fallbacks
     */
    private fun defaultUserShellFromPath(userShellPath: String?): Shell {
        if (isWindows()) {
            return getShell(ShellType.PowerShell) ?: ultimateFallbackShell()
        }

        val userDefaultShell = userShellPath?.let { path ->
            detectShellType(path)?.let { shellType ->
                getShell(shellType)
            }
        }

        val shellWithFallback = if (isMacOS()) {
            userDefaultShell
                ?: getShell(ShellType.Zsh)
                ?: getShell(ShellType.Bash)
        } else {
            userDefaultShell
                ?: getShell(ShellType.Bash)
                ?: getShell(ShellType.Zsh)
        }

        return shellWithFallback ?: ultimateFallbackShell()
    }

    /**
     * Get Zsh shell
     */
    private fun getZshShell(path: String?): Shell? {
        val shellPath = getShellPath(ShellType.Zsh, path, "zsh", listOf(DEFAULT_ZSH_PATH))
        return shellPath?.let { Shell(ShellType.Zsh, it) }
    }

    /**
     * Get Bash shell
     */
    private fun getBashShell(path: String?): Shell? {
        val shellPath = getShellPath(ShellType.Bash, path, "bash", listOf(DEFAULT_BASH_PATH))
        return shellPath?.let { Shell(ShellType.Bash, it) }
    }

    /**
     * Get Sh shell
     */
    private fun getShShell(path: String?): Shell? {
        val shellPath = getShellPath(ShellType.Sh, path, "sh", listOf(DEFAULT_SH_PATH))
        return shellPath?.let { Shell(ShellType.Sh, it) }
    }

    /**
     * Get PowerShell shell
     */
    private fun getPowerShellShell(path: String?): Shell? {
        val pwshPath = getShellPath(
            ShellType.PowerShell,
            path,
            "pwsh",
            listOf("/usr/local/bin/pwsh")
        )
        val powershellPath = getShellPath(
            ShellType.PowerShell,
            path,
            "powershell",
            listOf(DEFAULT_POWERSHELL_PATH)
        )
        
        val shellPath = pwshPath ?: powershellPath
        return shellPath?.let { Shell(ShellType.PowerShell, it) }
    }

    /**
     * Get Cmd shell
     */
    private fun getCmdShell(path: String?): Shell? {
        val shellPath = getShellPath(ShellType.Cmd, path, "cmd", listOf(DEFAULT_CMD_PATH))
        return shellPath?.let { Shell(ShellType.Cmd, it) }
    }

    /**
     * Get shell path with fallbacks
     */
    private fun getShellPath(
        shellType: ShellType,
        providedPath: String?,
        binaryName: String,
        fallbackPaths: List<String>
    ): String? {
        // If exact provided path exists, use it
        if (providedPath != null && fileExists(providedPath)) {
            return providedPath
        }

        // Check if the shell we are trying to load is user's default shell
        val defaultShellPath = getUserShellPath()
        if (defaultShellPath != null && detectShellType(defaultShellPath) == shellType) {
            return defaultShellPath
        }

        // Try to find in PATH
        val pathInPath = findInPath(binaryName)
        if (pathInPath != null) {
            return pathInPath
        }

        // Try fallback paths
        for (path in fallbackPaths) {
            if (fileExists(path)) {
                return path
            }
        }

        return null
    }

    /**
     * Ultimate fallback shell
     */
    private fun ultimateFallbackShell(): Shell {
        return if (isWindows()) {
            Shell(ShellType.Cmd, DEFAULT_CMD_PATH)
        } else {
            Shell(ShellType.Sh, DEFAULT_SH_PATH)
        }
    }

    /**
     * Check if file exists
     */
    private fun fileExists(path: String): Boolean {
        return platformFileExists(path)
    }

    /**
     * Find executable in PATH
     */
    private fun findInPath(binaryName: String): String? {
        return platformFindInPath(binaryName)
    }

    /**
     * Check if running on Windows
     */
    private fun isWindows(): Boolean {
        return platformIsWindows()
    }

    /**
     * Check if running on macOS
     */
    private fun isMacOS(): Boolean {
        return platformIsMacOS()
    }
}

private fun platformGetUserShellPath(): String? = getenv("SHELL")?.toKString()
private fun platformFileExists(path: String): Boolean = access(path, F_OK) == 0
private fun platformFindInPath(binaryName: String): String? = null // TODO: Implement
private fun platformIsWindows(): Boolean = false
private fun platformIsMacOS(): Boolean = true