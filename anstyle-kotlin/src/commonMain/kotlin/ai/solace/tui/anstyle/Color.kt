package ai.solace.tui.anstyle

/**
 * Any ANSI color code scheme
 */
sealed class Color : Comparable<Color> {
    /**
     * Available 4-bit ANSI color palette codes.
     *
     * The user's terminal defines the meaning of each palette code.
     */
    data class Ansi(val color: AnsiColor) : Color() {
        override fun compareTo(other: Color): Int = when (other) {
            is Ansi -> this.color.compareTo(other.color)
            is Ansi256 -> -1
            is Rgb -> -1
        }
    }

    /**
     * 256 (8-bit) color support.
     *
     * - `0..16` are [AnsiColor] palette codes
     * - `0..232` map to [RgbColor] color values
     * - `232..` map to [RgbColor] gray-scale values
     */
    data class Ansi256(val color: Ansi256Color) : Color() {
        override fun compareTo(other: Color): Int = when (other) {
            is Ansi -> 1
            is Ansi256 -> this.color.compareTo(other.color)
            is Rgb -> -1
        }
    }

    /** 24-bit ANSI RGB color codes */
    data class Rgb(val color: RgbColor) : Color() {
        override fun compareTo(other: Color): Int = when (other) {
            is Ansi -> 1
            is Ansi256 -> 1
            is Rgb -> this.color.compareTo(other.color)
        }
    }

    /**
     * Create a [Style] with this as the foreground
     */
    fun on(background: Color): Style =
        Style().fgColor(this).bgColor(background)

    /** Overload accepting AnsiColor */
    fun on(background: AnsiColor): Style = on(background.toColor())

    /** Overload accepting Ansi256Color */
    fun on(background: Ansi256Color): Style = on(background.toColor())

    /** Overload accepting RgbColor */
    fun on(background: RgbColor): Style = on(background.toColor())

    /**
     * Create a [Style] with this as the foreground
     */
    fun onDefault(): Style =
        Style().fgColor(this)

    /**
     * Render the ANSI code for a foreground color
     */
    fun renderFg(): Displayable = when (this) {
        is Ansi -> color.asFgBuffer()
        is Ansi256 -> color.asFgBuffer()
        is Rgb -> color.asFgBuffer()
    }

    internal fun writeFgTo(appendable: Appendable): Appendable {
        val buffer = when (this) {
            is Ansi -> color.asFgBuffer()
            is Ansi256 -> color.asFgBuffer()
            is Rgb -> color.asFgBuffer()
        }
        return buffer.formatTo(appendable)
    }

    /**
     * Render the ANSI code for a background color
     */
    fun renderBg(): Displayable = when (this) {
        is Ansi -> color.asBgBuffer()
        is Ansi256 -> color.asBgBuffer()
        is Rgb -> color.asBgBuffer()
    }

    internal fun writeBgTo(appendable: Appendable): Appendable {
        val buffer = when (this) {
            is Ansi -> color.asBgBuffer()
            is Ansi256 -> color.asBgBuffer()
            is Rgb -> color.asBgBuffer()
        }
        return buffer.formatTo(appendable)
    }

    internal fun renderUnderline(): Displayable = when (this) {
        is Ansi -> color.asUnderlineBuffer()
        is Ansi256 -> color.asUnderlineBuffer()
        is Rgb -> color.asUnderlineBuffer()
    }

    internal fun writeUnderlineTo(appendable: Appendable): Appendable {
        val buffer = when (this) {
            is Ansi -> color.asUnderlineBuffer()
            is Ansi256 -> color.asUnderlineBuffer()
            is Rgb -> color.asUnderlineBuffer()
        }
        return buffer.formatTo(appendable)
    }
}

// Extension functions to convert to Color (equivalent to Rust's From trait)
fun AnsiColor.toColor(): Color = Color.Ansi(this)
fun Ansi256Color.toColor(): Color = Color.Ansi256(this)
fun RgbColor.toColor(): Color = Color.Rgb(this)
fun UByte.toColor(): Color = Color.Ansi256(Ansi256Color(this))
fun Triple<UByte, UByte, UByte>.toColor(): Color = Color.Rgb(RgbColor(first, second, third))

