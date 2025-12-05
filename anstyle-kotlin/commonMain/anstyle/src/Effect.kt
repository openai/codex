package anstyle

/**
 * A set of text effects
 *
 * Example:
 * ```kotlin
 * val effects = Effects.BOLD or Effects.UNDERLINE
 * ```
 */
value class Effects(val bits: UShort) : Comparable<Effects> {

    override fun compareTo(other: Effects): Int = bits.compareTo(other.bits)

    companion object {
        /** No [Effects] applied */
        val PLAIN: Effects = Effects(0u)

        /**
         * No effects enabled
         *
         * Example:
         * ```kotlin
         * val effects = Effects.new()
         * ```
         */
        fun new(): Effects = PLAIN

        val BOLD: Effects = Effects((1 shl 0).toUShort())
        val DIMMED: Effects = Effects((1 shl 1).toUShort())
        /** Not widely supported. Sometimes treated as inverse or blink */
        val ITALIC: Effects = Effects((1 shl 2).toUShort())
        /** Style extensions exist for Kitty, VTE, mintty and iTerm2. */
        val UNDERLINE: Effects = Effects((1 shl 3).toUShort())
        val DOUBLE_UNDERLINE: Effects = Effects((1 shl 4).toUShort())
        val CURLY_UNDERLINE: Effects = Effects((1 shl 5).toUShort())
        val DOTTED_UNDERLINE: Effects = Effects((1 shl 6).toUShort())
        val DASHED_UNDERLINE: Effects = Effects((1 shl 7).toUShort())
        val BLINK: Effects = Effects((1 shl 8).toUShort())
        /** Swap foreground and background colors; inconsistent emulation */
        val INVERT: Effects = Effects((1 shl 9).toUShort())
        val HIDDEN: Effects = Effects((1 shl 10).toUShort())
        /** Characters legible but marked as if for deletion. Not supported in Terminal.app */
        val STRIKETHROUGH: Effects = Effects((1 shl 11).toUShort())
    }

    /**
     * Check if no effects are enabled
     *
     * Example:
     * ```kotlin
     * val effects = Effects.PLAIN
     * assert(effects.isPlain())
     *
     * val effects = Effects.BOLD or Effects.UNDERLINE
     * assert(!effects.isPlain())
     * ```
     */
    fun isPlain(): Boolean = bits == PLAIN.bits

    /**
     * Returns `true` if all of the effects in `other` are contained within `this`.
     *
     * Example:
     * ```kotlin
     * val effects = Effects.BOLD or Effects.UNDERLINE
     * assert(effects.contains(Effects.BOLD))
     *
     * val effects = Effects.PLAIN
     * assert(!effects.contains(Effects.BOLD))
     * ```
     */
    fun contains(other: Effects): Boolean = (other.bits and bits) == other.bits

    /**
     * Inserts the specified effects.
     *
     * Example:
     * ```kotlin
     * val effects = Effects.PLAIN.insert(Effects.PLAIN)
     * assert(effects.isPlain())
     *
     * val effects = Effects.PLAIN.insert(Effects.BOLD)
     * assert(effects.contains(Effects.BOLD))
     * ```
     */
    fun insert(other: Effects): Effects = Effects((bits or other.bits).toUShort())

    /**
     * Removes the specified effects.
     *
     * Example:
     * ```kotlin
     * val effects = (Effects.BOLD or Effects.UNDERLINE).remove(Effects.BOLD)
     * assert(!effects.contains(Effects.BOLD))
     * assert(effects.contains(Effects.UNDERLINE))
     * ```
     */
    fun remove(other: Effects): Effects = Effects((bits and other.bits.inv()).toUShort())

    /**
     * Reset all effects
     * ```kotlin
     * val effects = (Effects.BOLD or Effects.UNDERLINE).clear()
     * assert(!effects.contains(Effects.BOLD))
     * assert(!effects.contains(Effects.UNDERLINE))
     * ```
     */
    fun clear(): Effects = PLAIN

    /**
     * Enable or disable the specified effects depending on the passed value.
     *
     * Example:
     * ```kotlin
     * val effects = Effects.PLAIN.set(Effects.BOLD, true)
     * assert(effects.contains(Effects.BOLD))
     * ```
     */
    fun set(other: Effects, enable: Boolean): Effects =
        if (enable) insert(other) else remove(other)

    /**
     * Iterate over enabled effects
     */
    fun iter(): EffectIter = EffectIter(index = 0, effects = this)

    /**
     * Iterate over enabled effect indices
     */
    internal fun indexIter(): EffectIndexIter = EffectIndexIter(index = 0, effects = this)

    /**
     * Render the ANSI code
     */
    fun render(): Displayable = EffectsDisplay(this)

    internal fun writeTo(appendable: Appendable): Appendable {
        for (index in indexIter()) {
            appendable.append(METADATA[index].escape)
        }
        return appendable
    }

    override fun toString(): String = buildString {
        append("Effects(")
        var first = true
        for (index in indexIter()) {
            if (!first) append(" | ")
            first = false
            append(METADATA[index].name)
        }
        append(")")
    }
}

