/**
 * ANSI escape sequence parser.
 *
 * This module provides the core parsing logic for converting byte sequences containing
 * ANSI escape codes into ratatui [Text] objects with appropriate styling.
 *
 * The parser supports:
 * - SGR (Select Graphic Rendition) codes for text styling
 * - 4-bit colors (standard and bright)
 * - 8-bit indexed colors (256 color palette)
 * - 24-bit true colors (RGB)
 * - Style modifiers (bold, italic, underline, blink, etc.)
 *
 * Invalid or unrecognized escape sequences are silently ignored.
 */
package ansitotui

import ratatui.style.Color
import ratatui.style.Modifier
import ratatui.style.Style
import ratatui.text.Line
import ratatui.text.Span
import ratatui.text.Text

/**
 * Color type indicator for extended colors (8-bit and 24-bit).
 */
private enum class ColorType {
    /** Eight bit color (256 color palette) */
    EightBit,
    /** 24-bit color or true color (RGB) */
    TrueColor
}

/**
 * An ANSI item with code and optional color.
 */
private data class AnsiItem(
    val code: AnsiCode,
    val color: Color? = null
)

/**
 * ANSI state accumulator.
 */
private data class AnsiStates(
    val items: MutableList<AnsiItem> = mutableListOf(),
    val style: Style = Style.default()
) {
    /**
     * Convert accumulated items to a Style.
     */
    fun toStyle(): Style {
        var result = style
        if (items.isEmpty()) {
            // [m should be treated as a reset as well
            return Style.reset()
        }
        for (item in items) {
            result = when (item.code) {
                is AnsiCode.Reset -> Style.reset()
                is AnsiCode.Bold -> result.addModifier(Modifier.BOLD)
                is AnsiCode.Faint -> result.addModifier(Modifier.DIM)
                is AnsiCode.Normal -> result.removeModifier(Modifier.BOLD or Modifier.DIM)
                is AnsiCode.Italic -> result.addModifier(Modifier.ITALIC)
                is AnsiCode.NotItalic -> result.removeModifier(Modifier.ITALIC)
                is AnsiCode.Underline -> result.addModifier(Modifier.UNDERLINED)
                is AnsiCode.UnderlineOff -> result.removeModifier(Modifier.UNDERLINED)
                is AnsiCode.SlowBlink -> result.addModifier(Modifier.SLOW_BLINK)
                is AnsiCode.RapidBlink -> result.addModifier(Modifier.RAPID_BLINK)
                is AnsiCode.BlinkOff -> result.removeModifier(Modifier.SLOW_BLINK or Modifier.RAPID_BLINK)
                is AnsiCode.Reverse -> result.addModifier(Modifier.REVERSED)
                is AnsiCode.Conceal -> result.addModifier(Modifier.HIDDEN)
                is AnsiCode.Reveal -> result.removeModifier(Modifier.HIDDEN)
                is AnsiCode.CrossedOut -> result.addModifier(Modifier.CROSSED_OUT)
                is AnsiCode.CrossedOutOff -> result.removeModifier(Modifier.CROSSED_OUT)
                is AnsiCode.DefaultForegroundColor -> result.fg(Color.Reset)
                is AnsiCode.DefaultBackgroundColor -> result.bg(Color.Reset)
                is AnsiCode.SetForegroundColor -> {
                    item.color?.let { result.fg(it) } ?: result
                }
                is AnsiCode.SetBackgroundColor -> {
                    item.color?.let { result.bg(it) } ?: result
                }
                is AnsiCode.ForegroundColor -> result.fg(item.code.color)
                is AnsiCode.BackgroundColor -> result.bg(item.code.color)
                else -> result
            }
        }
        return result
    }
}

/**
 * Parser state for tracking position in byte array.
 *
 * This class provides low-level parsing utilities for navigating through
 * byte sequences, similar to parser combinator libraries.
 *
 * @property data The byte array being parsed.
 */
private class Parser(private val data: ByteArray) {
    var pos: Int = 0

    val remaining: Int get() = data.size - pos
    val isAtEnd: Boolean get() = pos >= data.size

    fun peek(): Byte? = if (pos < data.size) data[pos] else null
    fun peekChar(): Char? = peek()?.toInt()?.toChar()

    fun advance(): Byte? {
        return if (pos < data.size) data[pos++] else null
    }

