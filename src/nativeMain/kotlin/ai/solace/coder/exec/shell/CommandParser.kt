package ai.solace.coder.exec.shell

/**
 * Result of parsing a shell command into executable and arguments.
 * Note: This is different from ai.solace.coder.protocol.ParsedCommand which is a protocol type.
 */
data class ShellParsedCommand(
    val executable: String,
    val args: List<String>
)

/**
 * Shell command parser with support for quoting and escaping
 */
class CommandParser {
    companion object {
        private const val SINGLE_QUOTE = '\''
        private const val DOUBLE_QUOTE = '"'
        private const val BACKSLASH = '\\'
        private const val SPACE = ' '
        private const val TAB = '\t'
        private const val NEWLINE = '\n'
        private const val CARRIAGE_RETURN = '\r'
    }

    /**
     * Parse a shell command string into executable and arguments
     */
    fun parse(command: String): ShellParsedCommand {
        val tokens = tokenize(command)
        if (tokens.isEmpty()) {
            throw IllegalArgumentException("Empty command")
        }
        return ShellParsedCommand(tokens.first(), tokens.drop(1))
    }

    /**
     * Tokenize a command string handling quotes and escapes
     */
    fun tokenize(command: String): List<String> {
        val tokens = mutableListOf<String>()
        val currentToken = StringBuilder()
        var inSingleQuote = false
        var inDoubleQuote = false
        var escapeNext = false

        for (char in command) {
            when {
                escapeNext -> {
                    currentToken.append(char)
                    escapeNext = false
                }
                char == BACKSLASH && !inSingleQuote -> {
                    escapeNext = true
                }
                char == SINGLE_QUOTE && !inDoubleQuote -> {
                    inSingleQuote = !inSingleQuote
                }
                char == DOUBLE_QUOTE && !inSingleQuote -> {
                    inDoubleQuote = !inDoubleQuote
                }
                isWhitespace(char) && !inSingleQuote && !inDoubleQuote -> {
                    if (currentToken.isNotEmpty()) {
                        tokens.add(currentToken.toString())
                        currentToken.clear()
                    }
                }
                else -> {
                    currentToken.append(char)
                }
            }
        }

        // Add the last token if there is one
        if (currentToken.isNotEmpty()) {
            tokens.add(currentToken.toString())
        }

        return tokens
    }

    /**
     * Check if a character is whitespace
     */
    private fun isWhitespace(char: Char): Boolean {
        return char == SPACE || char == TAB || char == NEWLINE || char == CARRIAGE_RETURN
    }

    /**
     * Escape a string for shell execution
     */
    fun escapeForShell(argument: String, shellType: ShellType): String {
        return when (shellType) {
            ShellType.PowerShell -> escapeForPowerShell(argument)
            ShellType.Cmd -> escapeForCmd(argument)
            else -> escapeForUnixShell(argument)
        }
    }

    /**
     * Escape argument for Unix shells (bash, zsh, sh)
     */
    private fun escapeForUnixShell(argument: String): String {
        if (argument.isEmpty()) return "''"
        
        // Check if we need quoting
        val needsQuoting = argument.any { char ->
            char.isWhitespace() || char == SINGLE_QUOTE || char == DOUBLE_QUOTE || 
            char == BACKSLASH || char == '$' || char == '`' || char == '&' || 
            char == ';' || char == '|' || char == '<' || char == '>' || 
            char == '(' || char == ')' || char == '!' || char == '*' || 
            char == '?' || char == '[' || char == ']' || char == '{' || 
            char == '}' || char == '~' || char == '#'
        }

        if (!needsQuoting) {
            return argument
        }

        // Use single quotes if no single quotes in the string
        if (!argument.contains(SINGLE_QUOTE)) {
            return "'$argument'"
        }

        // Use double quotes with escaping
        val escaped = StringBuilder()
        escaped.append('"')
        for (char in argument) {
            when (char) {
                DOUBLE_QUOTE, BACKSLASH, '$', '`' -> escaped.append('\\').append(char)
                else -> escaped.append(char)
            }
        }
        escaped.append('"')
        return escaped.toString()
    }

    /**
     * Escape argument for PowerShell
     */
    private fun escapeForPowerShell(argument: String): String {
        if (argument.isEmpty()) return "''"
        
        val needsQuoting = argument.any { char ->
            char.isWhitespace() || char == SINGLE_QUOTE || char == DOUBLE_QUOTE ||
            char == '`' || char == '$' || char == '(' || char == ')' || 
            char == '&' || char == ';' || char == '|' || char == '<' || 
            char == '>' || char == '@' || char == ',' || char == '!' || 
            char == '%' || char == '+'            }

        if (!needsQuoting) {
            return argument
        }

        // PowerShell uses single quotes for literal strings
        if (!argument.contains(SINGLE_QUOTE)) {
            return "'$argument'"
        }

        // Use double quotes with escaping
        val escaped = StringBuilder()
        escaped.append('"')
        for (char in argument) {
            when (char) {
                DOUBLE_QUOTE, '`', '$' -> escaped.append('`').append(char)
                else -> escaped.append(char)
            }
        }
        escaped.append('"')
        return escaped.toString()
    }

