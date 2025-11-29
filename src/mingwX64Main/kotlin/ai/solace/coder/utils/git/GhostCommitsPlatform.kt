package ai.solace.coder.utils.git

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.alloc
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.ptr
import kotlinx.cinterop.refTo
import kotlinx.cinterop.toKString
import platform.posix.S_IFDIR
import platform.posix.S_IFMT
import platform.posix._pclose
import platform.posix._popen
import platform.posix._stat64
import platform.posix.fgets
import platform.posix.putenv

/**
 * Windows implementation of git command execution.
 */
@OptIn(ExperimentalForeignApi::class)
internal actual fun platformExecuteGit(
    cwd: String,
    args: List<String>,
    extraEnv: Map<String, String>
): Pair<String, Int> {
    // Set up environment variables using putenv
    for ((key, value) in extraEnv) {
        putenv("$key=$value")
    }

    // Build command string with cd prefix for Windows
    // Windows uses double quotes for arguments with spaces
    val gitArgs = listOf("git") + args
    val gitCommand = gitArgs.joinToString(" ") { arg ->
        if (arg.contains(' ') || arg.contains('"')) {
            "\"${arg.replace("\"", "\\\"")}\""
        } else {
            arg
        }
    }

    // Combine cd and git command
    val command = "cd /d \"$cwd\" && $gitCommand"

    try {
        // Execute command and capture output using _popen
        val fp = _popen(command, "r")
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

        val exitCode = _pclose(fp)

        return Pair(output.toString(), exitCode)
    } finally {
        // Clear environment variables (best effort) by setting to empty
        for ((key, _) in extraEnv) {
            putenv("$key=")
        }
    }
}

/**
 * Windows implementation of general command execution.
 */
@OptIn(ExperimentalForeignApi::class)
internal actual fun platformExecuteCommand(args: List<String>): Int {
    val command = args.joinToString(" ") { arg ->
        if (arg.contains(' ') || arg.contains('"')) {
            "\"${arg.replace("\"", "\\\"")}\""
        } else {
            arg
        }
    }

    return platform.posix.system(command)
}

/**
 * Windows implementation of directory check.
 */
@OptIn(ExperimentalForeignApi::class)
internal actual fun platformIsDirectory(path: String): Boolean {
    return memScoped {
        val statBuf = alloc<_stat64>()
        if (platform.posix._stat64(path, statBuf.ptr) != 0) {
            false
        } else {
            (statBuf.st_mode.toInt() and S_IFMT) == S_IFDIR
        }
    }
}
