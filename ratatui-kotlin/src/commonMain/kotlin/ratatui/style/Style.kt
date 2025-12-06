/**
 * `style` contains the primitives used to control how your user interface will look.
 *
 * There are two ways to set styles:
 * - Creating and using the [Style] class. (e.g. `Style.new().fg(Color.Red)`).
 * - Using style shorthands. (e.g. `"hello".red()`).
 *
 * ## Using the Style class
 *
 * This is the original approach to styling and likely the most common. This is useful when
 * creating style variables to reuse, however the shorthands are often more convenient and
 * readable for most use cases.
 *
 * ### Example
 *
 * ```kotlin
 * val headingStyle = Style.new()
 *     .fg(Color.Black)
 *     .bg(Color.Green)
 *     .addModifier(Modifier.ITALIC or Modifier.BOLD)
 * val span = Span.styled("hello", headingStyle)
 * ```
 *
 * ## Using style shorthands
 *
 * Originally Ratatui only had the ability to set styles using the `Style` struct. This is still
 * supported, but there are now shorthands for all the styles that can be set. These save you from
 * having to create a `Style` struct every time you want to set a style.
 *
 * The shorthands are implemented in the [Stylize] interface which is automatically implemented for
 * many types via the [Styled] interface. This means that you can use the shorthands on any type
 * that implements [Styled]. E.g.:
 * - Strings when styled return a [Span]
 * - [Span]s can be styled again, which will merge the styles.
 * - Many widget types can be styled directly rather than calling their `style()` method.
 *
 * See the [Stylize] and [Styled] interfaces for more information.
 *
 * ### Example
 *
 * ```kotlin
 * assertEquals(
 *     "hello".red().onBlue().bold(),
 *     Span.styled(
 *         "hello",
 *         Style.default()
 *             .fg(Color.Red)
 *             .bg(Color.Blue)
 *             .addModifier(Modifier.BOLD)
 *     )
 * )
 *
 * assertEquals(
 *     Text.from("hello").red().onBlue().bold(),
 *     Text.from("hello").style(
 *         Style.default()
 *             .fg(Color.Red)
 *             .bg(Color.Blue)
 *             .addModifier(Modifier.BOLD)
 *     )
 * )
 * ```
 *
 * [Span]: ratatui.text.Span
 */
package ratatui.style

/**
 * Modifier changes the way a piece of text is displayed.
 *
 * They are bitflags so they can easily be composed using [or] (bitwise OR).
 *
 * [Style.Companion.from] with Modifier is implemented so you can use `Modifier` anywhere that accepts
 * `Style`.
 *
 * ## Examples
 *
 * ```kotlin
 * val m = Modifier.BOLD or Modifier.ITALIC
 * ```
 */
/**
 * Value class wrapper for modifier bit flags.
 * Note: In Kotlin/Native, value classes don't have the same inline optimization as JVM,
 * but they still provide type safety and a clean API.
 */
