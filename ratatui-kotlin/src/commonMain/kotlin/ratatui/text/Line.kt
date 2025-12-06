package ratatui.text

import ratatui.buffer.Buffer
import ratatui.layout.Alignment
import ratatui.layout.Rect
import ratatui.style.Style
import ratatui.style.Styled
import ratatui.widgets.Widget

/**
 * A line of text, consisting of one or more [Span]s.
 *
 * [Line]s are used wherever text is displayed in the terminal and represent a single line of
 * text. When a [Line] is rendered, it is rendered as a single line of text, with each [Span]
 * being rendered in order (left to right).
 *
 * Any newlines in the content are removed when creating a [Line] using the constructor or
 * conversion methods.
 *
 * # Constructor Methods
 *
 * - [Line.default] creates a line with empty content and the default style.
 * - [Line.raw] creates a line with the given content and the default style.
 * - [Line.styled] creates a line with the given content and style.
 *
 * # Conversion Methods
 *
 * - [Line.from] creates a `Line` from a [String].
 * - [Line.fromIter] creates a line from an iterator of items that are convertible to [Span].
 *
 * # Setter Methods
 *
 * These methods are fluent setters. They return a `Line` with the property set.
 *
 * - [Line.spans] sets the content of the line.
 * - [Line.style] sets the style of the line.
 * - [Line.alignment] sets the alignment of the line.
 * - [Line.leftAligned] sets the alignment of the line to [Alignment.Left].
 * - [Line.centered] sets the alignment of the line to [Alignment.Center].
 * - [Line.rightAligned] sets the alignment of the line to [Alignment.Right].
 *
 * # Other Methods
 *
 * - [Line.patchStyle] patches the style of the line, adding modifiers from the given style.
 * - [Line.resetStyle] resets the style of the line.
 * - [Line.width] returns the unicode width of the content held by this line.
 * - [Line.styledGraphemes] returns a list of the graphemes held by this line.
 * - [Line.pushSpan] adds a span to the line.
 *
 * # Compatibility Notes
 *
 * Before v0.26.0, [Line] did not have a `style` field and instead relied on only the styles that
 * were set on each [Span] contained in the `spans` field. The [Line.patchStyle] method was
 * the only way to set the overall style for individual lines. For this reason, this field may not
 * be supported yet by all widgets (outside of the `ratatui` crate itself).
 *
 * # Examples
 *
 * ## Creating Lines
 * [Line]s can be created from [Span]s, [String]s. They can be styled with a [Style].
 *
 * ```kotlin
 * val style = Style.new().yellow()
 * val line = Line.raw("Hello, world!").style(style)
 * val line = Line.styled("Hello, world!", style)
 *
 * val line = Line.from("Hello, world!")
 * val line = Line.from(String("Hello, world!"))
 * val line = Line.from(listOf(
 *     Span.styled("Hello", Style.new().blue()),
 *     Span.raw(" world!"),
 * ))
 * ```
 *
 * ## Styling Lines
 *
 * The line's [Style] is used by the rendering widget to determine how to style the line. Each
 * [Span] in the line will be styled with the [Style] of the line, and then with its own
 * [Style]. If the line is longer than the available space, the style is applied to the entire
 * line, and the line is truncated. `Line` also implements [Styled] which means you can use the
 * methods of the `Stylize` trait.
 *
 * ```kotlin
 * val line = Line.from("Hello world!").style(Style.new().yellow().italic())
 * val line = Line.from("Hello world!").yellow().italic()
 * ```
 *
 * ## Aligning Lines
 *
 * The line's [Alignment] is used by the rendering widget to determine how to align the line
 * within the available space. If the line is longer than the available space, the alignment is
 * ignored and the line is truncated.
 *
 * ```kotlin
 * val line = Line.from("Hello world!").alignment(Alignment.Right)
 * val line = Line.from("Hello world!").centered()
 * val line = Line.from("Hello world!").leftAligned()
 * val line = Line.from("Hello world!").rightAligned()
 * ```
 *
 * ## Rendering Lines
 *
 * `Line` implements the [Widget] trait, which means it can be rendered to a [Buffer].
 *
 * ```kotlin
 * // in another widget's render method
 * val line = Line.from("Hello world!").style(Style.new().yellow().italic())
 * line.render(area, buf)
 * ```
 */
