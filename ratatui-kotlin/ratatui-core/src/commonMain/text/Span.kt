package ratatui.text

import ratatui.buffer.Buffer
import ratatui.layout.Rect
import ratatui.style.Color
import ratatui.style.Modifier
import ratatui.style.Style
import ratatui.style.Styled
import ratatui.widgets.Widget

/**
 * Represents a part of a line that is contiguous and where all characters share the same style.
 *
 * A `Span` is the smallest unit of text that can be styled. It is usually combined in the [Line]
 * type to represent a line of text where each `Span` may have a different style.
 *
 * # Constructor Methods
 *
 * - [Span.default] creates a span with empty content and the default style.
 * - [Span.raw] creates a span with the specified content and the default style.
 * - [Span.styled] creates a span with the specified content and style.
 *
 * # Setter Methods
 *
 * These methods are fluent setters. They return a new `Span` with the specified property set.
 *
 * - [Span.content] sets the content of the span.
 * - [Span.style] sets the style of the span.
 *
 * # Other Methods
 *
 * - [Span.patchStyle] patches the style of the span, adding modifiers from the given style.
 * - [Span.resetStyle] resets the style of the span.
 * - [Span.width] returns the unicode width of the content held by this span.
 * - [Span.styledGraphemes] returns a list of graphemes held by this span.
 *
 * # Examples
 *
 * A `Span` with `style` set to [Style.default] can be created from a `String`.
 *
 * ```kotlin
 * val span = Span.raw("test content")
 * val span = Span.from("test content")
 * ```
 *
 * Styled spans can be created using [Span.styled] or by converting strings using methods from
 * the [Stylize] interface.
 *
 * ```kotlin
 * val span = Span.styled("test content", Style.new().green())
 *
 * // using Stylize trait shortcuts
 * val span = "test content".green()
 * ```
 *
 * `Span` implements the [Styled] interface, which allows it to be styled using the shortcut methods
 * defined in the [Stylize] interface.
 *
 * ```kotlin
 * val span = Span.raw("test content").green().onYellow().italic()
 * ```
 *
 * `Span` implements the [Widget] interface, which allows it to be rendered to a [Buffer]. Often
 * apps will use the `Paragraph` widget instead of rendering `Span` directly, as it handles text
 * wrapping and alignment for you.
 */