value class Modifier(val bits: UShort) {

    /** Check if the modifier is empty (no flags set) */
    fun isEmpty(): Boolean = bits == 0.toUShort()

    /** Check if the modifier contains all flags */
    fun isAll(): Boolean = bits == ALL_BITS

    /** Check if this modifier contains the given modifier */
    fun contains(other: Modifier): Boolean = (bits and other.bits) == other.bits

    /** Union of two modifiers (bitwise OR) */
    infix fun or(other: Modifier): Modifier = Modifier((bits.toInt() or other.bits.toInt()).toUShort())

    /** Alias for [or] */
    fun union(other: Modifier): Modifier = this or other

    /** Intersection of two modifiers (bitwise AND) */
    infix fun and(other: Modifier): Modifier = Modifier((bits.toInt() and other.bits.toInt()).toUShort())

    /** Alias for [and] */
    fun intersection(other: Modifier): Modifier = this and other

    /** Difference of two modifiers (this AND NOT other) */
    fun difference(other: Modifier): Modifier = Modifier((bits.toInt() and other.bits.toInt().inv()).toUShort())

    /** Insert flags from another modifier */
    fun insert(other: Modifier): Modifier = this or other

    /** Remove flags from another modifier */
    fun remove(other: Modifier): Modifier = difference(other)

    /** Iterate over all set flags */
    fun iter(): List<Modifier> = ALL_FLAGS.filter { contains(it) }

    /**
     * Format the modifier as `NONE` if the modifier is empty or as a list of flags separated by
     * `|` otherwise.
     */
    override fun toString(): String {
        if (isEmpty()) return "NONE"
        return iter().joinToString(" | ") { flag ->
            when (flag) {
                BOLD -> "BOLD"
                DIM -> "DIM"
                ITALIC -> "ITALIC"
                UNDERLINED -> "UNDERLINED"
                SLOW_BLINK -> "SLOW_BLINK"
                RAPID_BLINK -> "RAPID_BLINK"
                REVERSED -> "REVERSED"
                HIDDEN -> "HIDDEN"
                CROSSED_OUT -> "CROSSED_OUT"
                else -> "0x${flag.bits.toString(16)}"
            }
        }
    }

    companion object {
        /** Bold text modifier */
        val BOLD = Modifier(0b0000_0000_0001u)
        /** Dim/faint text modifier */
        val DIM = Modifier(0b0000_0000_0010u)
        /** Italic text modifier */
        val ITALIC = Modifier(0b0000_0000_0100u)
        /** Underlined text modifier */
        val UNDERLINED = Modifier(0b0000_0000_1000u)
        /** Slow blink text modifier */
        val SLOW_BLINK = Modifier(0b0000_0001_0000u)
        /** Rapid blink text modifier */
        val RAPID_BLINK = Modifier(0b0000_0010_0000u)
        /** Reversed (inverse) text modifier */
        val REVERSED = Modifier(0b0000_0100_0000u)
        /** Hidden text modifier */
        val HIDDEN = Modifier(0b0000_1000_0000u)
        /** Crossed out (strikethrough) text modifier */
        val CROSSED_OUT = Modifier(0b0001_0000_0000u)

        private const val ALL_BITS: UShort = 0b0001_1111_1111u

        /** All flags set */
        fun all(): Modifier = Modifier(ALL_BITS)

        /** No flags set (empty modifier) */
        fun empty(): Modifier = Modifier(0u)

        /** Default modifier (empty) */
        fun default(): Modifier = empty()

        /** List of all individual modifier flags */
        private val ALL_FLAGS = listOf(
            BOLD, DIM, ITALIC, UNDERLINED, SLOW_BLINK,
            RAPID_BLINK, REVERSED, HIDDEN, CROSSED_OUT
        )
    }
}

/**
 * Style lets you control the main characteristics of the displayed elements.
 *
 * ```kotlin
 * Style.default()
 *     .fg(Color.Black)
 *     .bg(Color.Green)
 *     .addModifier(Modifier.ITALIC or Modifier.BOLD)
 * ```
 *
 * Styles can also be created with a shorthand notation.
 *
 * ```kotlin
 * Style.new().black().onGreen().italic().bold()
 * ```
 *
 * For more information about the style shorthands, see the [Stylize] interface.
 *
 * We implement factory functions from [Color] and [Modifier] to [Style] so you can use them
 * anywhere that accepts `Style`.
 *
 * ```kotlin
 * Line.styled("hello", Style.new().fg(Color.Red))
 * // simplifies to
 * Line.styled("hello", Style.from(Color.Red))
 *
 * Line.styled("hello", Style.new().addModifier(Modifier.BOLD))
 * // simplifies to
 * Line.styled("hello", Style.from(Modifier.BOLD))
 * ```
 *
 * Styles represents an incremental change. If you apply the styles S1, S2, S3 to a cell of the
 * terminal buffer, the style of this cell will be the result of the merge of S1, S2 and S3, not
 * just S3.
 *
 * ```kotlin
 * val styles = listOf(
 *     Style.default()
 *         .fg(Color.Blue)
 *         .addModifier(Modifier.BOLD or Modifier.ITALIC),
 *     Style.default()
 *         .bg(Color.Red)
 *         .addModifier(Modifier.UNDERLINED),
 *     Style.default()
 *         .fg(Color.Yellow)
 *         .removeModifier(Modifier.ITALIC),
 * )
 * val buffer = Buffer.empty(Rect(0, 0, 1, 1))
 * for (style in styles) {
 *     buffer[0, 0].setStyle(style)
 * }
 * assertEquals(
 *     Style(
 *         fg = Color.Yellow,
 *         bg = Color.Red,
 *         addModifier = Modifier.BOLD or Modifier.UNDERLINED,
 *         subModifier = Modifier.empty(),
 *     ),
 *     buffer[0, 0].style(),
 * )
 * ```
 *
 * The default implementation returns a `Style` that does not modify anything. If you wish to
 * reset all properties until that point use [Style.reset].
 *
 * ```kotlin
 * val styles = listOf(
 *     Style.default()
 *         .fg(Color.Blue)
 *         .addModifier(Modifier.BOLD or Modifier.ITALIC),
 *     Style.reset().fg(Color.Yellow),
 * )
 * val buffer = Buffer.empty(Rect(0, 0, 1, 1))
 * for (style in styles) {
 *     buffer[0, 0].setStyle(style)
 * }
 * assertEquals(
 *     Style(
 *         fg = Color.Yellow,
 *         bg = Color.Reset,
 *         addModifier = Modifier.empty(),
 *         subModifier = Modifier.empty(),
 *     ),
 *     buffer[0, 0].style(),
 * )
 * ```
 */
