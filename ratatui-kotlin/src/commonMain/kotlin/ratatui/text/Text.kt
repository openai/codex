package ratatui.text

import ratatui.buffer.Buffer
import ratatui.layout.Alignment
import ratatui.layout.Rect
import ratatui.style.Style
import ratatui.style.Styled
import ratatui.widgets.Widget

/**
 * A string split over one or more lines.
 *
 * [Text] is used wherever text is displayed in the terminal and represents one or more [Line]s
 * of text. When a [Text] is rendered, each line is rendered as a single line of text from top to
 * bottom of the area. The text can be styled and aligned.
 *
 * # Constructor Methods
 *
 * - [Text.raw] creates a `Text` (potentially multiple lines) with no style.
 * - [Text.styled] creates a `Text` (potentially multiple lines) with a style.
 * - [Text.default] creates a `Text` with empty content and the default style.
 *
 * # Conversion Methods
 *
 * - [Text.from] creates a `Text` from a [String].
 * - [Text.from] creates a `Text` from a [Span].
 * - [Text.from] creates a `Text` from a [Line].
 * - [Text.from] creates a `Text` from a `List<Line>`.
 * - [Text.fromIter] creates a `Text` from an iterator of items that can be converted into `Line`.
 *
 * # Setter Methods
 *
 * These methods are fluent setters. They return a `Text` with the property set.
 *
 * - [Text.style] sets the style of this `Text`.
 * - [Text.alignment] sets the alignment for this `Text`.
 * - [Text.leftAligned] sets the alignment to [Alignment.Left].
 * - [Text.centered] sets the alignment to [Alignment.Center].
 * - [Text.rightAligned] sets the alignment to [Alignment.Right].
 *
 * # Iteration Methods
 *
 * - [Text.iterator] returns an iterator over the lines of the text.
 *
 * # Other Methods
 *
 * - [Text.width] returns the max width of all the lines.
 * - [Text.height] returns the height.
 * - [Text.patchStyle] patches the style of this `Text`, adding modifiers from the given style.
 * - [Text.resetStyle] resets the style of the `Text`.
 * - [Text.pushLine] adds a line to the text.
 * - [Text.pushSpan] adds a span to the last line of the text.
 *
 * # Examples
 *
 * ## Creating Text
 *
 * A [Text], like a [Line], can be constructed using one of the many `from` methods or
 * via the [Text.raw] and [Text.styled] methods.
 *
 * ```kotlin
 * val style = Style.new().yellow().italic()
 * val text = Text.raw("The first line\nThe second line").style(style)
 * val text = Text.styled("The first line\nThe second line", style)
 *
 * val text = Text.from("The first line\nThe second line")
 * val text = Text.from(Span.styled("The first line\nThe second line", style))
 * val text = Text.from(Line.from("The first line"))
 * val text = Text.from(listOf(
 *     Line.from("The first line"),
 *     Line.from("The second line"),
 * ))
 * val text = Text.fromIter(listOf("The first line", "The second line").map { Line.from(it) })
 *
 * val text = Text.default()
 * text.extend(listOf(
 *     Line.from("The first line"),
 *     Line.from("The second line"),
 * ))
 * text.extend(Text.from("The third line\nThe fourth line"))
 * ```
 *
 * ## Styling Text
 *
 * The text's [Style] is used by the rendering widget to determine how to style the text. Each
 * [Line] in the text will be styled with the [Style] of the text, and then with its own
 * [Style]. `Text` also implements [Styled] which means you can use the methods of the
 * `Stylize` trait.
 *
 * ```kotlin
 * val text = Text.from("The first line\nThe second line").style(Style.new().yellow().italic())
 * val text = Text.from("The first line\nThe second line").yellow().italic()
 * val text = Text.from(listOf(
 *     Line.from("The first line").yellow(),
 *     Line.from("The second line").yellow(),
 * )).italic()
 * ```
 *
 * ## Aligning Text
 *
 * The text's [Alignment] can be set using [Text.alignment] or the related helper methods.
 * Lines composing the text can also be individually aligned with [Line.alignment].
 *
 * ```kotlin
 * val text = Text.from("The first line\nThe second line").alignment(Alignment.Right)
 * val text = Text.from("The first line\nThe second line").rightAligned()
 * val text = Text.from(listOf(
 *     Line.from("The first line").leftAligned(),
 *     Line.from("The second line").rightAligned(),
 *     Line.from("The third line"),
 * )).centered()
 * ```
 *
 * ## Rendering Text
 *
 * `Text` implements the [Widget] trait, which means it can be rendered to a [Buffer].
 *
 * ```kotlin
 * // within another widget's render method:
 * val text = Text.from("The first line\nThe second line")
 * text.render(area, buf)
 * ```
 *
 * ## Rendering Text with a Paragraph Widget
 *
 * Usually apps will use the `Paragraph` widget instead of rendering a `Text` directly as it
 * provides more functionality.
 *
 * ```kotlin
 * val text = Text.from("The first line\nThe second line")
 * val paragraph = Paragraph.new(text)
 *     .wrap(Wrap(trim = true))
 *     .scroll(Pair(1, 1))
 * paragraph.render(area, buf)
 * ```
 */