data class Span(
    /** The content of the span. */
    val content: String = "",
    /** The style of the span. */
    val style: Style = Style.default()
) : Styled<Span>, Widget {

    /**
     * Sets the content of the span.
     *
     * This is a fluent setter method which must be chained or used as it returns a new Span.
     *
     * # Examples
     *
     * ```kotlin
     * val span = Span.default().content("content")
     * ```
     */
    fun content(content: String): Span = copy(content = content)

    /**
     * Sets the style of the span.
     *
     * This is a fluent setter method which must be chained or used as it returns a new Span.
     *
     * In contrast to [patchStyle], this method replaces the style of the span instead of
     * patching it.
     *
     * `style` accepts any type that is convertible to [Style] (e.g. [Style], [Color], or
     * your own type that implements conversion to Style).
     *
     * # Examples
     *
     * ```kotlin
     * val span = Span.default().style(Style.new().green())
     * ```
     */
    fun style(style: Style): Span = copy(style = style)

    /**
     * Patches the style of the Span, adding modifiers from the given style.
     *
     * This is a fluent setter method which must be chained or used as it returns a new Span.
     *
     * # Example
     *
     * ```kotlin
     * val span = Span.styled("test content", Style.new().green().italic())
     *     .patchStyle(Style.new().red().onYellow().bold())
     * assertEquals(span.style, Style.new().red().onYellow().italic().bold())
     * ```
     */
    fun patchStyle(style: Style): Span = copy(style = this.style.patch(style))

    /**
     * Resets the style of the Span.
     *
     * This is equivalent to calling `patchStyle(Style.reset())`.
     *
     * This is a fluent setter method which must be chained or used as it returns a new Span.
     *
     * # Example
     *
     * ```kotlin
     * val span = Span.styled(
     *     "Test Content",
     *     Style.new().darkGray().onYellow().italic(),
     * ).resetStyle()
     * assertEquals(span.style, Style.reset())
     * ```
     */
    fun resetStyle(): Span = patchStyle(Style.reset())

    /**
     * Returns the unicode width of the content held by this span.
     */
    fun width(): Int = unicodeWidth(content)

    /**
     * Returns a list of graphemes held by this span.
     *
     * `baseStyle` is the [Style] that will be patched with the `Span`'s `style` to get the
     * resulting [Style].
     *
     * # Example
     *
     * ```kotlin
     * val span = Span.styled("Test", Style.new().green().italic())
     * val style = Style.new().red().onYellow()
     * assertEquals(
     *     span.styledGraphemes(style),
     *     listOf(
     *         StyledGrapheme("T", Style.new().green().onYellow().italic()),
     *         StyledGrapheme("e", Style.new().green().onYellow().italic()),
     *         StyledGrapheme("s", Style.new().green().onYellow().italic()),
     *         StyledGrapheme("t", Style.new().green().onYellow().italic()),
     *     ),
     * )
     * ```
     */
    fun styledGraphemes(baseStyle: Style = Style.default()): List<StyledGrapheme> {
        val patchedStyle = baseStyle.patch(style)
        return graphemes(content)
            .filter { g -> !g.any { it.isISOControl() } }
            .map { g -> StyledGrapheme(g, patchedStyle) }
    }

    /**
     * Converts this Span into a left-aligned [Line]
     *
     * # Example
     *
     * ```kotlin
     * val line = "Test Content".green().italic().intoLeftAlignedLine()
     * ```
     */
    fun intoLeftAlignedLine(): Line = Line.from(this).leftAligned()

    /**
     * Converts this Span into a center-aligned [Line]
     *
     * # Example
     *
     * ```kotlin
     * val line = "Test Content".green().italic().intoCenteredLine()
     * ```
     */
    fun intoCenteredLine(): Line = Line.from(this).centered()

    /**
     * Converts this Span into a right-aligned [Line]
     *
     * # Example
     *
     * ```kotlin
     * val line = "Test Content".green().italic().intoRightAlignedLine()
     * ```
     */
    fun intoRightAlignedLine(): Line = Line.from(this).rightAligned()

    // Styled implementation
    override fun getStyle(): Style = style

    override fun setStyle(style: Style): Span = style(style)

    // Widget implementation
    override fun render(area: Rect, buf: Buffer) {
        val clippedArea = area.intersection(buf.area)
        if (clippedArea.isEmpty()) {
            return
        }
        var x = clippedArea.x
        val y = clippedArea.y
        for ((i, grapheme) in styledGraphemes(Style.default()).withIndex()) {
            val symbolWidth = unicodeWidth(grapheme.symbol).toUShort()
            val nextX = (x + symbolWidth).coerceAtMost(UShort.MAX_VALUE)
            if (nextX > clippedArea.right()) {
                break
            }

            if (i == 0) {
                // the first grapheme is always set on the cell
                buf[x, y]
                    .setSymbol(grapheme.symbol)
                    .setStyle(grapheme.style)
            } else if (x == clippedArea.x) {
                // there is one or more zero-width graphemes in the first cell, so the first cell
                // must be appended to.
                buf[x, y]
                    .appendSymbol(grapheme.symbol)
                    .setStyle(grapheme.style)
            } else if (symbolWidth.toInt() == 0) {
                // append zero-width graphemes to the previous cell
                buf[(x - 1u).toUShort(), y]
                    .appendSymbol(grapheme.symbol)
                    .setStyle(grapheme.style)
            } else {
                // just a normal grapheme (not first, not zero-width, not overflowing the area)
                buf[x, y]
                    .setSymbol(grapheme.symbol)
                    .setStyle(grapheme.style)
            }

            // multi-width graphemes must clear the cells of characters that are hidden by the
            // grapheme, otherwise the hidden characters will be re-rendered if the grapheme is
            // overwritten.
            for (xHidden in (x + 1u).toUShort() until nextX) {
                // it may seem odd that the style of the hidden cells are not set to the style of
                // the grapheme, but this is how the existing buffer.setSpan() method works.
                buf[xHidden, y].reset()
            }
            x = nextX
        }
    }

    override fun toString(): String {
        return content.lines().joinToString("")
    }

    /**
     * Operator + to combine two Spans into a Line.
     */
    operator fun plus(other: Span): Line = Line.fromIter(listOf(this, other))

    companion object {
        /**
         * Create a span with empty content and default style.
         */
        fun default(): Span = Span()

        /**
         * Create a span with the default style.
         *
         * # Examples
         *
         * ```kotlin
         * Span.raw("test content")
         * ```
         */
        fun raw(content: String): Span = Span(content = content, style = Style.default())

        /**
         * Create a span with the specified style.
         *
         * # Examples
         *
         * ```kotlin
         * val style = Style.new().yellow().onGreen().italic()
         * Span.styled("test content", style)
         * ```
         */
        fun styled(content: String, style: Style): Span = Span(content = content, style = style)

        /**
         * Create a span from a string (alias for [raw]).
         */
        fun from(content: String): Span = raw(content)

        /**
         * Create a span from another span.
         */
        fun from(span: Span): Span = span.copy()
    }
}

