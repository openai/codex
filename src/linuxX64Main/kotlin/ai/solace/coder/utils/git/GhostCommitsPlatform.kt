package ai.solace.coder.utils.git

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.refTo
import kotlinx.cinterop.toKString
import platform.posix.chdir
import platform.posix.fgets
import platform.posix.getcwd
import platform.posix.pclose
import platform.posix.popen
import platform.posix.setenv
import platform.posix.unsetenv

/**
 * Helper to extract exit status from wait status (WIFEXITED/WEXITSTATUS macros)
 */
private fun extractExitCode(status: Int): Int {
    // On Linux/POSIX: WIFEXITED(status) is ((status & 0x7f) == 0)
    // WEXITSTATUS(status) is ((status >> 8) & 0xff)
    val ifExited = (status and 0x7f) == 0
    return if (ifExited) {
        (status shr 8) and 0xff
    } else {
        1 // Process terminated abnormally
    }
}

/**
 * Linux implementation of git command execution.
 */
@OptIn(ExperimentalForeignApi::class)
internal actual fun platformExecuteGit(
    cwd: String,
    args: List<String>,
    extraEnv: Map<String, String>
): Pair<String, Int> {
    // Set up environment variables
    for ((key, value) in extraEnv) {
        setenv(key, value, 1)
    }

    // Build command string with proper escaping
    val gitArgs = listOf("git") + args
    val command = gitArgs.joinToString(" ") { arg ->
        if (arg.contains(' ') || arg.contains('"') || arg.contains('\'')) {
            "\"${arg.replace("\"", "\\\"")}\""
        } else {
            arg
        }
    }

    // Save current directory
    val buffer = ByteArray(1024)
    val savedCwd = getcwd(buffer.refTo(0), buffer.size.toULong())?.toKString() ?: ""

    // Change to target directory
    if (chdir(cwd) != 0) {
        return Pair("", 1)
    }

    try {
        // Execute command and capture output
        val fp = popen(command, "r")
        if (fp == null) {
            return Pair("", 1)
        }

        val output = StringBuilder()
        val readBuffer = ByteArray(4096)

        while (true) {
            val line = fgets(readBuffer.refTo(0), readBuffer.size, fp)
            if (line == null) break
            output.append(line.toKString())
        }

        val status = pclose(fp)
        val exitCode = extractExitCode(status)

        return Pair(output.toString(), exitCode)
    } finally {
        // Restore original directory
        if (savedCwd.isNotEmpty()) {
            chdir(savedCwd)
        }

        // Clear environment variables (best effort)
        for ((key, _) in extraEnv) {
            unsetenv(key)
        }
    }
}

/**
 * Linux implementation of general command execution.
 */
@OptIn(ExperimentalForeignApi::class)
internal actual fun platformExecuteCommand(args: List<String>): Int {
    val command = args.joinToString(" ") { arg ->
        if (arg.contains(' ') || arg.contains('"') || arg.contains('\'')) {
            "\"${arg.replace("\"", "\\\"")}\""
        } else {
            arg
        }
    }

    val status = platform.posix.system(command)
    return extractExitCode(status)
}