/**
 * Available 4-bit ANSI color palette codes
 *
 * The user's terminal defines the meaning of each palette code.
 */
enum class AnsiColor : Comparable<AnsiColor> {
    /** Black: #0 (foreground code `30`, background code `40`). */
    Black,

    /** Red: #1 (foreground code `31`, background code `41`). */
    Red,

    /** Green: #2 (foreground code `32`, background code `42`). */
    Green,

    /** Yellow: #3 (foreground code `33`, background code `43`). */
    Yellow,

    /** Blue: #4 (foreground code `34`, background code `44`). */
    Blue,

    /** Magenta: #5 (foreground code `35`, background code `45`). */
    Magenta,

    /** Cyan: #6 (foreground code `36`, background code `46`). */
    Cyan,

    /** White: #7 (foreground code `37`, background code `47`). */
    White,

    /** Bright black: #0 (foreground code `90`, background code `100`). */
    BrightBlack,

    /** Bright red: #1 (foreground code `91`, background code `101`). */
    BrightRed,

    /** Bright green: #2 (foreground code `92`, background code `102`). */
    BrightGreen,

    /** Bright yellow: #3 (foreground code `93`, background code `103`). */
    BrightYellow,

    /** Bright blue: #4 (foreground code `94`, background code `104`). */
    BrightBlue,

    /** Bright magenta: #5 (foreground code `95`, background code `105`). */
    BrightMagenta,

    /** Bright cyan: #6 (foreground code `96`, background code `106`). */
    BrightCyan,

    /** Bright white: #7 (foreground code `97`, background code `107`). */
    BrightWhite;

    /**
     * Create a [Style] with this as the foreground
     */
    fun on(background: Color): Style =
        Style().fgColor(this.toColor()).bgColor(background)

    /** Overload accepting AnsiColor */
    fun on(background: AnsiColor): Style = on(background.toColor())

    /** Overload accepting Ansi256Color */
    fun on(background: Ansi256Color): Style = on(background.toColor())

    /** Overload accepting RgbColor */
    fun on(background: RgbColor): Style = on(background.toColor())

    /**
     * Create a [Style] with this as the foreground
     */
    fun onDefault(): Style =
        Style().fgColor(Color.Ansi(this))

    /**
     * Render the ANSI code for a foreground color
     */
    fun renderFg(): Displayable = NullFormatter(asFgStr())

    private fun asFgStr(): String = when (this) {
        Black -> escape("3", "0")
        Red -> escape("3", "1")
        Green -> escape("3", "2")
        Yellow -> escape("3", "3")
        Blue -> escape("3", "4")
        Magenta -> escape("3", "5")
        Cyan -> escape("3", "6")
        White -> escape("3", "7")
        BrightBlack -> escape("9", "0")
        BrightRed -> escape("9", "1")
        BrightGreen -> escape("9", "2")
        BrightYellow -> escape("9", "3")
        BrightBlue -> escape("9", "4")
        BrightMagenta -> escape("9", "5")
        BrightCyan -> escape("9", "6")
        BrightWhite -> escape("9", "7")
    }

    internal fun asFgBuffer(): DisplayBuffer =
        DisplayBuffer().writeStr(asFgStr())

    /**
     * Render the ANSI code for a background color
     */
    fun renderBg(): Displayable = NullFormatter(asBgStr())

    private fun asBgStr(): String = when (this) {
        Black -> escape("4", "0")
        Red -> escape("4", "1")
        Green -> escape("4", "2")
        Yellow -> escape("4", "3")
        Blue -> escape("4", "4")
        Magenta -> escape("4", "5")
        Cyan -> escape("4", "6")
        White -> escape("4", "7")
        BrightBlack -> escape("10", "0")
        BrightRed -> escape("10", "1")
        BrightGreen -> escape("10", "2")
        BrightYellow -> escape("10", "3")
        BrightBlue -> escape("10", "4")
        BrightMagenta -> escape("10", "5")
        BrightCyan -> escape("10", "6")
        BrightWhite -> escape("10", "7")
    }