data class Line(
    /** The style of this line of text. */
    val style: Style = Style.default(),

    /** The alignment of this line of text. */
    val alignment: Alignment? = null,

    /** The spans that make up this line of text. */
    val spans: MutableList<Span> = mutableListOf()
) : Styled<Line>, Widget, Iterable<Span> {

    /**
     * Sets the spans of this line of text.
     *
     * `spans` accepts any iterable that yields items that are convertible to [Span] (e.g.
     * [String], [Span]).
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.default().spans(listOf("Hello".blue(), " world!".green()))
     * ```
     */
    fun spans(spans: List<Span>): Line = copy(spans = spans.toMutableList())

    /**
     * Sets the style of this line of text.
     *
     * Defaults to [Style.default].
     *
     * Note: This field was added in v0.26.0. Prior to that, the style of a line was determined
     * only by the style of each [Span] contained in the line. For this reason, this field may
     * not be supported by all widgets (outside of the `ratatui` crate itself).
     *
     * `style` accepts any type that is convertible to [Style].
     *
     * # Examples
     * ```kotlin
     * val line = Line.from("foo").style(Style.new().red())
     * ```
     */
    fun style(style: Style): Line = copy(style = style)

    /**
     * Sets the target alignment for this line of text.
     *
     * Defaults to `null`, meaning the alignment is determined by the rendering widget.
     * Setting the alignment of a Line generally overrides the alignment of its
     * parent Text or Widget.
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.from("Hi, what's up?")
     * assertEquals(null, line.alignment)
     * assertEquals(
     *     Alignment.Right,
     *     line.alignment(Alignment.Right).alignment
     * )
     * ```
     */
    fun alignment(alignment: Alignment): Line = copy(alignment = alignment)

    /**
     * Left-aligns this line of text.
     *
     * Convenience shortcut for `Line.alignment(Alignment.Left)`.
     * Setting the alignment of a Line generally overrides the alignment of its
     * parent Text or Widget, with the default alignment being inherited from the parent.
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.from("Hi, what's up?").leftAligned()
     * ```
     */
    fun leftAligned(): Line = alignment(Alignment.Left)

    /**
     * Center-aligns this line of text.
     *
     * Convenience shortcut for `Line.alignment(Alignment.Center)`.
     * Setting the alignment of a Line generally overrides the alignment of its
     * parent Text or Widget, with the default alignment being inherited from the parent.
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.from("Hi, what's up?").centered()
     * ```
     */
    fun centered(): Line = alignment(Alignment.Center)

    /**
     * Right-aligns this line of text.
     *
     * Convenience shortcut for `Line.alignment(Alignment.Right)`.
     * Setting the alignment of a Line generally overrides the alignment of its
     * parent Text or Widget, with the default alignment being inherited from the parent.
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.from("Hi, what's up?").rightAligned()
     * ```
     */
    fun rightAligned(): Line = alignment(Alignment.Right)

    /**
     * Returns the width of the underlying string.
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.from(listOf("Hello".blue(), " world!".green()))
     * assertEquals(12, line.width())
     * ```
     */
    fun width(): Int = spans.sumOf { it.width() }

    /**
     * Returns a list of graphemes held by this line.
     *
     * `baseStyle` is the [Style] that will be patched with each grapheme [Style] to get
     * the resulting [Style].
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.styled("Text", Style.default().fg(Color.Yellow))
     * val style = Style.default().fg(Color.Green).bg(Color.Black)
     * assertEquals(
     *     line.styledGraphemes(style),
     *     listOf(
     *         StyledGrapheme("T", Style.default().fg(Color.Yellow).bg(Color.Black)),
     *         StyledGrapheme("e", Style.default().fg(Color.Yellow).bg(Color.Black)),
     *         StyledGrapheme("x", Style.default().fg(Color.Yellow).bg(Color.Black)),
     *         StyledGrapheme("t", Style.default().fg(Color.Yellow).bg(Color.Black)),
     *     )
     * )
     * ```
     */
    fun styledGraphemes(baseStyle: Style = Style.default()): List<StyledGrapheme> {
        val patchedBaseStyle = baseStyle.patch(style)
        return spans.flatMap { span -> span.styledGraphemes(patchedBaseStyle) }
    }

    /**
     * Patches the style of this Line, adding modifiers from the given style.
     *
     * This is useful for when you want to apply a style to a line that already has some styling.
     * In contrast to [Line.style], this method will not overwrite the existing style, but
     * instead will add the given style's modifiers to this Line's style.
     *
     * This is a fluent setter method which must be chained or used as it returns a new Line.
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.styled("My text", Modifier.ITALIC)
     * val styledLine = Line.styled("My text", Style.new().fg(Color.Yellow).addModifier(Modifier.ITALIC))
     * assertEquals(styledLine, line.patchStyle(Color.Yellow))
     * ```
     */
    fun patchStyle(style: Style): Line = copy(style = this.style.patch(style))

    /**
     * Resets the style of this Line.
     *
     * Equivalent to calling `patchStyle(Style.reset())`.
     *
     * This is a fluent setter method which must be chained or used as it returns a new Line.
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.styled("My text", Style.default().yellow().onRed().italic())
     * assertEquals(Style.reset(), line.resetStyle().style)
     * ```
     */
    fun resetStyle(): Line = patchStyle(Style.reset())

    /**
     * Adds a span to the line.
     *
     * `span` can be any type that is convertible into a `Span`. For example, you can pass a
     * [String] or a [Span].
     *
     * # Examples
     *
     * ```kotlin
     * val line = Line.from("Hello, ")
     * line.pushSpan(Span.raw("world!"))
     * line.pushSpan(" How are you?")
     * ```
     */
    fun pushSpan(span: Span) {
        spans.add(span)
    }

    /**
     * Adds a span to the line from a string.
     */
    fun pushSpan(content: String) {
        spans.add(Span.raw(content))
    }

    // Iterable implementation
    override fun iterator(): Iterator<Span> = spans.iterator()

    // Styled implementation
    override fun getStyle(): Style = style

    override fun setStyle(style: Style): Line = style(style)

    // Widget implementation
    override fun render(area: Rect, buf: Buffer) {
        renderWithAlignment(area, buf, null)
    }

    /**
     * An internal implementation method for `Widget.render` that allows the parent widget to
     * define a default alignment, to be used if `Line.alignment` is `null`.
     */
    internal fun renderWithAlignment(
        area: Rect,
        buf: Buffer,
        parentAlignment: Alignment?
    ) {
        val clippedArea = area.intersection(buf.area)
        if (clippedArea.isEmpty()) {
            return
        }
        val renderArea = clippedArea.copy(height = 1u)
        val lineWidth = width()
        if (lineWidth == 0) {
            return
        }

        buf.setStyle(renderArea, style)

        val effectiveAlignment = alignment ?: parentAlignment

        val areaWidth = renderArea.width.toInt()
        val canRenderCompleteLine = lineWidth <= areaWidth
        if (canRenderCompleteLine) {
            val indentWidth = when (effectiveAlignment) {
                Alignment.Center -> (areaWidth - lineWidth) / 2
                Alignment.Right -> areaWidth - lineWidth
                Alignment.Left, null -> 0
            }
            val indentedArea = renderArea.indentX(indentWidth.toUShort())
            renderSpans(spans, indentedArea, buf, 0)
        } else {
            // There is not enough space to render the whole line. As the right side is truncated by
            // the area width, only truncate the left.
            val skipWidth = when (effectiveAlignment) {
                Alignment.Center -> (lineWidth - areaWidth) / 2
                Alignment.Right -> lineWidth - areaWidth
                Alignment.Left, null -> 0
            }
            renderSpans(spans, renderArea, buf, skipWidth)
        }
    }

    override fun toString(): String {
        return spans.joinToString("") { it.content }
    }

    /**
     * Operator + to combine a Line with a Span.
     */
    operator fun plus(span: Span): Line {
        val newSpans = spans.toMutableList()
        newSpans.add(span)
        return copy(spans = newSpans)
    }

    /**
     * Operator + to combine two Lines into a Text.
     */
    operator fun plus(other: Line): Text = Text.from(listOf(this, other))

    companion object {
        /**
         * Create a line with empty content and default style.
         */
        fun default(): Line = Line()

        /**
         * Create a line with the default style.
         *
         * `content` is a string. Any newlines are converted to separate spans.
         *
         * # Examples
         *
         * ```kotlin
         * Line.raw("test content")
         * ```
         */
        fun raw(content: String): Line {
            return Line(spans = contentToSpans(content))
        }

        /**
         * Create a line with the given style.
         *
         * `content` is a string. Any newlines are converted to separate spans.
         *
         * # Examples
         *
         * ```kotlin
         * val style = Style.new().yellow().italic()
         * Line.styled("My text", style)
         * ```
         */
        fun styled(content: String, style: Style): Line {
            return Line(style = style, spans = contentToSpans(content))
        }

        /**
         * Create a line from a string.
         */
        fun from(content: String): Line = raw(content)

        /**
         * Create a line from a single span.
         */
        fun from(span: Span): Line = Line(spans = mutableListOf(span))

        /**
         * Create a line from a list of spans.
         */
        fun from(spans: List<Span>): Line = Line(spans = spans.toMutableList())

        /**
         * Create a line from an iterator of spans.
         */
        fun fromIter(spans: Iterable<Span>): Line = Line(spans = spans.toMutableList())

        /**
         * Convert content string to a list of spans, splitting on newlines.
         */
        private fun contentToSpans(content: String): MutableList<Span> {
            return content.lines().map { Span.raw(it) }.toMutableList()
        }
    }
}