/**
 * A styled grapheme - a single grapheme cluster with an associated style.
 */
data class StyledGrapheme(
    val symbol: String,
    val style: Style
) {
    companion object {
        fun new(symbol: String, style: Style): StyledGrapheme = StyledGrapheme(symbol, style)
    }
}

/**
 * A trait for converting a value to a [Span].
 *
 * This interface provides a way to convert any displayable type to a Span.
 */
interface ToSpan {
    /**
     * Converts the value to a [Span].
     */
    fun toSpan(): Span
}

// Extension function to convert any type to a Span
fun Any.toSpan(): Span = Span.raw(this.toString())

/**
 * Calculate the unicode display width of a string.
 * This is a simplified implementation - for full Unicode support,
 * consider using a library like ICU4C.
 */
internal fun unicodeWidth(s: String): Int {
    var width = 0
    for (char in s) {
        width += when {
            char.isISOControl() -> 0
            // CJK characters are typically 2 columns wide
            char.code in 0x4E00..0x9FFF -> 2  // CJK Unified Ideographs
            char.code in 0x3400..0x4DBF -> 2  // CJK Unified Ideographs Extension A
            char.code in 0x20000..0x2A6DF -> 2  // CJK Unified Ideographs Extension B
            char.code in 0xF900..0xFAFF -> 2  // CJK Compatibility Ideographs
            char.code in 0xFF00..0xFF60 -> 2  // Fullwidth Forms
            char.code in 0xFFE0..0xFFE6 -> 2  // Fullwidth Forms
            // Zero-width characters
            char.code == 0x200B -> 0  // Zero Width Space
            char.code == 0x200C -> 0  // Zero Width Non-Joiner
            char.code == 0x200D -> 0  // Zero Width Joiner
            char.code == 0x200E -> 0  // Left-to-Right Mark
            char.code == 0x200F -> 0  // Right-to-Left Mark
            char.code == 0xFEFF -> 0  // Zero Width No-Break Space
            // Combining characters
            char.code in 0x0300..0x036F -> 0  // Combining Diacritical Marks
            char.code in 0x1AB0..0x1AFF -> 0  // Combining Diacritical Marks Extended
            char.code in 0x1DC0..0x1DFF -> 0  // Combining Diacritical Marks Supplement
            char.code in 0x20D0..0x20FF -> 0  // Combining Diacritical Marks for Symbols
            char.code in 0xFE20..0xFE2F -> 0  // Combining Half Marks
            else -> 1
        }
    }
    return width
}

/**
 * Split a string into grapheme clusters.
 * This is a simplified implementation that handles basic cases.
 * For full Unicode grapheme cluster support, consider using ICU4C.
 */
internal fun graphemes(s: String): List<String> {
    if (s.isEmpty()) return emptyList()

    val result = mutableListOf<String>()
    val chars = s.toList()
    var i = 0

    while (i < chars.size) {
        val sb = StringBuilder()
        sb.append(chars[i])
        i++

        // Consume any following combining characters or zero-width joiners
        while (i < chars.size) {
            val code = chars[i].code
            val isCombining = code in 0x0300..0x036F ||
                code in 0x1AB0..0x1AFF ||
                code in 0x1DC0..0x1DFF ||
                code in 0x20D0..0x20FF ||
                code in 0xFE20..0xFE2F ||
                code == 0x200D ||  // Zero Width Joiner
                code == 0x200B ||  // Zero Width Space
                code == 0x200C ||  // Zero Width Non-Joiner
                code == 0x200E ||  // Left-to-Right Mark
                code == 0x200F     // Right-to-Left Mark

            if (isCombining) {
                sb.append(chars[i])
                i++
            } else {
                break
            }
        }

        result.add(sb.toString())
    }

    return result
}