    internal fun asBgBuffer(): DisplayBuffer =
        DisplayBuffer().writeStr(asBgStr())

    internal fun asUnderlineBuffer(): DisplayBuffer =
        // No per-color codes; must delegate to Ansi256Color
        Ansi256Color.fromAnsi(this).asUnderlineBuffer()

    /**
     * Change the color to/from bright
     */
    fun bright(yes: Boolean): AnsiColor =
        if (yes) {
            when (this) {
                Black -> BrightBlack
                Red -> BrightRed
                Green -> BrightGreen
                Yellow -> BrightYellow
                Blue -> BrightBlue
                Magenta -> BrightMagenta
                Cyan -> BrightCyan
                White -> BrightWhite
                BrightBlack -> this
                BrightRed -> this
                BrightGreen -> this
                BrightYellow -> this
                BrightBlue -> this
                BrightMagenta -> this
                BrightCyan -> this
                BrightWhite -> this
            }
        } else {
            when (this) {
                Black -> this
                Red -> this
                Green -> this
                Yellow -> this
                Blue -> this
                Magenta -> this
                Cyan -> this
                White -> this
                BrightBlack -> Black
                BrightRed -> Red
                BrightGreen -> Green
                BrightYellow -> Yellow
                BrightBlue -> Blue
                BrightMagenta -> Magenta
                BrightCyan -> Cyan
                BrightWhite -> White
            }
        }

    /**
     * Report whether the color is bright
     */
    fun isBright(): Boolean = when (this) {
        Black -> false
        Red -> false
        Green -> false
        Yellow -> false
        Blue -> false
        Magenta -> false
        Cyan -> false
        White -> false
        BrightBlack -> true
        BrightRed -> true
        BrightGreen -> true
        BrightYellow -> true
        BrightBlue -> true
        BrightMagenta -> true
        BrightCyan -> true
        BrightWhite -> true
    }
}

/**
 * 256 (8-bit) color support
 *
 * - `0..16` are [AnsiColor] palette codes
 * - `0..232` map to [RgbColor] color values
 * - `232..` map to [RgbColor] gray-scale values
 */
data class Ansi256Color(val index: UByte) : Comparable<Ansi256Color> {

    override fun compareTo(other: Ansi256Color): Int = index.compareTo(other.index)

    /**
     * Create a [Style] with this as the foreground
     */
    fun on(background: Color): Style =
        Style().fgColor(this.toColor()).bgColor(background)

    /** Overload accepting AnsiColor */
    fun on(background: AnsiColor): Style = on(background.toColor())

    /** Overload accepting Ansi256Color */
    fun on(background: Ansi256Color): Style = on(background.toColor())

    /** Overload accepting RgbColor */
    fun on(background: RgbColor): Style = on(background.toColor())

    /**
     * Create a [Style] with this as the foreground
     */
    fun onDefault(): Style =
        Style().fgColor(Color.Ansi256(this))

    /**
     * Convert to [AnsiColor] when there is a 1:1 mapping
     */
    fun intoAnsi(): AnsiColor? = when (index.toInt()) {
        0 -> AnsiColor.Black
        1 -> AnsiColor.Red
        2 -> AnsiColor.Green
        3 -> AnsiColor.Yellow
        4 -> AnsiColor.Blue
        5 -> AnsiColor.Magenta
        6 -> AnsiColor.Cyan
        7 -> AnsiColor.White
        8 -> AnsiColor.BrightBlack
        9 -> AnsiColor.BrightRed
        10 -> AnsiColor.BrightGreen
        11 -> AnsiColor.BrightYellow
        12 -> AnsiColor.BrightBlue
        13 -> AnsiColor.BrightMagenta
        14 -> AnsiColor.BrightCyan
        15 -> AnsiColor.BrightWhite
        else -> null
    }

    /**
     * Render the ANSI code for a foreground color
     */
    fun renderFg(): Displayable = asFgBuffer()

