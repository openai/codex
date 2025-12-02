// port-lint: source codex-rs/protocol/src/user_input.rs
package ai.solace.coder.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * User input types.
 *
 * Ported from Rust codex-rs/protocol/src/user_input.rs
 */

@Serializable
sealed class UserInput {
    @Serializable
    @SerialName("text")
    data class Text(
        val text: String
    ) : UserInput()

    /**
     * Pre-encoded data: URI image.
     */
    @Serializable
    @SerialName("image")
    data class Image(
        @SerialName("image_url")
        val imageUrl: String
    ) : UserInput()

    /**
     * Local image path provided by the user. This will be converted to an
     * `Image` variant (base64 data URL) during request serialization.
     */
    @Serializable
    @SerialName("local_image")
    data class LocalImage(
        val path: String
    ) : UserInput()
}
