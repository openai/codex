package anstyle

/**
 * ANSI Text styling
 *
 * You can use a `Style` to render the corresponding ANSI code.
 *
 * Example:
 * ```kotlin
 * val style = Style().bold()
 *
 * val value = 42
 * println("${style.render()}$value${style.renderReset()}")
 * ```
 */
data class Style(
    private val fg: Color? = null,
    private val bg: Color? = null,
    private val underline: Color? = null,
    private val effects: Effects = Effects.PLAIN
) : Displayable, Comparable<Style> {

    override fun compareTo(other: Style): Int {
        // Compare fg
        val fgCmp = compareNullable(fg, other.fg)
        if (fgCmp != 0) return fgCmp
        // Compare bg
        val bgCmp = compareNullable(bg, other.bg)
        if (bgCmp != 0) return bgCmp
        // Compare underline
        val ulCmp = compareNullable(underline, other.underline)
        if (ulCmp != 0) return ulCmp
        // Compare effects
        return effects.compareTo(other.effects)
    }

    private fun <T : Comparable<T>> compareNullable(a: T?, b: T?): Int {
        return when {
            a == null && b == null -> 0
            a == null -> -1
            b == null -> 1
            else -> a.compareTo(b)
        }
    }

    // # Core

    /**
     * Set foreground color
     *
     * Example:
     * ```kotlin
     * val style = Style().fgColor(AnsiColor.Red.toColor())
     * ```
     */
    fun fgColor(fg: Color?): Style = copy(fg = fg)

    /**
     * Set foreground color (convenience overload)
     */
    fun fgColor(fg: Color): Style = copy(fg = fg)

    /**
     * Set background color
     *
     * Example:
     * ```kotlin
     * val style = Style().bgColor(AnsiColor.Red.toColor())
     * ```
     */
    fun bgColor(bg: Color?): Style = copy(bg = bg)

    /**
     * Set background color (convenience overload)
     */
    fun bgColor(bg: Color): Style = copy(bg = bg)

    /**
     * Set underline color
     *
     * Example:
     * ```kotlin
     * val style = Style().underlineColor(AnsiColor.Red.toColor())
     * ```
     */
    fun underlineColor(underline: Color?): Style = copy(underline = underline)

    /**
     * Set text effects
     *
     * Example:
     * ```kotlin
     * val style = Style().effects(Effects.BOLD or Effects.UNDERLINE)
     * ```
     */
    fun effects(effects: Effects): Style = copy(effects = effects)

    /**
     * Render the ANSI code
     *
     * `Style` also implements [Displayable] directly, so calling this method is optional.
     */
    fun render(): Displayable = StyleDisplay(this)

    override fun formatTo(appendable: Appendable): Appendable {
        effects.render().formatTo(appendable)

        fg?.let { it.renderFg().formatTo(appendable) }
        bg?.let { it.renderBg().formatTo(appendable) }
        underline?.let { it.renderUnderline().formatTo(appendable) }

        return appendable
    }

    /**
     * Write the ANSI code
     */
    fun writeTo(appendable: Appendable): Appendable {
        effects.writeTo(appendable)

        fg?.let { it.writeFgTo(appendable) }
        bg?.let { it.writeBgTo(appendable) }
        underline?.let { it.writeUnderlineTo(appendable) }

        return appendable
    }

    /**
     * Renders the relevant [Reset] code
     *
     * Unlike [Reset.render], this will elide the code if there is nothing to reset.
     */
    fun renderReset(): Displayable =
        if (this != Style()) Reset else object : Displayable {
            override fun formatTo(appendable: Appendable): Appendable = appendable
            override fun toString(): String = ""
        }

    /**
     * Write the relevant [Reset] code
     *
     * Unlike [Reset.render], this will elide the code if there is nothing to reset.
     */
    fun writeResetTo(appendable: Appendable): Appendable {
        if (this != Style()) {
            appendable.append(RESET)
        }
        return appendable
    }

    // # Convenience

    /**
     * Apply `bold` effect
     *
     * Example:
     * ```kotlin
     * val style = Style().bold()
     * ```
     */
    fun bold(): Style = copy(effects = effects.insert(Effects.BOLD))

    /**
     * Apply `dimmed` effect
     *
     * Example:
     * ```kotlin
     * val style = Style().dimmed()
     * ```
     */
    fun dimmed(): Style = copy(effects = effects.insert(Effects.DIMMED))

    /**
     * Apply `italic` effect
     *
     * Example:
     * ```kotlin
     * val style = Style().italic()
     * ```
     */
    fun italic(): Style = copy(effects = effects.insert(Effects.ITALIC))

    /**
     * Apply `underline` effect
     *
     * Example:
     * ```kotlin
     * val style = Style().underline()
     * ```
     */
    fun underline(): Style = copy(effects = effects.insert(Effects.UNDERLINE))

    /**
     * Apply `blink` effect
     *
     * Example:
     * ```kotlin
     * val style = Style().blink()
     * ```
     */
    fun blink(): Style = copy(effects = effects.insert(Effects.BLINK))

    /**
     * Apply `invert` effect
     *
     * Example:
     * ```kotlin
     * val style = Style().invert()
     * ```
     */
    fun invert(): Style = copy(effects = effects.insert(Effects.INVERT))

    /**
     * Apply `hidden` effect
     *
     * Example:
     * ```kotlin
     * val style = Style().hidden()
     * ```
     */
    fun hidden(): Style = copy(effects = effects.insert(Effects.HIDDEN))

    /**
     * Apply `strikethrough` effect
     *
     * Example:
     * ```kotlin
     * val style = Style().strikethrough()
     * ```
     */
    fun strikethrough(): Style = copy(effects = effects.insert(Effects.STRIKETHROUGH))

    // # Reflection

    /** Get the foreground color */
    fun getFgColor(): Color? = fg

    /** Get the background color */
    fun getBgColor(): Color? = bg

    /** Get the underline color */
    fun getUnderlineColor(): Color? = underline

    /** Get the effects */
    fun getEffects(): Effects = effects

    /**
     * Check if no styling is enabled
     */
    fun isPlain(): Boolean =
        fg == null && bg == null && underline == null && effects.isPlain()

    override fun toString(): String = buildString { formatTo(this) }
}

// Extension function: convert Effects to Style
fun Effects.toStyle(): Style = Style(effects = this)

// Operator extensions for Style
infix fun Style.or(effects: Effects): Style = copy(effects = getEffects().insert(effects))
operator fun Style.minus(effects: Effects): Style = copy(effects = getEffects().remove(effects))

// Equality comparison with Effects
fun Style.equals(effects: Effects): Boolean = this == effects.toStyle()

internal class StyleDisplay(private val style: Style) : Displayable {
    override fun formatTo(appendable: Appendable): Appendable = style.formatTo(appendable)

    override fun toString(): String = buildString { formatTo(this) }
}

// Tests
class StyleTest {
    @kotlin.test.Test
    fun printSizeOf() {
        // In Kotlin, data class size depends on JVM/Native implementation
        println("Style: data class with 4 fields (3 nullable Color + Effects)")
        println("StyleDisplay: class wrapping Style")
    }

    @kotlin.test.Test
    fun basicUsage() {
        val style = Style().bold()
        kotlin.test.assertTrue(style.getEffects().contains(Effects.BOLD))
    }
}
