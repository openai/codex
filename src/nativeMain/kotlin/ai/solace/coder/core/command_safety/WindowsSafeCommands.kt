// port-lint: source core/src/command_safety/windows_safe_commands.rs
package ai.solace.coder.core.command_safety

import ai.solace.coder.exec.shell.CommandParser

/**
 * On Windows, we conservatively allow only clearly read-only PowerShell invocations
 * that match a small safelist. Anything else (including direct CMD commands) is unsafe.
 */
fun isSafeCommandWindows(command: List<String>): Boolean {
    val commands = tryParsePowershellCommandSequence(command)
    if (commands != null) {
        return commands.all { cmd -> isSafePowershellCommand(cmd) }
    }
    // Only PowerShell invocations are allowed on Windows for now; anything else is unsafe.
    return false
}

/**
 * Returns each command sequence if the invocation starts with a PowerShell binary.
 * For example, the tokens from `pwsh Get-ChildItem | Measure-Object` become two sequences.
 */
private fun tryParsePowershellCommandSequence(command: List<String>): List<List<String>>? {
    if (command.isEmpty()) return null
    val exe = command.first()
    val rest = command.drop(1)
    if (!isPowershellExecutable(exe)) {
        return null
    }
    return parsePowershellInvocation(rest)
}

/**
 * Parses a PowerShell invocation into discrete command vectors, rejecting unsafe patterns.
 */
private fun parsePowershellInvocation(args: List<String>): List<List<String>>? {
    if (args.isEmpty()) {
        // Examples rejected here: "pwsh" and "powershell.exe" with no additional arguments.
        return null
    }

    var idx = 0
    while (idx < args.size) {
        val arg = args[idx]
        val lower = arg.lowercase()
        when {
            lower == "-command" || lower == "/command" || lower == "-c" -> {
                val script = args.getOrNull(idx + 1) ?: return null
                if (idx + 2 != args.size) {
                    // Reject if there is more than one token representing the actual command.
                    // Examples rejected here: "pwsh -Command foo bar" and "powershell -c ls extra".
                    return null
                }
                return parsePowershellScript(script)
            }
            lower.startsWith("-command:") || lower.startsWith("/command:") -> {
                if (idx + 1 != args.size) {
                    // Reject if there are more tokens after the command itself.
                    // Examples rejected here: "pwsh -Command:dir C:\\" and "powershell /Command:dir C:\\" with trailing args.
                    return null
                }
                val colonIdx = arg.indexOf(':')
                if (colonIdx < 0) return null
                val script = arg.substring(colonIdx + 1)
                return parsePowershellScript(script)
            }

            // Benign, no-arg flags we tolerate.
            lower in listOf("-nologo", "-noprofile", "-noninteractive", "-mta", "-sta") -> {
                idx += 1
                continue
            }

            // Explicitly forbidden/opaque or unnecessary for read-only operations.
            lower in listOf("-encodedcommand", "-ec", "-file", "/file", "-windowstyle", "-executionpolicy", "-workingdirectory") -> {
                // Examples rejected here: "pwsh -EncodedCommand ..." and "powershell -File script.ps1".
                return null
            }

            // Unknown switch → bail conservatively.
            lower.startsWith('-') -> {
                // Examples rejected here: "pwsh -UnknownFlag" and "powershell -foo bar".
                return null
            }

            // If we hit non-flag tokens, treat the remainder as a command sequence.
            // This happens if powershell is invoked without -Command, e.g.
            // ["pwsh", "-NoLogo", "git", "-c", "core.pager=cat", "status"]
            else -> {
                return splitIntoCommands(args.subList(idx, args.size).toMutableList())
            }
        }
    }

    // Examples rejected here: "pwsh" and "powershell.exe -NoLogo" without a script.
    return null
}

/**
 * Tokenizes an inline PowerShell script and delegates to the command splitter.
 * Examples of when this is called: pwsh.exe -Command '<script>' or pwsh.exe -Command:<script>
 */
private fun parsePowershellScript(script: String): List<List<String>>? {
    val tokens = shlexSplit(script) ?: return null
    return splitIntoCommands(tokens.toMutableList())
}

/**
 * Splits tokens into pipeline segments while ensuring no unsafe separators slip through.
 * e.g. Get-ChildItem | Measure-Object -> [['Get-ChildItem'], ['Measure-Object']]
 */
private fun splitIntoCommands(tokens: List<String>): List<List<String>>? {
    if (tokens.isEmpty()) {
        // Examples rejected here: "pwsh -Command ''" and "pwsh -Command \"\"".
        return null
    }

    val commands = mutableListOf<List<String>>()
    var current = mutableListOf<String>()
    for (token in tokens) {
        when (token) {
            "|", "||", "&&", ";" -> {
                if (current.isEmpty()) {
                    // Examples rejected here: "pwsh -Command '| Get-ChildItem'" and "pwsh -Command '; dir'".
                    return null
                }
                commands.add(current.toList())
                current = mutableListOf()
            }
            // Reject if any token embeds separators, redirection, or call operator characters.
            else -> {
                if (token.contains('|') || token.contains(';') || token.contains('>') ||
                    token.contains('<') || token.contains('&') || token.contains("\$(")
                ) {
                    // Examples rejected here: "pwsh -Command 'dir|select'" and "pwsh -Command 'echo hi > out.txt'".
                    return null
                }
                current.add(token)
            }
        }
    }

    if (current.isEmpty()) {
        // Examples rejected here: "pwsh -Command 'dir |'" and "pwsh -Command 'Get-ChildItem ;'".
        return null
    }
    commands.add(current.toList())
    return commands
}