    fun advanceChar(): Char? = advance()?.toInt()?.toChar()

    fun slice(start: Int, end: Int): ByteArray = data.sliceArray(start until end)

    fun takeWhile(predicate: (Byte) -> Boolean): ByteArray {
        val start = pos
        while (pos < data.size && predicate(data[pos])) {
            pos++
        }
        return slice(start, pos)
    }

    fun takeUntil(predicate: (Byte) -> Boolean): ByteArray {
        val start = pos
        while (pos < data.size && !predicate(data[pos])) {
            pos++
        }
        return slice(start, pos)
    }

    fun expect(b: Byte): Boolean {
        if (peek() == b) {
            advance()
            return true
        }
        return false
    }

    fun expectChar(c: Char): Boolean = expect(c.code.toByte())

    fun expectTag(tag: String): Boolean {
        if (pos + tag.length > data.size) return false
        for (i in tag.indices) {
            if (data[pos + i] != tag[i].code.toByte()) return false
        }
        pos += tag.length
        return true
    }

    fun parseUByte(): UByte? {
        val start = pos
        while (pos < data.size && data[pos] in '0'.code.toByte()..'9'.code.toByte()) {
            pos++
        }
        if (start == pos) return null
        val str = slice(start, pos).decodeToString()
        return str.toUByteOrNull()
    }

    fun parseInt(): Int? {
        val start = pos
        if (pos < data.size && data[pos] == '-'.code.toByte()) pos++
        while (pos < data.size && data[pos] in '0'.code.toByte()..'9'.code.toByte()) {
            pos++
        }
        if (start == pos) return null
        val str = slice(start, pos).decodeToString()
        return str.toIntOrNull()
    }

    fun skipOptionalSemicolon() {
        if (peek() == ';'.code.toByte()) advance()
    }
}

/**
 * Parse a byte array containing ANSI escape sequences into a [Text].
 *
 * This is the main entry point for ANSI parsing. It processes the entire
 * byte array line by line, accumulating styled spans into a [Text] object.
 *
 * @param data The byte array to parse.
 * @return A [Text] object with styled lines and spans.
 */
internal fun parseText(data: ByteArray): Text {
    val lines = mutableListOf<Line>()
    var lastStyle = Style.default()
    val parser = Parser(data)

    while (!parser.isAtEnd) {
        val (line, style) = parseLine(parser, lastStyle)
        lines.add(line)
        lastStyle = style
    }

    return Text.from(lines)
}

/**
 * Parse a single line from the parser.
 *
 * @param parser The parser state.
 * @param style The current style to apply to spans.
 * @return A pair of the parsed [Line] and the style at end of line.
 */
private fun parseLine(parser: Parser, style: Style): Pair<Line, Style> {
    // Take until newline
    val lineData = parser.takeUntil { it == '\n'.code.toByte() }

    // Skip newline if present
    if (parser.peek() == '\n'.code.toByte()) {
        parser.advance()
    }

    val spans = mutableListOf<Span>()
    var lastStyle = style
    val lineParser = Parser(lineData)

    while (!lineParser.isAtEnd) {
        val span = parseSpan(lineParser, lastStyle)
        lastStyle = lastStyle.patch(span.style)
        if (span.content.isNotEmpty()) {
            spans.add(span)
        }
    }

    return Pair(Line.from(spans), lastStyle)
}

/**
 * Parse a single span from the parser.
 *
 * A span consists of an optional style escape sequence followed by text content.
 *
 * @param parser The parser state.
 * @param lastStyle The style from the previous span.
 * @return The parsed [Span] with content and style.
 */
private fun parseSpan(parser: Parser, lastStyle: Style): Span {
    var currentStyle = lastStyle

    // Try to parse style escape sequence
    val styleResult = parseStyle(parser, currentStyle)
    if (styleResult != null) {
        currentStyle = currentStyle.patch(styleResult)
    }

    // Take text until escape or newline
    val textData = parser.takeWhile { it != ESC && it != '\n'.code.toByte() }
    val text = try {
        textData.decodeToString()
    } catch (e: Exception) {
        // Invalid UTF-8, skip
        ""
    }

    return Span.styled(text, currentStyle)
}

private const val ESC: Byte = 0x1B