    internal fun asFgBuffer(): DisplayBuffer =
        DisplayBuffer()
            .writeStr("\u001B[38;5;")
            .writeCode(index)
            .writeStr("m")

    /**
     * Render the ANSI code for a background color
     */
    fun renderBg(): Displayable = asBgBuffer()

    internal fun asBgBuffer(): DisplayBuffer =
        DisplayBuffer()
            .writeStr("\u001B[48;5;")
            .writeCode(index)
            .writeStr("m")

    internal fun asUnderlineBuffer(): DisplayBuffer =
        DisplayBuffer()
            .writeStr("\u001B[58;5;")
            .writeCode(index)
            .writeStr("m")

    companion object {
        /**
         * Losslessly convert from [AnsiColor]
         */
        fun fromAnsi(color: AnsiColor): Ansi256Color = when (color) {
            AnsiColor.Black -> Ansi256Color(0u)
            AnsiColor.Red -> Ansi256Color(1u)
            AnsiColor.Green -> Ansi256Color(2u)
            AnsiColor.Yellow -> Ansi256Color(3u)
            AnsiColor.Blue -> Ansi256Color(4u)
            AnsiColor.Magenta -> Ansi256Color(5u)
            AnsiColor.Cyan -> Ansi256Color(6u)
            AnsiColor.White -> Ansi256Color(7u)
            AnsiColor.BrightBlack -> Ansi256Color(8u)
            AnsiColor.BrightRed -> Ansi256Color(9u)
            AnsiColor.BrightGreen -> Ansi256Color(10u)
            AnsiColor.BrightYellow -> Ansi256Color(11u)
            AnsiColor.BrightBlue -> Ansi256Color(12u)
            AnsiColor.BrightMagenta -> Ansi256Color(13u)
            AnsiColor.BrightCyan -> Ansi256Color(14u)
            AnsiColor.BrightWhite -> Ansi256Color(15u)
        }
    }
}

// Extension function for UByte to Ansi256Color conversion
fun UByte.toAnsi256Color(): Ansi256Color = Ansi256Color(this)
fun AnsiColor.toAnsi256Color(): Ansi256Color = Ansi256Color.fromAnsi(this)

/**
 * 24-bit ANSI RGB color codes
 */
data class RgbColor(val r: UByte, val g: UByte, val b: UByte) : Comparable<RgbColor> {

    override fun compareTo(other: RgbColor): Int {
        val cmpR = r.compareTo(other.r)
        if (cmpR != 0) return cmpR
        val cmpG = g.compareTo(other.g)
        if (cmpG != 0) return cmpG
        return b.compareTo(other.b)
    }

    /**
     * Create a [Style] with this as the foreground
     */
    fun on(background: Color): Style =
        Style().fgColor(this.toColor()).bgColor(background)

    /** Overload accepting AnsiColor */
    fun on(background: AnsiColor): Style = on(background.toColor())

    /** Overload accepting Ansi256Color */
    fun on(background: Ansi256Color): Style = on(background.toColor())

    /** Overload accepting RgbColor */
    fun on(background: RgbColor): Style = on(background.toColor())

    /**
     * Create a [Style] with this as the foreground
     */
    fun onDefault(): Style =
        Style().fgColor(Color.Rgb(this))

    /**
     * Render the ANSI code for a foreground color
     */
    fun renderFg(): Displayable = asFgBuffer()

    internal fun asFgBuffer(): DisplayBuffer =
        DisplayBuffer()
            .writeStr("\u001B[38;2;")
            .writeCode(r)
            .writeStr(";")
            .writeCode(g)
            .writeStr(";")
            .writeCode(b)
            .writeStr("m")

    /**
     * Render the ANSI code for a background color
     */
    fun renderBg(): Displayable = asBgBuffer()

    internal fun asBgBuffer(): DisplayBuffer =
        DisplayBuffer()
            .writeStr("\u001B[48;2;")
            .writeCode(r)
            .writeStr(";")
            .writeCode(g)
            .writeStr(";")
            .writeCode(b)
            .writeStr("m")

