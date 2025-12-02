// port-lint: source core/src/command_safety/is_safe_command.rs
package ai.solace.coder.core.command_safety

import ai.solace.coder.core.bash.parseShellLcPlainCommands

/**
 * Checks if a command is known to be safe.
 */
fun isKnownSafeCommand(command: List<String>): Boolean {
    val normalizedCommand: List<String> = command.map { s ->
        if (s == "zsh") {
            "bash"
        } else {
            s
        }
    }

    if (isSafeCommandWindows(normalizedCommand)) {
        return true
    }

    if (isSafeToCallWithExec(normalizedCommand)) {
        return true
    }

    // Support `bash -lc "..."` where the script consists solely of one or
    // more "plain" commands (only bare words / quoted strings) combined with
    // a conservative allow-list of shell operators that themselves do not
    // introduce side effects ( "&&", "||", ";", and "|" ). If every
    // individual command in the script is itself a known-safe command, then
    // the composite expression is considered safe.
    val allCommands = parseShellLcPlainCommands(normalizedCommand)
    if (allCommands != null && allCommands.isNotEmpty() && allCommands.all { cmd -> isSafeToCallWithExec(cmd) }) {
        return true
    }
    return false
}

private fun isSafeToCallWithExec(command: List<String>): Boolean {
    val cmd0 = command.firstOrNull() ?: return false

    // Extract just the filename from the path
    val baseName = cmd0.substringAfterLast('/').substringAfterLast('\\')

    return when (baseName) {
        "cat", "cd", "echo", "false", "grep", "head", "ls", "nl", "pwd", "tail", "true", "wc", "which" -> true

        "find" -> {
            // Certain options to `find` can delete files, write to files, or
            // execute arbitrary commands, so we cannot auto-approve the
            // invocation of `find` in such cases.
            val unsafeFindOptions = listOf(
                // Options that can execute arbitrary commands.
                "-exec", "-execdir", "-ok", "-okdir",
                // Option that deletes matching files.
                "-delete",
                // Options that write pathnames to a file.
                "-fls", "-fprint", "-fprint0", "-fprintf"
            )

            !command.any { arg -> unsafeFindOptions.contains(arg) }
        }

        // Ripgrep
        "rg" -> {
            val unsafeRipgrepOptionsWithArgs = listOf(
                // Takes an arbitrary command that is executed for each match.
                "--pre",
                // Takes a command that can be used to obtain the local hostname.
                "--hostname-bin"
            )
            val unsafeRipgrepOptionsWithoutArgs = listOf(
                // Calls out to other decompression tools, so do not auto-approve
                // out of an abundance of caution.
                "--search-zip",
                "-z"
            )

            !command.any { arg ->
                unsafeRipgrepOptionsWithoutArgs.contains(arg) ||
                    unsafeRipgrepOptionsWithArgs.any { opt ->
                        arg == opt || arg.startsWith("$opt=")
                    }
            }
        }

        // Git
        "git" -> {
            val subCommand = command.getOrNull(1)
            subCommand in listOf("branch", "status", "log", "diff", "show")
        }

        // Rust
        "cargo" -> command.getOrNull(1) == "check"

        // Special-case `sed -n {N|M,N}p`
        "sed" -> {
            command.size <= 4 &&
                command.getOrNull(1) == "-n" &&
                isValidSedNArg(command.getOrNull(2))
        }

        // ── anything else ─────────────────────────────────────────────────
        else -> false
    }
}

/**
 * Returns true if `arg` matches /^(\d+,)?\d+p$/
 */
private fun isValidSedNArg(arg: String?): Boolean {
    // unwrap or bail
    val s = arg ?: return false

    // must end with 'p', strip it
    val core = if (s.endsWith('p')) {
        s.dropLast(1)
    } else {
        return false
    }

    // split on ',' and ensure 1 or 2 numeric parts
    val parts = core.split(',')
    return when (parts.size) {
        // single number, e.g. "10"
        1 -> parts[0].isNotEmpty() && parts[0].all { it.isDigit() }

        // two numbers, e.g. "1,5"
        2 -> {
            parts[0].isNotEmpty() &&
                parts[1].isNotEmpty() &&
                parts[0].all { it.isDigit() } &&
                parts[1].all { it.isDigit() }
        }

        // anything else (more than one comma) is invalid
        else -> false
    }
}