/**
 * Parse a style escape sequence (SGR - Select Graphic Rendition).
 *
 * SGR sequences have the format: ESC [ <params> m
 * where params are semicolon-separated numeric codes.
 *
 * @param parser The parser state.
 * @param style The current style to modify.
 * @return The new style if a valid SGR sequence was parsed, null otherwise.
 */
private fun parseStyle(parser: Parser, style: Style): Style? {
    if (parser.peek() != ESC) return null

    val startPos = parser.pos
    parser.advance() // consume ESC

    // Check for CSI (Control Sequence Introducer) = ESC[
    if (parser.peek() != '['.code.toByte()) {
        // Try to consume other escape sequences
        consumeAnyEscapeSequence(parser)
        return null
    }
    parser.advance() // consume [

    // Parse SGR (Select Graphic Rendition) codes
    val items = mutableListOf<AnsiItem>()

    while (!parser.isAtEnd && parser.peek() != 'm'.code.toByte()) {
        val item = parseSgrItem(parser) ?: break
        items.add(item)
        parser.skipOptionalSemicolon()
    }

    // Expect 'm' terminator
    if (parser.peek() != 'm'.code.toByte()) {
        // Not a valid SGR sequence, try to recover
        parser.pos = startPos + 1 // skip just the ESC
        consumeAnyEscapeSequence(parser)
        return null
    }
    parser.advance() // consume 'm'

    return AnsiStates(items, style).toStyle()
}

/**
 * Consume any escape sequence we don't understand.
 *
 * This handles CSI sequences (ESC [) and OSC sequences (ESC ])
 * that we don't specifically parse, preventing them from appearing
 * as garbage in the output text.
 *
 * @param parser The parser state.
 */
private fun consumeAnyEscapeSequence(parser: Parser) {
    val nextChar = parser.peek() ?: return

    when (nextChar.toInt().toChar()) {
        '[' -> {
            // CSI sequence: consume until alpha character
            parser.advance()
            parser.takeUntil { it.toInt().toChar().isLetter() }
            if (!parser.isAtEnd) parser.advance() // consume terminator
        }
        ']' -> {
            // OSC sequence: consume until BEL (0x07) or ST (ESC \)
            parser.advance()
            parser.takeUntil { it == 0x07.toByte() }
            if (!parser.isAtEnd) parser.advance() // consume BEL
        }
        else -> {
            // Unknown sequence, just skip one character
        }
    }
}

/**
 * Parse a single SGR item (one numeric code in the sequence).
 *
 * @param parser The parser state.
 * @return The parsed [AnsiItem] or null if parsing failed.
 */
private fun parseSgrItem(parser: Parser): AnsiItem? {
    val code = parser.parseUByte() ?: return null
    val ansiCode = AnsiCode.from(code)

    val color = when (ansiCode) {
        is AnsiCode.SetForegroundColor, is AnsiCode.SetBackgroundColor -> {
            parser.skipOptionalSemicolon()
            parseColor(parser)
        }
        else -> null
    }

    return AnsiItem(ansiCode, color)
}

/**
 * Parse an extended color (8-bit indexed or 24-bit RGB).
 *
 * Extended colors follow code 38 (foreground) or 48 (background) and have the format:
 * - 8-bit: `38;5;N` where N is 0-255
 * - 24-bit: `38;2;R;G;B` where R, G, B are 0-255
 *
 * @param parser The parser state.
 * @return The parsed [Color] or null if parsing failed.
 */
private fun parseColor(parser: Parser): Color? {
    val colorType = parseColorType(parser) ?: return null
    parser.skipOptionalSemicolon()

    return when (colorType) {
        ColorType.TrueColor -> {
            val r = parser.parseUByte() ?: return null
            if (!parser.expectChar(';')) return null
            val g = parser.parseUByte() ?: return null
            if (!parser.expectChar(';')) return null
            val b = parser.parseUByte() ?: return null
            Color.Rgb(r, g, b)
        }
        ColorType.EightBit -> {
            val index = parser.parseUByte() ?: return null
            Color.Indexed(index)
        }
    }
}

/**
 * Parse the color type indicator (2 for RGB, 5 for indexed).
 *
 * @param parser The parser state.
 * @return The [ColorType] or null if parsing failed.
 */
private fun parseColorType(parser: Parser): ColorType? {
    val t = parser.parseInt() ?: return null
    if (!parser.expectChar(';')) return null

    return when (t) {
        2 -> ColorType.TrueColor
        5 -> ColorType.EightBit
        else -> null
    }
}