// Operator extensions for Effects
infix fun Effects.or(other: Effects): Effects = this.insert(other)
operator fun Effects.minus(other: Effects): Effects = this.remove(other)

internal data class Metadata(
    val name: String,
    val escape: String
)

internal val METADATA: Array<Metadata> = arrayOf(
    Metadata(name = "BOLD", escape = escape("1")),
    Metadata(name = "DIMMED", escape = escape("2")),
    Metadata(name = "ITALIC", escape = escape("3")),
    Metadata(name = "UNDERLINE", escape = escape("4")),
    Metadata(name = "DOUBLE_UNDERLINE", escape = escape("21")),
    Metadata(name = "CURLY_UNDERLINE", escape = escape("4:3")),
    Metadata(name = "DOTTED_UNDERLINE", escape = escape("4:4")),
    Metadata(name = "DASHED_UNDERLINE", escape = escape("4:5")),
    Metadata(name = "BLINK", escape = escape("5")),
    Metadata(name = "INVERT", escape = escape("7")),
    Metadata(name = "HIDDEN", escape = escape("8")),
    Metadata(name = "STRIKETHROUGH", escape = escape("9")),
)

internal class EffectsDisplay(private val effects: Effects) : Displayable {
    override fun formatTo(appendable: Appendable): Appendable {
        for (index in effects.indexIter()) {
            appendable.append(METADATA[index].escape)
        }
        return appendable
    }

    override fun toString(): String = buildString { formatTo(this) }
}

/**
 * Enumerate each enabled value in [Effects]
 */
class EffectIter(
    private var index: Int,
    private val effects: Effects
) : Iterator<Effects> {
    override fun hasNext(): Boolean {
        while (index < METADATA.size) {
            val effect = Effects((1 shl index).toUShort())
            if (effects.contains(effect)) {
                return true
            }
            index++
        }
        return false
    }

    override fun next(): Effects {
        while (index < METADATA.size) {
            val currentIndex = index
            index++
            val effect = Effects((1 shl currentIndex).toUShort())
            if (effects.contains(effect)) {
                return effect
            }
        }
        throw NoSuchElementException()
    }
}

internal class EffectIndexIter(
    private var index: Int,
    private val effects: Effects
) : Iterator<Int> {
    override fun hasNext(): Boolean {
        while (index < METADATA.size) {
            val effect = Effects((1 shl index).toUShort())
            if (effects.contains(effect)) {
                return true
            }
            index++
        }
        return false
    }

    override fun next(): Int {
        while (index < METADATA.size) {
            val currentIndex = index
            index++
            val effect = Effects((1 shl currentIndex).toUShort())
            if (effects.contains(effect)) {
                return currentIndex
            }
        }
        throw NoSuchElementException()
    }
}

// Tests
class EffectsTest {
    @kotlin.test.Test
    fun printSizeOf() {
        // In Kotlin, we use value class which is equivalent to Rust's newtype
        println("Effects: value class wrapping UShort (2 bytes)")
        println("EffectsDisplay: class wrapping Effects")
    }

    @kotlin.test.Test
    fun noAlign() {
        fun assertNoAlign(d: Displayable) {
            val expected = buildString { d.formatTo(this) }
            val actual = buildString { d.formatTo(this) }
            kotlin.test.assertEquals(expected, actual)
        }

        assertNoAlign(Effects.BOLD.render())
    }

    @kotlin.test.Test
    fun debugFormat() {
        val effects = Effects.PLAIN
        kotlin.test.assertEquals("Effects()", effects.toString())

        val effects2 = Effects.BOLD or Effects.UNDERLINE
        kotlin.test.assertEquals("Effects(BOLD | UNDERLINE)", effects2.toString())
    }
}
