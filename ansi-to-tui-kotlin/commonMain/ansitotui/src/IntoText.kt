/**
 * Parses a [ByteArray] as a byte sequence with ANSI colors to [ratatui.text.Text].
 *
 * Invalid ANSI colors / sequences will be ignored.
 *
 * ## Supported features
 * - UTF-8 parsing
 * - Most stuff like **Bold** / *Italic* / Underline / ~~Strikethrough~~
 * - Supports 4-bit color palettes
 * - Supports 8-bit color
 * - Supports True color (RGB / 24-bit color)
 *
 * ## Example
 *
 * ```kotlin
 * val bytes = "\u001b[38;2;225;192;203mAAAAA\u001b[0m".encodeToByteArray()
 * val text = bytes.intoText()
 * ```
 *
 * Example parsing from a string:
 *
 * ```kotlin
 * val content = "Hello \u001b[31mRed\u001b[0m World"
 * val text = content.intoText()
 * ```
 */
package ansitotui

import ratatui.text.Text

/**
 * Convert a [ByteArray] containing ANSI escape sequences to a [Text].
 *
 * Invalid ANSI sequences are ignored.
 *
 * @return The parsed [Text] with styles applied.
 * @throws AnsiError.Utf8Error if the input contains invalid UTF-8 sequences.
 */
fun ByteArray.intoText(): Text = parseText(this)

/**
 * Convert a [String] containing ANSI escape sequences to a [Text].
 *
 * Invalid ANSI sequences are ignored.
 *
 * @return The parsed [Text] with styles applied.
 */
fun String.intoText(): Text = this.encodeToByteArray().intoText()