data class Style(
    /** The foreground color. */
    val fg: Color? = null,
    /** The background color. */
    val bg: Color? = null,
    /** The underline color (requires underline modifier to be visible). */
    val underlineColor: Color? = null,
    /** The modifiers to add. */
    val addModifier: Modifier = Modifier.empty(),
    /** The modifiers to remove. */
    val subModifier: Modifier = Modifier.empty()
) {

    /**
     * Returns a copy with the foreground color changed.
     *
     * ## Examples
     *
     * ```kotlin
     * val style = Style.default().fg(Color.Blue)
     * val diff = Style.default().fg(Color.Red)
     * assertEquals(style.patch(diff), Style.default().fg(Color.Red))
     * ```
     */
    fun fg(color: Color): Style = copy(fg = color)

    /**
     * Returns a copy with the background color changed.
     *
     * ## Examples
     *
     * ```kotlin
     * val style = Style.default().bg(Color.Blue)
     * val diff = Style.default().bg(Color.Red)
     * assertEquals(style.patch(diff), Style.default().bg(Color.Red))
     * ```
     */
    fun bg(color: Color): Style = copy(bg = color)

    /**
     * Changes the underline color. The text must be underlined with a modifier for this to work.
     *
     * This uses a non-standard ANSI escape sequence. It is supported by most terminal emulators,
     * but is only implemented in the crossterm backend.
     *
     * See [Wikipedia](https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_(Select_Graphic_Rendition)_parameters)
     * code `58` and `59` for more information.
     *
     * ## Examples
     *
     * ```kotlin
     * val style = Style.default()
     *     .underlineColor(Color.Blue)
     *     .addModifier(Modifier.UNDERLINED)
     * val diff = Style.default()
     *     .underlineColor(Color.Red)
     *     .addModifier(Modifier.UNDERLINED)
     * assertEquals(
     *     style.patch(diff),
     *     Style.default()
     *         .underlineColor(Color.Red)
     *         .addModifier(Modifier.UNDERLINED)
     * )
     * ```
     */
    fun underlineColor(color: Color): Style = copy(underlineColor = color)

    /**
     * Changes the text emphasis.
     *
     * When applied, it adds the given modifier to the `Style` modifiers.
     *
     * ## Examples
     *
     * ```kotlin
     * val style = Style.default().addModifier(Modifier.BOLD)
     * val diff = Style.default().addModifier(Modifier.ITALIC)
     * val patched = style.patch(diff)
     * assertEquals(patched.addModifier, Modifier.BOLD or Modifier.ITALIC)
     * assertEquals(patched.subModifier, Modifier.empty())
     * ```
     */
    fun addModifier(modifier: Modifier): Style = copy(
        subModifier = subModifier.difference(modifier),
        addModifier = addModifier.union(modifier)
    )

    /**
     * Changes the text emphasis.
     *
     * When applied, it removes the given modifier from the `Style` modifiers.
     *
     * ## Examples
     *
     * ```kotlin
     * val style = Style.default().addModifier(Modifier.BOLD or Modifier.ITALIC)
     * val diff = Style.default().removeModifier(Modifier.ITALIC)
     * val patched = style.patch(diff)
     * assertEquals(patched.addModifier, Modifier.BOLD)
     * assertEquals(patched.subModifier, Modifier.ITALIC)
     * ```
     */
    fun removeModifier(modifier: Modifier): Style = copy(
        addModifier = addModifier.difference(modifier),
        subModifier = subModifier.union(modifier)
    )

    /**
     * Results in a combined style that is equivalent to applying the two individual styles to
     * a style one after the other.
     *
     * ## Examples
     * ```kotlin
     * val style1 = Style.default().fg(Color.Yellow)
     * val style2 = Style.default().bg(Color.Red)
     * val combined = style1.patch(style2)
     * assertEquals(
     *     Style.default().patch(style1).patch(style2),
     *     Style.default().patch(combined)
     * )
     * ```
     */
    fun patch(other: Style): Style {
        var newAddModifier = addModifier.remove(other.subModifier)
        newAddModifier = newAddModifier.insert(other.addModifier)
        var newSubModifier = subModifier.remove(other.addModifier)
        newSubModifier = newSubModifier.insert(other.subModifier)

        return Style(
            fg = other.fg ?: fg,
            bg = other.bg ?: bg,
            underlineColor = other.underlineColor ?: underlineColor,
            addModifier = newAddModifier,
            subModifier = newSubModifier
        )
    }

    // -------------------------------------------------------------------------
    // Color shorthand methods (foreground)
    // -------------------------------------------------------------------------

    /** Set foreground to black */
    fun black(): Style = fg(Color.Black)
    /** Set foreground to red */
    fun red(): Style = fg(Color.Red)
    /** Set foreground to green */
    fun green(): Style = fg(Color.Green)
    /** Set foreground to yellow */
    fun yellow(): Style = fg(Color.Yellow)
    /** Set foreground to blue */
    fun blue(): Style = fg(Color.Blue)
    /** Set foreground to magenta */
    fun magenta(): Style = fg(Color.Magenta)
    /** Set foreground to cyan */
    fun cyan(): Style = fg(Color.Cyan)
    /** Set foreground to gray */
    fun gray(): Style = fg(Color.Gray)
    /** Set foreground to dark gray */
    fun darkGray(): Style = fg(Color.DarkGray)
    /** Set foreground to light red */
    fun lightRed(): Style = fg(Color.LightRed)
    /** Set foreground to light green */
    fun lightGreen(): Style = fg(Color.LightGreen)
    /** Set foreground to light yellow */
    fun lightYellow(): Style = fg(Color.LightYellow)
    /** Set foreground to light blue */
    fun lightBlue(): Style = fg(Color.LightBlue)
    /** Set foreground to light magenta */
    fun lightMagenta(): Style = fg(Color.LightMagenta)
    /** Set foreground to light cyan */
    fun lightCyan(): Style = fg(Color.LightCyan)
    /** Set foreground to white */
    fun white(): Style = fg(Color.White)

    // -------------------------------------------------------------------------
    // Color shorthand methods (background)
    // -------------------------------------------------------------------------

    /** Set background to black */
    fun onBlack(): Style = bg(Color.Black)
    /** Set background to red */
    fun onRed(): Style = bg(Color.Red)
    /** Set background to green */
    fun onGreen(): Style = bg(Color.Green)
    /** Set background to yellow */
    fun onYellow(): Style = bg(Color.Yellow)
    /** Set background to blue */
    fun onBlue(): Style = bg(Color.Blue)
    /** Set background to magenta */
    fun onMagenta(): Style = bg(Color.Magenta)
    /** Set background to cyan */
    fun onCyan(): Style = bg(Color.Cyan)
    /** Set background to gray */
    fun onGray(): Style = bg(Color.Gray)
    /** Set background to dark gray */
    fun onDarkGray(): Style = bg(Color.DarkGray)
    /** Set background to light red */
    fun onLightRed(): Style = bg(Color.LightRed)
    /** Set background to light green */
    fun onLightGreen(): Style = bg(Color.LightGreen)
    /** Set background to light yellow */
    fun onLightYellow(): Style = bg(Color.LightYellow)
    /** Set background to light blue */
    fun onLightBlue(): Style = bg(Color.LightBlue)
    /** Set background to light magenta */
    fun onLightMagenta(): Style = bg(Color.LightMagenta)
    /** Set background to light cyan */
    fun onLightCyan(): Style = bg(Color.LightCyan)
    /** Set background to white */
    fun onWhite(): Style = bg(Color.White)

    // -------------------------------------------------------------------------
    // Modifier shorthand methods (add)
    // -------------------------------------------------------------------------

    /** Add bold modifier */
    fun bold(): Style = addModifier(Modifier.BOLD)
    /** Add dim modifier */
    fun dim(): Style = addModifier(Modifier.DIM)
    /** Add italic modifier */
    fun italic(): Style = addModifier(Modifier.ITALIC)
    /** Add underlined modifier */
    fun underlined(): Style = addModifier(Modifier.UNDERLINED)
    /** Add slow blink modifier */
    fun slowBlink(): Style = addModifier(Modifier.SLOW_BLINK)
    /** Add rapid blink modifier */
    fun rapidBlink(): Style = addModifier(Modifier.RAPID_BLINK)
    /** Add reversed modifier */
    fun reversed(): Style = addModifier(Modifier.REVERSED)
    /** Add hidden modifier */
    fun hidden(): Style = addModifier(Modifier.HIDDEN)
    /** Add crossed out modifier */
    fun crossedOut(): Style = addModifier(Modifier.CROSSED_OUT)

    // -------------------------------------------------------------------------
    // Modifier shorthand methods (remove)
    // -------------------------------------------------------------------------

    /** Remove bold modifier */
    fun notBold(): Style = removeModifier(Modifier.BOLD)
    /** Remove dim modifier */
    fun notDim(): Style = removeModifier(Modifier.DIM)
    /** Remove italic modifier */
    fun notItalic(): Style = removeModifier(Modifier.ITALIC)
    /** Remove underlined modifier */
    fun notUnderlined(): Style = removeModifier(Modifier.UNDERLINED)
    /** Remove slow blink modifier */
    fun notSlowBlink(): Style = removeModifier(Modifier.SLOW_BLINK)
    /** Remove rapid blink modifier */
    fun notRapidBlink(): Style = removeModifier(Modifier.RAPID_BLINK)
    /** Remove reversed modifier */
    fun notReversed(): Style = removeModifier(Modifier.REVERSED)
    /** Remove hidden modifier */
    fun notHidden(): Style = removeModifier(Modifier.HIDDEN)
    /** Remove crossed out modifier */
    fun notCrossedOut(): Style = removeModifier(Modifier.CROSSED_OUT)

    /**
     * Formats the style in a way that can be copy-pasted into code using the style shorthands.
     *
     * This is useful for debugging and for generating code snippets.
     */
    override fun toString(): String {
        val parts = mutableListOf("Style.new()")
        fg?.let { parts.add(".${colorToFgMethod(it)}") }
        bg?.let { parts.add(".${colorToBgMethod(it)}") }
        underlineColor?.let { parts.add(".underlineColor($it)") }
        for (modifier in addModifier.iter()) {
            parts.add(".${modifierToAddMethod(modifier)}")
        }
        for (modifier in subModifier.iter()) {
            parts.add(".${modifierToRemoveMethod(modifier)}")
        }
        return parts.joinToString("")
    }

    companion object {
        /** Returns a `Style` with default properties (no modifications). */
        fun new(): Style = Style()

        /** Returns a `Style` with default properties (no modifications). */
        fun default(): Style = Style()

        /**
         * Returns a `Style` resetting all properties.
         *
         * When applied to a cell, this will reset all style properties to their defaults.
         */
        fun reset(): Style = Style(
            fg = Color.Reset,
            bg = Color.Reset,
            underlineColor = Color.Reset,
            addModifier = Modifier.empty(),
            subModifier = Modifier.all()
        )

        // -------------------------------------------------------------------------
        // Factory methods from various types
        // -------------------------------------------------------------------------

        /**
         * Creates a new `Style` with the given foreground color.
         *
         * To specify a foreground and background color, use [from] with a Pair.
         *
         * ## Example
         *
         * ```kotlin
         * val style = Style.from(Color.Red)
         * ```
         */
        fun from(color: Color): Style = new().fg(color)

        /**
         * Creates a new `Style` with the given foreground and background colors.
         *
         * ## Example
         *
         * ```kotlin
         * // red foreground, blue background
         * val style = Style.from(Color.Red, Color.Blue)
         * // default foreground, blue background
         * val style = Style.from(Color.Reset, Color.Blue)
         * ```
         */
        fun from(fg: Color, bg: Color): Style = new().fg(fg).bg(bg)

        /**
         * Creates a new `Style` with the given modifier added.
         *
         * To specify multiple modifiers, use the `or` operator.
         *
         * ## Example
         *
         * ```kotlin
         * // add bold and italic
         * val style = Style.from(Modifier.BOLD or Modifier.ITALIC)
         * ```
         */
        fun from(modifier: Modifier): Style = new().addModifier(modifier)

        /**
         * Creates a new `Style` with the given modifiers added and removed.
         *
         * ## Example
         *
         * ```kotlin
         * // add bold and italic, remove dim
         * val style = Style.from(
         *     addModifier = Modifier.BOLD or Modifier.ITALIC,
         *     subModifier = Modifier.DIM
         * )
         * ```
         */
        fun from(addModifier: Modifier, subModifier: Modifier): Style =
            new().addModifier(addModifier).removeModifier(subModifier)

        /**
         * Creates a new `Style` with the given foreground color and modifier added.
         *
         * ## Example
         *
         * ```kotlin
         * // red foreground, add bold and italic
         * val style = Style.from(Color.Red, Modifier.BOLD or Modifier.ITALIC)
         * ```
         */
        fun from(fg: Color, modifier: Modifier): Style = new().fg(fg).addModifier(modifier)

        /**
         * Creates a new `Style` with the given foreground and background colors and modifier added.
         *
         * ## Example
         *
         * ```kotlin
         * // red foreground, blue background, add bold and italic
         * val style = Style.from(Color.Red, Color.Blue, Modifier.BOLD or Modifier.ITALIC)
         * ```
         */
        fun from(fg: Color, bg: Color, modifier: Modifier): Style =
            new().fg(fg).bg(bg).addModifier(modifier)

        /**
         * Creates a new `Style` with the given foreground and background colors and modifiers
         * added and removed.
         *
         * ## Example
         *
         * ```kotlin
         * // red foreground, blue background, add bold and italic, remove dim
         * val style = Style.from(
         *     fg = Color.Red,
         *     bg = Color.Blue,
         *     addModifier = Modifier.BOLD or Modifier.ITALIC,
         *     subModifier = Modifier.DIM
         * )
         * ```
         */
        fun from(fg: Color, bg: Color, addModifier: Modifier, subModifier: Modifier): Style =
            new().fg(fg).bg(bg).addModifier(addModifier).removeModifier(subModifier)
    }
}