data class Text(
    /** The style of this text. */
    val style: Style = Style.default(),

    /** The alignment of this text. */
    val alignment: Alignment? = null,

    /** The lines that make up this piece of text. */
    val lines: MutableList<Line> = mutableListOf()
) : Styled<Text>, Widget, Iterable<Line> {

    /**
     * Returns the max width of all the lines.
     *
     * # Examples
     *
     * ```kotlin
     * val text = Text.from("The first line\nThe second line")
     * assertEquals(15, text.width())
     * ```
     */
    fun width(): Int = lines.maxOfOrNull { it.width() } ?: 0

    /**
     * Returns the height.
     *
     * # Examples
     *
     * ```kotlin
     * val text = Text.from("The first line\nThe second line")
     * assertEquals(2, text.height())
     * ```
     */
    fun height(): Int = lines.size

    /**
     * Sets the style of this text.
     *
     * Defaults to [Style.default].
     *
     * Note: This field was added in v0.26.0. Prior to that, the style of a text was determined
     * only by the style of each [Line] contained in the line. For this reason, this field may
     * not be supported by all widgets (outside of the `ratatui` crate itself).
     *
     * # Examples
     * ```kotlin
     * val line = Text.from("foo").style(Style.new().red())
     * ```
     */
    fun style(style: Style): Text = copy(style = style)

    /**
     * Patches the style of this Text, adding modifiers from the given style.
     *
     * This is useful for when you want to apply a style to a text that already has some styling.
     * In contrast to [Text.style], this method will not overwrite the existing style, but
     * instead will add the given style's modifiers to this text's style.
     *
     * `Text` also implements [Styled] which means you can use the methods of the
     * `Stylize` extension functions.
     *
     * This is a fluent setter method which must be chained or used as it consumes self
     *
     * # Examples
     *
     * ```kotlin
     * val rawText = Text.styled("The first line\nThe second line", Style.default().addModifier(Modifier.ITALIC))
     * val styledText = Text.styled(
     *     "The first line\nThe second line",
     *     Style.default().fg(Color.Yellow).addModifier(Modifier.ITALIC)
     * )
     * assertNotEquals(rawText, styledText)
     *
     * val patchedText = rawText.patchStyle(Style.default().fg(Color.Yellow))
     * assertEquals(patchedText, styledText)
     * ```
     */
    fun patchStyle(style: Style): Text = copy(style = this.style.patch(style))

    /**
     * Resets the style of the Text.
     *
     * Equivalent to calling [patchStyle] with [Style.reset].
     *
     * This is a fluent setter method which must be chained or used as it consumes self
     *
     * # Examples
     *
     * ```kotlin
     * val text = Text.styled(
     *     "The first line\nThe second line",
     *     Style.default().fg(Color.Yellow).addModifier(Modifier.ITALIC)
     * )
     *
     * val resetText = text.resetStyle()
     * assertEquals(Style.reset(), resetText.style)
     * ```
     */
    fun resetStyle(): Text = patchStyle(Style.reset())

    /**
     * Sets the alignment for this text.
     *
     * Defaults to: `null`, meaning the alignment is determined by the rendering widget.
     * Setting the alignment of a Text generally overrides the alignment of its
     * parent Widget.
     *
     * Alignment can be set individually on each line to override this text's alignment.
     *
     * # Examples
     *
     * Set alignment to the whole text.
     *
     * ```kotlin
     * val text = Text.from("Hi, what's up?")
     * assertNull(text.alignment)
     * assertEquals(
     *     Alignment.Right,
     *     text.alignment(Alignment.Right).alignment
     * )
     * ```
     *
     * Set a default alignment and override it on a per line basis.
     *
     * ```kotlin
     * val text = Text.from(listOf(
     *     Line.from("left").alignment(Alignment.Left),
     *     Line.from("default"),
     *     Line.from("default"),
     *     Line.from("right").alignment(Alignment.Right),
     * )).alignment(Alignment.Center)
     * ```
     *
     * Will render the following
     *
     * ```plain
     * left
     *   default
     *   default
     *       right
     * ```
     */
    fun alignment(alignment: Alignment): Text = copy(alignment = alignment)

    /**
     * Left-aligns the whole text.
     *
     * Convenience shortcut for `Text.alignment(Alignment.Left)`.
     * Setting the alignment of a Text generally overrides the alignment of its
     * parent Widget, with the default alignment being inherited from the parent.
     *
     * Alignment can be set individually on each line to override this text's alignment.
     *
     * # Examples
     *
     * ```kotlin
     * val text = Text.from("Hi, what's up?").leftAligned()
     * ```
     */
    fun leftAligned(): Text = alignment(Alignment.Left)

    /**
     * Center-aligns the whole text.
     *
     * Convenience shortcut for `Text.alignment(Alignment.Center)`.
     * Setting the alignment of a Text generally overrides the alignment of its
     * parent Widget, with the default alignment being inherited from the parent.
     *
     * Alignment can be set individually on each line to override this text's alignment.
     *
     * # Examples
     *
     * ```kotlin
     * val text = Text.from("Hi, what's up?").centered()
     * ```
     */
    fun centered(): Text = alignment(Alignment.Center)

    /**
     * Right-aligns the whole text.
     *
     * Convenience shortcut for `Text.alignment(Alignment.Right)`.
     * Setting the alignment of a Text generally overrides the alignment of its
     * parent Widget, with the default alignment being inherited from the parent.
     *
     * Alignment can be set individually on each line to override this text's alignment.
     *
     * # Examples
     *
     * ```kotlin
     * val text = Text.from("Hi, what's up?").rightAligned()
     * ```
     */
    fun rightAligned(): Text = alignment(Alignment.Right)

    /**
     * Returns an iterator over the lines of the text.
     *
     * Note: In Kotlin, Text implements [Iterable], so you can iterate directly.
     */
    override fun iterator(): Iterator<Line> = lines.iterator()

    /**
     * Adds a line to the text.
     *
     * # Examples
     *
     * ```kotlin
     * val text = Text.from("Hello, world!")
     * text.pushLine(Line.from("How are you?"))
     * text.pushLine(Span.from("How are you?"))
     * text.pushLine("How are you?")
     * ```
     */
    fun pushLine(line: Line) {
        lines.add(line)
    }

    /**
     * Adds a line from a Span to the text.
     */
    fun pushLine(span: Span) {
        lines.add(Line.from(span))
    }

    /**
     * Adds a line from a String to the text.
     */
    fun pushLine(content: String) {
        lines.add(Line.from(content))
    }

    /**
     * Adds a span to the last line of the text.
     *
     * If the text has no lines, a new line with the span is created.
     *
     * # Examples
     *
     * ```kotlin
     * val text = Text.from("Hello, world!")
     * text.pushSpan(Span.from("How are you?"))
     * text.pushSpan("How are you?")
     * ```
     */
    fun pushSpan(span: Span) {
        val last = lines.lastOrNull()
        if (last != null) {
            last.pushSpan(span)
        } else {
            lines.add(Line.from(span))
        }
    }

    /**
     * Adds a span from a String to the last line of the text.
     */
    fun pushSpan(content: String) {
        pushSpan(Span.raw(content))
    }

    /**
     * Extends this text with lines from the given iterable.
     */
    fun extend(iter: Iterable<Line>) {
        lines.addAll(iter)
    }

    /**
     * Extends this text with lines from the given text.
     */
    fun extend(other: Text) {
        lines.addAll(other.lines)
    }

    /**
     * Returns string representation of this text.
     */
    override fun toString(): String = buildString {
        lines.forEachIndexed { index, line ->
            if (index > 0) append('\n')
            append(line.toString())
        }
    }

    // Styled interface implementation
    override fun getStyle(): Style = style
    override fun setStyle(style: Style): Text = copy(style = style)

    // Widget interface implementation
    override fun render(area: Rect, buf: Buffer) {
        // Simple implementation: render each line at consecutive y positions
        val lineIterator = lines.iterator()
        var y = area.y.toInt()
        while (lineIterator.hasNext() && y < area.bottom().toInt()) {
            val line = lineIterator.next()
            // Apply text's style and alignment to the line
            val effectiveAlignment = line.alignment ?: alignment ?: Alignment.Left
            val lineWidth = line.width()
            val x = when (effectiveAlignment) {
                Alignment.Left -> area.x.toInt()
                Alignment.Center -> area.x.toInt() + ((area.width.toInt() - lineWidth) / 2).coerceAtLeast(0)
                Alignment.Right -> (area.right().toInt() - lineWidth).coerceAtLeast(area.x.toInt())
            }
            buf.setLine(x.toUShort(), y.toUShort(), line.style(style.patch(line.style)), area.width)
            y++
        }
    }

    companion object {
        /**
         * Creates a new [Text] with default values.
         */
        fun default(): Text = Text()

        /**
         * Creates a [Text] (potentially multiple lines) with no style.
         *
         * Any newlines in the content are converted to separate lines.
         *
         * # Examples
         *
         * ```kotlin
         * val text = Text.raw("The first line\nThe second line")
         * ```
         */
        fun raw(content: String): Text {
            val lines = content.split('\n').map { Line.from(it) }.toMutableList()
            return Text(lines = lines)
        }

        /**
         * Creates a [Text] (potentially multiple lines) with a style.
         *
         * Any newlines in the content are converted to separate lines.
         *
         * # Examples
         *
         * ```kotlin
         * val style = Style.new().yellow().italic()
         * val text = Text.styled("The first line\nThe second line", style)
         * ```
         */
        fun styled(content: String, style: Style): Text {
            val lines = content.split('\n').map { Line.from(it) }.toMutableList()
            return Text(style = style, lines = lines)
        }

        /**
         * Creates a [Text] from a [String].
         *
         * Any newlines in the content are converted to separate lines.
         */
        fun from(content: String): Text = raw(content)

        /**
         * Creates a [Text] from a [Span].
         */
        fun from(span: Span): Text = Text(lines = mutableListOf(Line.from(span)))

        /**
         * Creates a [Text] from a [Line].
         */
        fun from(line: Line): Text = Text(lines = mutableListOf(line))

        /**
         * Creates a [Text] from a list of [Line]s.
         */
        fun from(lines: List<Line>): Text = Text(lines = lines.toMutableList())

        /**
         * Creates a [Text] from an iterable of items that can be converted into [Line].
         */
        fun fromIter(iter: Iterable<Line>): Text = Text(lines = iter.toMutableList())
    }
}

/**
 * Adds a [Line] to this [Text], returning a new Text.
 */
operator fun Text.plus(line: Line): Text {
    val result = this.copy(lines = this.lines.toMutableList())
    result.pushLine(line)
    return result
}

/**
 * Adds two [Text]s together.
 *
 * This ignores the style and alignment of the second Text.
 */
operator fun Text.plus(text: Text): Text {
    val result = this.copy(lines = this.lines.toMutableList())
    result.extend(text)
    return result
}

/**
 * Extension function to convert any value to a [Text] using its [toString] representation.
 */
fun Any.toText(): Text = Text.raw(this.toString())

/**
 * Extension function to convert an Int to a [Text].
 */
fun Int.toText(): Text = Text.raw(this.toString())
