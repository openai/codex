// port-lint: source codex-rs/protocol/src/parse_command.rs
package ai.solace.coder.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Parsed command types.
 *
 * Ported from Rust codex-rs/protocol/src/parse_command.rs
 */

@Serializable
sealed class ParsedCommand {
    @Serializable
    @SerialName("read")
    data class Read(
        val cmd: String,
        val name: String,
        /**
         * (Best effort) Path to the file being read by the command. When
         * possible, this is an absolute path, though when relative, it should
         * be resolved against the `cwd` that will be used to run the command
         * to derive the absolute path.
         */
        val path: String
    ) : ParsedCommand()

    @Serializable
    @SerialName("list_files")
    data class ListFiles(
        val cmd: String,
        val path: String? = null
    ) : ParsedCommand()

    @Serializable
    @SerialName("search")
    data class Search(
        val cmd: String,
        val query: String? = null,
        val path: String? = null
    ) : ParsedCommand()

    @Serializable
    @SerialName("unknown")
    data class Unknown(
        val cmd: String
    ) : ParsedCommand()
}