// =============================================================================
// Helper functions for toString formatting
// =============================================================================

private fun colorToFgMethod(color: Color): String = when (color) {
    is Color.Black -> "black()"
    is Color.Red -> "red()"
    is Color.Green -> "green()"
    is Color.Yellow -> "yellow()"
    is Color.Blue -> "blue()"
    is Color.Magenta -> "magenta()"
    is Color.Cyan -> "cyan()"
    is Color.Gray -> "gray()"
    is Color.DarkGray -> "darkGray()"
    is Color.LightRed -> "lightRed()"
    is Color.LightGreen -> "lightGreen()"
    is Color.LightYellow -> "lightYellow()"
    is Color.LightBlue -> "lightBlue()"
    is Color.LightMagenta -> "lightMagenta()"
    is Color.LightCyan -> "lightCyan()"
    is Color.White -> "white()"
    is Color.Reset -> "fg(Color.Reset)"
    is Color.Rgb -> "fg(Color.Rgb(${color.r}u, ${color.g}u, ${color.b}u))"
    is Color.Indexed -> "fg(Color.Indexed(${color.index}u))"
}

private fun colorToBgMethod(color: Color): String = when (color) {
    is Color.Black -> "onBlack()"
    is Color.Red -> "onRed()"
    is Color.Green -> "onGreen()"
    is Color.Yellow -> "onYellow()"
    is Color.Blue -> "onBlue()"
    is Color.Magenta -> "onMagenta()"
    is Color.Cyan -> "onCyan()"
    is Color.Gray -> "onGray()"
    is Color.DarkGray -> "onDarkGray()"
    is Color.LightRed -> "onLightRed()"
    is Color.LightGreen -> "onLightGreen()"
    is Color.LightYellow -> "onLightYellow()"
    is Color.LightBlue -> "onLightBlue()"
    is Color.LightMagenta -> "onLightMagenta()"
    is Color.LightCyan -> "onLightCyan()"
    is Color.White -> "onWhite()"
    is Color.Reset -> "bg(Color.Reset)"
    is Color.Rgb -> "bg(Color.Rgb(${color.r}u, ${color.g}u, ${color.b}u))"
    is Color.Indexed -> "bg(Color.Indexed(${color.index}u))"
}