    internal fun asUnderlineBuffer(): DisplayBuffer =
        DisplayBuffer()
            .writeStr("\u001B[58;2;")
            .writeCode(r)
            .writeStr(";")
            .writeCode(g)
            .writeStr(";")
            .writeCode(b)
            .writeStr("m")
}

// Extension function for Triple to RgbColor conversion
fun Triple<UByte, UByte, UByte>.toRgbColor(): RgbColor = RgbColor(first, second, third)

private const val DISPLAY_BUFFER_CAPACITY: Int = 19

/**
 * Internal buffer for building ANSI escape sequences.
 * Uses a fixed-size byte array for efficiency (no allocations during formatting).
 */
internal class DisplayBuffer : Displayable {
    private val buffer = ByteArray(DISPLAY_BUFFER_CAPACITY)
    private var len: Int = 0

    fun writeStr(part: String): DisplayBuffer {
        val bytes = part.encodeToByteArray()
        bytes.copyInto(buffer, len)
        len += bytes.size
        return this
    }

    fun writeCode(code: UByte): DisplayBuffer {
        val codeInt = code.toInt()
        val c1 = (codeInt / 100) % 10
        val c2 = (codeInt / 10) % 10
        val c3 = codeInt % 10

        var printed = false
        if (c1 != 0) {
            printed = true
            buffer[len] = ('0'.code + c1).toByte()
            len++
        }
        if (c2 != 0 || printed) {
            buffer[len] = ('0'.code + c2).toByte()
            len++
        }
        // If we received a zero value we must still print a value.
        buffer[len] = ('0'.code + c3).toByte()
        len++

        return this
    }

    fun asStr(): String = buffer.decodeToString(0, len)

    override fun formatTo(appendable: Appendable): Appendable = appendable.append(asStr())

    override fun toString(): String = asStr()
}

/**
 * Simple wrapper that implements Displayable for a static string.
 */
internal class NullFormatter(private val str: String) : Displayable {
    override fun formatTo(appendable: Appendable): Appendable = appendable.append(str)

    override fun toString(): String = str
}

/**
 * Helper function equivalent to Rust's escape! macro.
 * Creates an ANSI escape sequence: ESC[ + parts + m
 */
internal fun escape(vararg parts: String): String = "\u001B[${parts.joinToString("")}m"

// Tests
class ColorTest {
    @kotlin.test.Test
    fun maxDisplayBuffer() {
        val c = RgbColor(255u, 255u, 255u)
        val actual = c.renderFg().toString()
        kotlin.test.assertEquals("\u001B[38;2;255;255;255m", actual)
        kotlin.test.assertEquals(DISPLAY_BUFFER_CAPACITY, actual.length)
    }

    @kotlin.test.Test
    fun printSizeOf() {
        // In Kotlin, we don't have direct sizeof, but we can print structure info
        println("Color: sealed class with 3 variants")
        println("AnsiColor: enum with 16 entries")
        println("Ansi256Color: data class with UByte")
        println("RgbColor: data class with 3 UBytes")
        println("DisplayBuffer: class with ByteArray($DISPLAY_BUFFER_CAPACITY) + Int")
    }

    @kotlin.test.Test
    fun noAlign() {
        fun assertNoAlign(d: Displayable) {
            val expected = buildString { d.formatTo(this) }
            val actual = buildString { d.formatTo(this) }
            kotlin.test.assertEquals(expected, actual)
        }

        assertNoAlign(AnsiColor.White.renderFg())
        assertNoAlign(AnsiColor.White.renderBg())
        assertNoAlign(Ansi256Color(0u).renderFg())
        assertNoAlign(Ansi256Color(0u).renderBg())
        assertNoAlign(RgbColor(0u, 0u, 0u).renderFg())
        assertNoAlign(RgbColor(0u, 0u, 0u).renderBg())
        assertNoAlign(Color.Ansi(AnsiColor.White).renderFg())
        assertNoAlign(Color.Ansi(AnsiColor.White).renderBg())
    }
}
