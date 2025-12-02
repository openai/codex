// port-lint: source core/src/bash.rs
package ai.solace.coder.core.bash

import ai.solace.coder.exec.shell.ShellType
import ai.solace.coder.exec.shell.ShellDetector
import io.github.treesitter.ktreesitter.Language
import io.github.treesitter.ktreesitter.Node
import io.github.treesitter.ktreesitter.Parser
import io.github.treesitter.ktreesitter.Tree

/**
 * Parse the provided bash source using tree-sitter-bash, returning a Tree on
 * success or null if parsing failed.
 */
fun tryParseShell(shellLcArg: String): Tree? {
    return try {
        val language = Language.load("bash")
        val parser = Parser(language)
        parser.parse(shellLcArg)
    } catch (_: Exception) {
        null
    }
}

/**
 * Parse a script which may contain multiple simple commands joined only by
 * the safe logical/pipe/sequencing operators: `&&`, `||`, `;`, `|`.
 *
 * Returns a list of command word vectors if every command is a plain word-only
 * command and the parse tree does not contain disallowed constructs
 * (parentheses, redirections, substitutions, control flow, etc.). Otherwise
 * returns null.
 */
fun tryParseWordOnlyCommandsSequence(tree: Tree, src: String): List<List<String>>? {
    if (tree.rootNode.hasError) {
        return null
    }

    // List of allowed (named) node kinds for a "word only commands sequence".
    // If we encounter a named node that is not in this list we reject.
    val allowedKinds = setOf(
        // top level containers
        "program",
        "list",
        "pipeline",
        // commands & words
        "command",
        "command_name",
        "word",
        "string",
        "string_content",
        "raw_string",
        "number"
    )
    // Allow only safe punctuation / operator tokens; anything else causes reject.
    val allowedPunctTokens = setOf("&&", "||", ";", "|", "\"", "'")

    val root = tree.rootNode
    val stack = ArrayDeque<Node>()
    stack.addLast(root)
    val commandNodes = mutableListOf<Node>()

    while (stack.isNotEmpty()) {
        val node = stack.removeLast()
        val kind = node.type

        if (node.isNamed) {
            if (kind !in allowedKinds) {
                return null
            }
            if (kind == "command") {
                commandNodes.add(node)
            }
        } else {
            // Reject any punctuation / operator tokens that are not explicitly allowed.
            if (kind.any { c -> c in "&;|" } && kind !in allowedPunctTokens) {
                return null
            }
            if (kind !in allowedPunctTokens && kind.trim().isNotEmpty()) {
                // If it's a quote token or operator it's allowed above; we also allow whitespace tokens.
                // Any other punctuation like parentheses, braces, redirects, backticks, etc are rejected.
                return null
            }
        }
        for (i in 0 until node.childCount.toInt()) {
            val child = node.child(i.toUInt())
            if (child != null) {
                stack.addLast(child)
            }
        }
    }

    // Walk uses a stack (LIFO), so re-sort by position to restore source order.
    commandNodes.sortBy { it.startByte }

    val commands = mutableListOf<List<String>>()
    for (node in commandNodes) {
        val words = parsePlainCommandFromNode(node, src)
        if (words != null) {
            commands.add(words)
        } else {
            return null
        }
    }
    return commands
}

fun extractBashCommand(command: List<String>): Pair<String, String>? {
    if (command.size != 3) {
        return null
    }
    val shell = command[0]
    val flag = command[1]
    val script = command[2]

    if (flag != "-lc" && flag != "-c") {
        return null
    }

    val shellDetector = ShellDetector()
    val shellType = shellDetector.detectShellType(shell)
    if (shellType != ShellType.Zsh && shellType != ShellType.Bash && shellType != ShellType.Sh) {
        return null
    }

    return Pair(shell, script)
}

/**
 * Returns the sequence of plain commands within a `bash -lc "..."` or
 * `zsh -lc "..."` invocation when the script only contains word-only commands
 * joined by safe operators.
 */
fun parseShellLcPlainCommands(command: List<String>): List<List<String>>? {
    val (_, script) = extractBashCommand(command) ?: return null

    val tree = tryParseShell(script) ?: return null
    return tryParseWordOnlyCommandsSequence(tree, script)
}

private fun parsePlainCommandFromNode(cmd: Node, src: String): List<String>? {
    if (cmd.type != "command") {
        return null
    }
    val words = mutableListOf<String>()

    for (i in 0 until cmd.namedChildCount.toInt()) {
        val child = cmd.namedChild(i.toUInt()) ?: continue
        when (child.type) {
            "command_name" -> {
                val wordNode = child.namedChild(0u) ?: return null
                if (wordNode.type != "word") {
                    return null
                }
                val text = extractNodeText(wordNode, src) ?: return null
                words.add(text)
            }
            "word", "number" -> {
                val text = extractNodeText(child, src) ?: return null
                words.add(text)
            }
            "string" -> {
                if (child.childCount.toInt() == 3) {
                    val c0 = child.child(0u)
                    val c1 = child.child(1u)
                    val c2 = child.child(2u)
                    if (c0?.type == "\"" && c1?.type == "string_content" && c2?.type == "\"") {
                        val text = extractNodeText(c1, src) ?: return null
                        words.add(text)
                    } else {
                        return null
                    }
                } else {
                    return null
                }
            }
            "raw_string" -> {
                val rawString = extractNodeText(child, src) ?: return null
                val stripped = if (rawString.startsWith('\'') && rawString.endsWith('\'')) {
                    rawString.drop(1).dropLast(1)
                } else {
                    return null
                }
                words.add(stripped)
            }
            else -> return null
        }
    }
    return words
}

private fun extractNodeText(node: Node, src: String): String? {
    return try {
        val start = node.startByte.toInt()
        val end = node.endByte.toInt()
        if (start >= 0 && end <= src.length && start <= end) {
            src.substring(start, end)
        } else {
            null
        }
    } catch (_: Exception) {
        null
    }
}