private fun modifierToAddMethod(modifier: Modifier): String = when (modifier) {
    Modifier.BOLD -> "bold()"
    Modifier.DIM -> "dim()"
    Modifier.ITALIC -> "italic()"
    Modifier.UNDERLINED -> "underlined()"
    Modifier.SLOW_BLINK -> "slowBlink()"
    Modifier.RAPID_BLINK -> "rapidBlink()"
    Modifier.REVERSED -> "reversed()"
    Modifier.HIDDEN -> "hidden()"
    Modifier.CROSSED_OUT -> "crossedOut()"
    else -> "addModifier($modifier)"
}

private fun modifierToRemoveMethod(modifier: Modifier): String = when (modifier) {
    Modifier.BOLD -> "notBold()"
    Modifier.DIM -> "notDim()"
    Modifier.ITALIC -> "notItalic()"
    Modifier.UNDERLINED -> "notUnderlined()"
    Modifier.SLOW_BLINK -> "notSlowBlink()"
    Modifier.RAPID_BLINK -> "notRapidBlink()"
    Modifier.REVERSED -> "notReversed()"
    Modifier.HIDDEN -> "notHidden()"
    Modifier.CROSSED_OUT -> "notCrossedOut()"
    else -> "removeModifier($modifier)"
}

// =============================================================================
// Tests
// =============================================================================