/**
 * Returns true when the executable name is one of the supported PowerShell binaries.
 */
private fun isPowershellExecutable(exe: String): Boolean {
    val executableName = exe.substringAfterLast('/').substringAfterLast('\\').lowercase()
    return executableName in listOf("powershell", "powershell.exe", "pwsh", "pwsh.exe")
}

/**
 * Validates that a parsed PowerShell command stays within our read-only safelist.
 * Everything before this is parsing, and rejecting things that make us feel uncomfortable.
 */
private fun isSafePowershellCommand(words: List<String>): Boolean {
    if (words.isEmpty()) {
        // Examples rejected here: "pwsh -Command ''" and "pwsh -Command \"\"".
        return false
    }

    // Reject nested unsafe cmdlets inside parentheses or arguments
    for (w in words) {
        val inner = w
            .trim('(', ')')
            .trimStart('-')
            .lowercase()
        if (inner in listOf(
                "set-content",
                "add-content",
                "out-file",
                "new-item",
                "remove-item",
                "move-item",
                "copy-item",
                "rename-item",
                "start-process",
                "stop-process"
            )
        ) {
            // Examples rejected here: "Write-Output (Set-Content foo6.txt 'abc')" and "Get-Content (New-Item bar.txt)".
            return false
        }
    }

    // Block PowerShell call operator or any redirection explicitly.
    if (words.any { w ->
            w in listOf("&", ">", ">>", "1>", "2>", "2>&1", "*>", "<", "<<")
        }) {
        // Examples rejected here: "pwsh -Command '& Remove-Item foo'" and "pwsh -Command 'Get-Content foo > bar'".
        return false
    }

    val command = words[0]
        .trim('(', ')')
        .trimStart('-')
        .lowercase()

    return when (command) {
        "echo", "write-output", "write-host" -> true // (no redirection allowed)
        "dir", "ls", "get-childitem", "gci" -> true
        "cat", "type", "gc", "get-content" -> true
        "select-string", "sls", "findstr" -> true
        "measure-object", "measure" -> true
        "get-location", "gl", "pwd" -> true
        "test-path", "tp" -> true
        "resolve-path", "rvpa" -> true
        "select-object", "select" -> true
        "get-item" -> true

        "git" -> isSafeGitCommand(words)

        "rg" -> isSafeRipgrep(words)

        // Extra safety: explicitly prohibit common side-effecting cmdlets regardless of args.
        "set-content", "add-content", "out-file", "new-item", "remove-item", "move-item",
        "copy-item", "rename-item", "start-process", "stop-process" -> {
            // Examples rejected here: "pwsh -Command 'Set-Content notes.txt data'" and "pwsh -Command 'Remove-Item temp.log'".
            false
        }

        else -> {
            // Examples rejected here: "pwsh -Command 'Invoke-WebRequest https://example.com'" and "pwsh -Command 'Start-Service Spooler'".
            false
        }
    }
}

/**
 * Checks that an `rg` invocation avoids options that can spawn arbitrary executables.
 */
private fun isSafeRipgrep(words: List<String>): Boolean {
    val unsafeRipgrepOptionsWithArgs = listOf("--pre", "--hostname-bin")
    val unsafeRipgrepOptionsWithoutArgs = listOf("--search-zip", "-z")

    return !words.drop(1).any { arg ->
        val argLc = arg.lowercase()
        // Examples rejected here: "pwsh -Command 'rg --pre cat pattern'" and "pwsh -Command 'rg --search-zip pattern'".
        unsafeRipgrepOptionsWithoutArgs.contains(argLc) ||
            unsafeRipgrepOptionsWithArgs.any { opt ->
                argLc == opt || argLc.startsWith("$opt=")
            }
    }
}

/**
 * Ensures a Git command sticks to whitelisted read-only subcommands and flags.
 */
private fun isSafeGitCommand(words: List<String>): Boolean {
    val safeSubcommands = listOf("status", "log", "show", "diff", "cat-file")

    val iter = words.drop(1).iterator()
    while (iter.hasNext()) {
        val arg = iter.next()
        val argLc = arg.lowercase()

        if (arg.startsWith('-')) {
            if (arg.equals("-c", ignoreCase = true) || arg.equals("--config", ignoreCase = true)) {
                if (!iter.hasNext()) {
                    // Examples rejected here: "pwsh -Command 'git -c'" and "pwsh -Command 'git --config'".
                    return false
                }
                iter.next() // consume the config value
                continue
            }

            if (argLc.startsWith("-c=") ||
                argLc.startsWith("--config=") ||
                argLc.startsWith("--git-dir=") ||
                argLc.startsWith("--work-tree=")
            ) {
                continue
            }

            if (arg.equals("--git-dir", ignoreCase = true) || arg.equals("--work-tree", ignoreCase = true)) {
                if (!iter.hasNext()) {
                    // Examples rejected here: "pwsh -Command 'git --git-dir'" and "pwsh -Command 'git --work-tree'".
                    return false
                }
                iter.next() // consume the path
                continue
            }

            continue
        }

        return safeSubcommands.contains(argLc)
    }

    // Examples rejected here: "pwsh -Command 'git'" and "pwsh -Command 'git status --short | Remove-Item foo'".
    return false
}

/**
 * Simple shlex-like split for shell tokenization.
 */
private fun shlexSplit(input: String): List<String>? {
    val parser = CommandParser()
    return try {
        parser.tokenize(input)
    } catch (_: Exception) {
        null
    }
}
