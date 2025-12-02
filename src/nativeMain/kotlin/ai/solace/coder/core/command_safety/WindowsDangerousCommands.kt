// port-lint: source core/src/command_safety/windows_dangerous_commands.rs
package ai.solace.coder.core.command_safety

import ai.solace.coder.exec.shell.CommandParser

/**
 * Checks if a command is dangerous on Windows.
 */
fun isDangerousCommandWindows(command: List<String>): Boolean {
    // Prefer structured parsing for PowerShell/CMD so we can spot URL-bearing
    // invocations of ShellExecute-style entry points before falling back to
    // simple argv heuristics.
    if (isDangerousPowershell(command)) {
        return true
    }

    if (isDangerousCmd(command)) {
        return true
    }

    return isDirectGuiLaunch(command)
}

private fun isDangerousPowershell(command: List<String>): Boolean {
    if (command.isEmpty()) return false
    val exe = command.first()
    val rest = command.drop(1)

    if (!isPowershellExecutable(exe)) {
        return false
    }

    // Parse the PowerShell invocation to get a flat token list we can scan for
    // dangerous cmdlets/COM calls plus any URL-looking arguments. This is a
    // best-effort shlex split of the script text, not a full PS parser.
    val parsed = parsePowershellInvocation(rest) ?: return false

    val tokensLc: List<String> = parsed.tokens.map { t ->
        t.trim('\'', '"').lowercase()
    }
    val hasUrl = argsHaveUrl(parsed.tokens)

    if (hasUrl && tokensLc.any { t ->
            t == "start-process" || t == "start" || t == "saps" ||
                t == "invoke-item" || t == "ii" ||
                t.contains("start-process") || t.contains("invoke-item")
        }) {
        return true
    }

    if (hasUrl && tokensLc.any { t ->
            t.contains("shellexecute") || t.contains("shell.application")
        }) {
        return true
    }

    val first = tokensLc.firstOrNull()
    if (first != null) {
        // Legacy ShellExecute path via url.dll
        if (first == "rundll32" &&
            tokensLc.any { t -> t.contains("url.dll,fileprotocolhandler") } &&
            hasUrl
        ) {
            return true
        }
        if (first == "mshta" && hasUrl) {
            return true
        }
        if (isBrowserExecutable(first) && hasUrl) {
            return true
        }
        if ((first == "explorer" || first == "explorer.exe") && hasUrl) {
            return true
        }
    }

    return false
}

private fun isDangerousCmd(command: List<String>): Boolean {
    if (command.isEmpty()) return false
    val exe = command.first()
    val rest = command.drop(1)

    val base = executableBasename(exe) ?: return false
    if (base != "cmd" && base != "cmd.exe") {
        return false
    }

    val iter = rest.iterator()
    while (iter.hasNext()) {
        val arg = iter.next()
        val lower = arg.lowercase()
        when {
            lower == "/c" || lower == "/r" || lower == "-c" -> break
            lower.startsWith('/') -> continue
            // Unknown tokens before the command body => bail.
            else -> return false
        }
    }

    if (!iter.hasNext()) return false
    val firstCmd = iter.next()

    // Classic `cmd /c start https://...` ShellExecute path.
    if (!firstCmd.equals("start", ignoreCase = true)) {
        return false
    }

    val remaining = mutableListOf<String>()
    while (iter.hasNext()) {
        remaining.add(iter.next())
    }
    return argsHaveUrl(remaining)
}

private fun isDirectGuiLaunch(command: List<String>): Boolean {
    if (command.isEmpty()) return false
    val exe = command.first()
    val rest = command.drop(1)

    val base = executableBasename(exe) ?: return false

    // Explorer/rundll32/mshta or direct browser exe with a URL anywhere in args.
    if ((base == "explorer" || base == "explorer.exe") && argsHaveUrl(rest)) {
        return true
    }
    if ((base == "mshta" || base == "mshta.exe") && argsHaveUrl(rest)) {
        return true
    }
    if ((base == "rundll32" || base == "rundll32.exe") &&
        rest.any { t -> t.lowercase().contains("url.dll,fileprotocolhandler") } &&
        argsHaveUrl(rest)
    ) {
        return true
    }
    if (isBrowserExecutable(base) && argsHaveUrl(rest)) {
        return true
    }

    return false
}

private fun argsHaveUrl(args: List<String>): Boolean {
    return args.any { arg -> looksLikeUrl(arg) }
}

private fun looksLikeUrl(token: String): Boolean {
    // Strip common PowerShell punctuation around inline URLs (quotes, parens, trailing semicolons).
    // If the token embeds a URL alongside other text (e.g., Start-Process('https://...'))
    // as a single shlex token, grab the substring starting at the first URL prefix.
    val httpsIdx = token.indexOf("https://")
    val httpIdx = token.indexOf("http://")
    val urlIdx = when {
        httpsIdx >= 0 && httpIdx >= 0 -> minOf(httpsIdx, httpIdx)
        httpsIdx >= 0 -> httpsIdx
        httpIdx >= 0 -> httpIdx
        else -> -1
    }
    val urlish = if (urlIdx >= 0) token.substring(urlIdx) else token

    // Simple regex-like cleanup: trim leading quotes/parens/whitespace and trailing semicolons/closing parens
    val candidate = urlish
        .trimStart(' ', '"', '\'', '(')
        .trimEnd(' ', ';', ')')
        .takeWhile { it != '"' && it != '\'' && it != ')' && it != ';' && !it.isWhitespace() }

    // Check if it's a valid http/https URL
    return try {
        (candidate.startsWith("http://") || candidate.startsWith("https://")) &&
            candidate.length > 8 // At minimum "http://x"
    } catch (_: Exception) {
        false
    }
}

private fun executableBasename(exe: String): String? {
    val fileName = exe.substringAfterLast('/').substringAfterLast('\\')
    return if (fileName.isNotEmpty()) fileName.lowercase() else null
}

private fun isPowershellExecutable(exe: String): Boolean {
    val base = executableBasename(exe)
    return base == "powershell" || base == "powershell.exe" || base == "pwsh" || base == "pwsh.exe"
}

private fun isBrowserExecutable(name: String): Boolean {
    return name in listOf(
        "chrome", "chrome.exe",
        "msedge", "msedge.exe",
        "firefox", "firefox.exe",
        "iexplore", "iexplore.exe"
    )
}

private data class ParsedPowershell(
    val tokens: List<String>
)

private fun parsePowershellInvocation(args: List<String>): ParsedPowershell? {
    if (args.isEmpty()) {
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
                    return null
                }
                val tokens = shlexSplit(script) ?: return null
                return ParsedPowershell(tokens)
            }
            lower.startsWith("-command:") || lower.startsWith("/command:") -> {
                if (idx + 1 != args.size) {
                    return null
                }
                val colonIdx = arg.indexOf(':')
                if (colonIdx < 0) return null
                val script = arg.substring(colonIdx + 1)
                val tokens = shlexSplit(script) ?: return null
                return ParsedPowershell(tokens)
            }
            lower in listOf("-nologo", "-noprofile", "-noninteractive", "-mta", "-sta") -> {
                idx += 1
            }
            lower.startsWith('-') -> {
                idx += 1
            }
            else -> {
                val rest = args.subList(idx, args.size)
                return ParsedPowershell(rest)
            }
        }
    }

    return null
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