/**
 * Renders all the spans of the line that should be visible.
 */
private fun renderSpans(spans: List<Span>, area: Rect, buf: Buffer, spanSkipWidth: Int) {
    var currentArea = area
    var skipWidth = spanSkipWidth

    for (span in spans) {
        val spanWidth = span.width()

        // Ignore spans that are completely before the offset
        if (skipWidth >= spanWidth) {
            skipWidth -= spanWidth
            continue
        }

        // Apply the skip from the start of the span
        val availableWidth = spanWidth - skipWidth
        val offsetToApply = skipWidth
        skipWidth = 0 // ensure the next span is rendered in full

        if (currentArea.isEmpty()) {
            break
        }

        if (spanWidth <= availableWidth) {
            // Span is fully visible
            val offset = if (offsetToApply > 0) {
                // Truncate the start of the span
                val truncated = unicodeTruncateStart(span.content, availableWidth)
                val actualWidth = unicodeWidth(truncated)
                val firstGraphemeOffset = availableWidth - actualWidth
                Span.styled(truncated, span.style).render(currentArea.indentX(firstGraphemeOffset.toUShort()), buf)
                actualWidth
            } else {
                span.render(currentArea, buf)
                spanWidth
            }
            currentArea = currentArea.indentX(offset.coerceAtMost(UShort.MAX_VALUE.toInt()).toUShort())
        } else {
            // Span is only partially visible - truncate the start
            val truncated = unicodeTruncateStart(span.content, availableWidth)
            val actualWidth = unicodeWidth(truncated)
            val firstGraphemeOffset = availableWidth - actualWidth
            Span.styled(truncated, span.style).render(currentArea.indentX(firstGraphemeOffset.toUShort()), buf)
            currentArea = currentArea.indentX(actualWidth.coerceAtMost(UShort.MAX_VALUE.toInt()).toUShort())
        }
    }
}

/**
 * Truncate a string from the start to fit within the given width.
 * Returns the truncated string.
 */
private fun unicodeTruncateStart(s: String, maxWidth: Int): String {
    if (maxWidth <= 0) return ""
    val graphemes = graphemes(s)
    var totalWidth = graphemes.sumOf { unicodeWidth(it) }

    var startIndex = 0
    while (totalWidth > maxWidth && startIndex < graphemes.size) {
        totalWidth -= unicodeWidth(graphemes[startIndex])
        startIndex++
    }

    return graphemes.subList(startIndex, graphemes.size).joinToString("")
}

/**
 * A trait for converting a value to a [Line].
 *
 * This trait is automatically implemented for any type that implements the `toString` method.
 */
interface ToLine {
    /**
     * Converts the value to a [Line].
     */
    fun toLine(): Line
}

/**
 * Extension function to convert any type to a Line.
 */
fun Any.toLine(): Line = Line.from(this.toString())

// Extension function to convert a String to a Line
fun String.toLine(): Line = Line.from(this)
