// port-lint: source protocol/src/account.rs
package ai.solace.coder.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Account plan type.
 *
 * Ported from Rust codex-rs/protocol/src/account.rs
 */
@Serializable
enum class PlanType {
    @SerialName("free")
    Free,

    @SerialName("plus")
    Plus,

    @SerialName("pro")
    Pro,

    @SerialName("team")
    Team,

    @SerialName("business")
    Business,

    @SerialName("enterprise")
    Enterprise,

    @SerialName("edu")
    Edu,

    @SerialName("unknown")
    Unknown
}
