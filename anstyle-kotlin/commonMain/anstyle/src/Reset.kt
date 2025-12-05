package anstyle

/**
 * Interface mirroring Rust's core::fmt::Display trait.
 * Types implementing this can be formatted into an Appendable.
 */
interface Displayable {
    /**
     * Formats this value into the given Appendable.
     */
    fun formatTo(appendable: Appendable): Appendable
}

/**
 * Reset terminal formatting
 */
object Reset : Displayable, Comparable<Reset> {
    /**
     * Render the ANSI code
     *
     * [Reset] also implements [Displayable] directly, so calling this method is optional.
     */
    fun render(): Displayable = this

    override fun formatTo(appendable: Appendable): Appendable = appendable.append(RESET)

    override fun toString(): String = RESET

    override fun compareTo(other: Reset): Int = 0
}


internal const val RESET: String = "\u001B[0m"

// Tests
class ResetTest {
    @kotlin.test.Test
    fun printSizeOf() {
        // Reset is a singleton object (zero-sized equivalent in Kotlin)
        println("Reset: object (singleton)")
    }

    @kotlin.test.Test
    fun noAlign() {
        fun assertNoAlign(d: Displayable) {
            val expected = buildString { d.formatTo(this) }
            val actual = buildString { d.formatTo(this) }
            kotlin.test.assertEquals(expected, actual)
        }

        assertNoAlign(Reset)
        assertNoAlign(Reset.render())
    }
}