/**
 * Unit tests for Style and Modifier.
 *
 * In Kotlin, tests would typically be in a separate test source set.
 * These are included here as reference implementations matching the Rust tests.
 */
internal object StyleTests {

    // -------------------------------------------------------------------------
    // toString (debug) tests
    // -------------------------------------------------------------------------

    fun testDebug() {
        check(Style.new().toString() == "Style.new()")
        check(Style.default().toString() == "Style.new()")
        check(Style.new().red().toString() == "Style.new().red()")
        check(Style.new().onBlue().toString() == "Style.new().onBlue()")
        check(Style.new().bold().toString() == "Style.new().bold()")
        check(Style.new().notItalic().toString() == "Style.new().notItalic()")
        check(
            Style.new().red().onBlue().bold().italic().notDim().notHidden().toString() ==
            "Style.new().red().onBlue().bold().italic().notDim().notHidden()"
        )
    }

    // -------------------------------------------------------------------------
    // Patch combination tests
    // -------------------------------------------------------------------------

    fun testCombinedPatchGivesSameResultAsIndividualPatch() {
        val styles = listOf(
            Style.new(),
            Style.new().fg(Color.Yellow),
            Style.new().bg(Color.Yellow),
            Style.new().addModifier(Modifier.BOLD),
            Style.new().removeModifier(Modifier.BOLD),
            Style.new().addModifier(Modifier.ITALIC),
            Style.new().removeModifier(Modifier.ITALIC),
            Style.new().addModifier(Modifier.ITALIC or Modifier.BOLD),
            Style.new().removeModifier(Modifier.ITALIC or Modifier.BOLD),
        )
        for (a in styles) {
            for (b in styles) {
                for (c in styles) {
                    for (d in styles) {
                        check(
                            Style.new().patch(a).patch(b).patch(c).patch(d) ==
                            Style.new().patch(a.patch(b.patch(c.patch(d))))
                        )
                    }
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // Modifier tests
    // -------------------------------------------------------------------------

    fun testModifierDebug() {
        check(Modifier.empty().toString() == "NONE")
        check(Modifier.BOLD.toString() == "BOLD")
        check(Modifier.DIM.toString() == "DIM")
        check(Modifier.ITALIC.toString() == "ITALIC")
        check(Modifier.UNDERLINED.toString() == "UNDERLINED")
        check(Modifier.SLOW_BLINK.toString() == "SLOW_BLINK")
        check(Modifier.RAPID_BLINK.toString() == "RAPID_BLINK")
        check(Modifier.REVERSED.toString() == "REVERSED")
        check(Modifier.HIDDEN.toString() == "HIDDEN")
        check(Modifier.CROSSED_OUT.toString() == "CROSSED_OUT")
        check((Modifier.BOLD or Modifier.DIM).toString() == "BOLD | DIM")
        check(Modifier.all().toString() ==
            "BOLD | DIM | ITALIC | UNDERLINED | SLOW_BLINK | RAPID_BLINK | REVERSED | HIDDEN | CROSSED_OUT")
    }

    // -------------------------------------------------------------------------
    // Foreground color shorthand tests
    // -------------------------------------------------------------------------

    fun testFgCanBeStylized() {
        check(Style.new().black() == Style.new().fg(Color.Black))
        check(Style.new().red() == Style.new().fg(Color.Red))
        check(Style.new().green() == Style.new().fg(Color.Green))
        check(Style.new().yellow() == Style.new().fg(Color.Yellow))
        check(Style.new().blue() == Style.new().fg(Color.Blue))
        check(Style.new().magenta() == Style.new().fg(Color.Magenta))
        check(Style.new().cyan() == Style.new().fg(Color.Cyan))
        check(Style.new().white() == Style.new().fg(Color.White))
        check(Style.new().gray() == Style.new().fg(Color.Gray))
        check(Style.new().darkGray() == Style.new().fg(Color.DarkGray))
        check(Style.new().lightRed() == Style.new().fg(Color.LightRed))
        check(Style.new().lightGreen() == Style.new().fg(Color.LightGreen))
        check(Style.new().lightYellow() == Style.new().fg(Color.LightYellow))
        check(Style.new().lightBlue() == Style.new().fg(Color.LightBlue))
        check(Style.new().lightMagenta() == Style.new().fg(Color.LightMagenta))
        check(Style.new().lightCyan() == Style.new().fg(Color.LightCyan))
    }

    // -------------------------------------------------------------------------
    // Background color shorthand tests
    // -------------------------------------------------------------------------

    fun testBgCanBeStylized() {
        check(Style.new().onBlack() == Style.new().bg(Color.Black))
        check(Style.new().onRed() == Style.new().bg(Color.Red))
        check(Style.new().onGreen() == Style.new().bg(Color.Green))
        check(Style.new().onYellow() == Style.new().bg(Color.Yellow))
        check(Style.new().onBlue() == Style.new().bg(Color.Blue))
        check(Style.new().onMagenta() == Style.new().bg(Color.Magenta))
        check(Style.new().onCyan() == Style.new().bg(Color.Cyan))
        check(Style.new().onWhite() == Style.new().bg(Color.White))
        check(Style.new().onGray() == Style.new().bg(Color.Gray))
        check(Style.new().onDarkGray() == Style.new().bg(Color.DarkGray))
        check(Style.new().onLightRed() == Style.new().bg(Color.LightRed))
        check(Style.new().onLightGreen() == Style.new().bg(Color.LightGreen))
        check(Style.new().onLightYellow() == Style.new().bg(Color.LightYellow))
        check(Style.new().onLightBlue() == Style.new().bg(Color.LightBlue))
        check(Style.new().onLightMagenta() == Style.new().bg(Color.LightMagenta))
        check(Style.new().onLightCyan() == Style.new().bg(Color.LightCyan))
    }

    // -------------------------------------------------------------------------
    // Add modifier shorthand tests
    // -------------------------------------------------------------------------

    fun testAddModifierCanBeStylized() {
        check(Style.new().bold() == Style.new().addModifier(Modifier.BOLD))
        check(Style.new().dim() == Style.new().addModifier(Modifier.DIM))
        check(Style.new().italic() == Style.new().addModifier(Modifier.ITALIC))
        check(Style.new().underlined() == Style.new().addModifier(Modifier.UNDERLINED))
        check(Style.new().slowBlink() == Style.new().addModifier(Modifier.SLOW_BLINK))
        check(Style.new().rapidBlink() == Style.new().addModifier(Modifier.RAPID_BLINK))
        check(Style.new().reversed() == Style.new().addModifier(Modifier.REVERSED))
        check(Style.new().hidden() == Style.new().addModifier(Modifier.HIDDEN))
        check(Style.new().crossedOut() == Style.new().addModifier(Modifier.CROSSED_OUT))
    }

    // -------------------------------------------------------------------------
    // Remove modifier shorthand tests
    // -------------------------------------------------------------------------

    fun testRemoveModifierCanBeStylized() {
        check(Style.new().notBold() == Style.new().removeModifier(Modifier.BOLD))
        check(Style.new().notDim() == Style.new().removeModifier(Modifier.DIM))
        check(Style.new().notItalic() == Style.new().removeModifier(Modifier.ITALIC))
        check(Style.new().notUnderlined() == Style.new().removeModifier(Modifier.UNDERLINED))
        check(Style.new().notSlowBlink() == Style.new().removeModifier(Modifier.SLOW_BLINK))
        check(Style.new().notRapidBlink() == Style.new().removeModifier(Modifier.RAPID_BLINK))
        check(Style.new().notReversed() == Style.new().removeModifier(Modifier.REVERSED))
        check(Style.new().notHidden() == Style.new().removeModifier(Modifier.HIDDEN))
        check(Style.new().notCrossedOut() == Style.new().removeModifier(Modifier.CROSSED_OUT))
    }

    // -------------------------------------------------------------------------
    // From factory tests
    // -------------------------------------------------------------------------

    fun testFromColor() {
        check(Style.from(Color.Red) == Style.new().fg(Color.Red))
    }

    fun testFromColorColor() {
        check(Style.from(Color.Red, Color.Blue) == Style.new().fg(Color.Red).bg(Color.Blue))
    }

    fun testFromModifier() {
        check(
            Style.from(Modifier.BOLD or Modifier.ITALIC) ==
            Style.new().addModifier(Modifier.BOLD).addModifier(Modifier.ITALIC)
        )
    }

    fun testFromModifierModifier() {
        check(
            Style.from(Modifier.BOLD or Modifier.ITALIC, Modifier.DIM) ==
            Style.new()
                .addModifier(Modifier.BOLD)
                .addModifier(Modifier.ITALIC)
                .removeModifier(Modifier.DIM)
        )
    }

    fun testFromColorModifier() {
        check(
            Style.from(Color.Red, Modifier.BOLD or Modifier.ITALIC) ==
            Style.new()
                .fg(Color.Red)
                .addModifier(Modifier.BOLD)
                .addModifier(Modifier.ITALIC)
        )
    }

    fun testFromColorColorModifier() {
        check(
            Style.from(Color.Red, Color.Blue, Modifier.BOLD or Modifier.ITALIC) ==
            Style.new()
                .fg(Color.Red)
                .bg(Color.Blue)
                .addModifier(Modifier.BOLD)
                .addModifier(Modifier.ITALIC)
        )
    }

    fun testFromColorColorModifierModifier() {
        check(
            Style.from(Color.Red, Color.Blue, Modifier.BOLD or Modifier.ITALIC, Modifier.DIM) ==
            Style.new()
                .fg(Color.Red)
                .bg(Color.Blue)
                .addModifier(Modifier.BOLD)
                .addModifier(Modifier.ITALIC)
                .removeModifier(Modifier.DIM)
        )
    }

    // -------------------------------------------------------------------------
    // Run all tests
    // -------------------------------------------------------------------------

    fun runAll() {
        testDebug()
        testCombinedPatchGivesSameResultAsIndividualPatch()
        testModifierDebug()
        testFgCanBeStylized()
        testBgCanBeStylized()
        testAddModifierCanBeStylized()
        testRemoveModifierCanBeStylized()
        testFromColor()
        testFromColorColor()
        testFromModifier()
        testFromModifierModifier()
        testFromColorModifier()
        testFromColorColorModifier()
        testFromColorColorModifierModifier()
        println("All Style tests passed!")
    }
}

// Note: Serde serialization tests are not ported as Kotlin/Native uses different serialization
// libraries (kotlinx.serialization). Serialization support can be added separately if needed.
