package ai.solace.coder.core

/**
 * Error type for function call failures.
 *
 * Represents errors that can occur when executing function/tool calls.
 */
sealed class FunctionCallError : Exception() {
    /**
     * Error message to send back to the model.
     */
    data class RespondToModel(val message: String) : FunctionCallError() {
        override val message: String get() = this.message
        override fun toString(): String = message
    }

    /**
     * The function call was denied (e.g., by user or policy).
     */
    data class Denied(val reason: String) : FunctionCallError() {
        override val message: String get() = reason
        override fun toString(): String = reason
    }

    /**
     * LocalShellCall is missing a call_id or id.
     */
    data object MissingLocalShellCallId : FunctionCallError() {
        override val message: String get() = "LocalShellCall without call_id or id"
        override fun toString(): String = message
    }

    /**
     * A fatal error that cannot be recovered from.
     */
    data class Fatal(val reason: String) : FunctionCallError() {
        override val message: String get() = "Fatal error: $reason"
        override fun toString(): String = message
    }
}
