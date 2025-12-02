// port-lint: source codex-rs/protocol/src/config_types.rs
package ai.solace.coder.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Configuration type enums.
 *
 * Ported from Rust codex-rs/protocol/src/config_types.rs
 */

/**
 * See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#get-started-with-reasoning
 */
@Serializable
enum class ReasoningEffort {
    @SerialName("none")
    None,

    @SerialName("minimal")
    Minimal,

    @SerialName("low")
    Low,

    @SerialName("medium")
    Medium,

    @SerialName("high")
    High,

    @SerialName("xhigh")
    XHigh
}

/**
 * A summary of the reasoning performed by the model.
 * See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#reasoning-summaries
 */
@Serializable
enum class ReasoningSummary {
    @SerialName("auto")
    Auto,

    @SerialName("concise")
    Concise,

    @SerialName("detailed")
    Detailed,

    @SerialName("none")
    None
}

/**
 * Controls output length/detail on GPT-5 models via the Responses API.
 */
@Serializable
enum class Verbosity {
    @SerialName("low")
    Low,

    @SerialName("medium")
    Medium,

    @SerialName("high")
    High
}

/**
 * Sandbox mode for command execution.
 */
@Serializable
enum class SandboxMode {
    @SerialName("read-only")
    ReadOnly,

    @SerialName("workspace-write")
    WorkspaceWrite,

    @SerialName("danger-full-access")
    DangerFullAccess
}

/**
 * Forced login method.
 */
@Serializable
enum class ForcedLoginMethod {
    @SerialName("chatgpt")
    Chatgpt,

    @SerialName("api")
    Api
}

/**
 * Represents the trust level for a project directory.
 * This determines the approval policy and sandbox mode applied.
 */
@Serializable
enum class TrustLevel {
    @SerialName("trusted")
    Trusted,

    @SerialName("untrusted")
    Untrusted
}

/**
 * Type alias matching Rust: `use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig`
 */
typealias ReasoningEffortConfig = ReasoningEffort

/**
 * Type alias matching Rust: `use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig`
 */
typealias ReasoningSummaryConfig = ReasoningSummary
