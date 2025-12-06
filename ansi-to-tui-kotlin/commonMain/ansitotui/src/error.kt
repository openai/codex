/**
 * Error types for ANSI parsing.
 *
 * This module defines the [AnsiError] sealed class hierarchy for errors
 * that can occur during ANSI parsing.
 */
package ansitotui

/**
 * Error types for ANSI parsing.
 *
 * All parsing errors extend this sealed class to allow exhaustive
 * when-expression matching.
 */
sealed class AnsiError : Exception() {
    /**
     * Parser error (should never happen).
     */
    data class ParseError(override val message: String) : AnsiError() {
        override fun toString(): String = "Internal error: $message"
    }

    /**
     * Error parsing the input as UTF-8.
     */
    data class Utf8Error(override val cause: Throwable) : AnsiError() {
        override fun toString(): String = "Utf8Error: ${cause.message}"
    }
}
