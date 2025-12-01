// port-lint: source protocol/src/custom_prompts.rs
package ai.solace.coder.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Custom prompts types.
 *
 * Ported from Rust codex-rs/protocol/src/custom_prompts.rs
 */

/**
 * Base namespace for custom prompt slash commands (without trailing colon).
 * Example usage forms constructed in code:
 * - Command token after '/': "{PROMPTS_CMD_PREFIX}:name"
 * - Full slash prefix: "/{PROMPTS_CMD_PREFIX}:"
 */
const val PROMPTS_CMD_PREFIX = "prompts"

@Serializable
data class CustomPrompt(
    val name: String,
    val path: String,
    val content: String,
    val description: String? = null,
    @SerialName("argument_hint")
    val argumentHint: String? = null
)