    /**
     * Escape argument for Windows CMD
     */
    private fun escapeForCmd(argument: String): String {
        if (argument.isEmpty()) return "\"\""
        
        val needsQuoting = argument.any { char ->
            char.isWhitespace() || char == '"' || char == '%' || 
            char == '&' || char == '<' || char == '>' || char == '|' ||
            char == '^'
        }

        if (!needsQuoting) {
            return argument
        }

        // CMD uses double quotes with special escaping
        val escaped = StringBuilder()
        escaped.append('"')
        for (char in argument) {
            when (char) {
                '"' -> escaped.append('"').append('"')
                else -> escaped.append(char)
            }
        }
        escaped.append('"')
        return escaped.toString()
    }

    /**
     * Join arguments into a command string
     */
    fun joinCommand(executable: String, args: List<String>, shellType: ShellType): String {
        val parts = mutableListOf<String>()
        parts.add(escapeForShell(executable, shellType))
        parts.addAll(args.map { escapeForShell(it, shellType) })
        return parts.joinToString(" ")
    }

    /**
     * Validate a parsed command
     */
    fun validateCommand(parsed: ShellParsedCommand): Boolean {
        // Check if executable is not empty
        if (parsed.executable.isEmpty()) return false
        
        // Check for suspicious patterns
        val suspiciousPatterns = listOf(
            "..", "~", "$", "`", ";", "&", "|", "<", ">", "!", "*",
            "?", "[", "]", "{", "}", "#", "(", ")"
        )
        
        // Basic validation - in a real implementation, this would be more sophisticated
        return true
    }

    /**
     * Normalize a command path
     */
    fun normalizePath(path: String): String {
        return path.replace('\\', '/').trim()
    }

    /**
     * Check if a command looks like a built-in shell command
     */
    fun isShellBuiltin(command: String, shellType: ShellType): Boolean {
        val builtins = when (shellType) {
            ShellType.Bash, ShellType.Zsh -> setOf(
                "cd", "pwd", "echo", "export", "unset", "alias", "unalias",
                "history", "jobs", "fg", "bg", "kill", "wait", "type",
                "which", "whereis", "source", ".", "exec", "exit", "return",
                "break", "continue", "test", "[", "let", "declare", "local",
                "readonly", "typeset", "read", "readarray", "mapfile",
                "printf", "printf", "shift", "set", "unset", "shopt",
                "complete", "compgen", "compopt", "bind", "help", "hash"
            )
            ShellType.Sh -> setOf(
                "cd", "pwd", "echo", "export", "unset", "readonly", "trap",
                "wait", "exit", "return", "break", "continue", "test", "[",
                ":", ".", "exec", "kill", "shift", "set", "unset", "read",
                "printf", "command", "type", "times", "umask"
            )
            ShellType.PowerShell -> setOf(
                "cd", "pwd", "echo", "write-output", "write-host", "write-error",
                "write-warning", "write-verbose", "write-debug", "write-information",
                "out-file", "out-string", "out-null", "out-default", "out-host",
                "out-grid", "out-printer", "set-content", "get-content", "add-content",
                "clear-content", "clear-host", "clear-item", "copy-item", "get-item",
                "invoke-item", "move-item", "new-item", "remove-item", "rename-item",
                "set-item", "get-location", "set-location", "push-location", "pop-location",
                "get-childitem", "get-command", "get-history", "add-history", "clear-history",
                "get-job", "start-job", "stop-job", "remove-job", "wait-job", "receive-job",
                "get-process", "start-process", "stop-process", "wait-process", "debug-process"
            )
            ShellType.Cmd -> setOf(
                "cd", "chdir", "md", "mkdir", "rd", "rmdir", "del", "erase",
                "copy", "xcopy", "move", "ren", "rename", "type", "more", "find",
                "findstr", "sort", "dir", "tree", "path", "ver", "vol", "date",
                "time", "set", "setlocal", "endlocal", "call", "echo", "pause",
                "break", "cls", "cmd", "exit", "goto", "if", "for", "do", "rem",
                "start", "assoc", "ftype", "pushd", "popd", "attrib", "cacls",
                "comp", "compact", "convert", "expand", "fc", "format", "mode",
                "more", "recover", "replace", "subst", "shutdown", "tasklist",
                "taskkill", "timeout", "title", "color", "prompt"
            )
        }
        
        return command.lowercase() in builtins
    }
}