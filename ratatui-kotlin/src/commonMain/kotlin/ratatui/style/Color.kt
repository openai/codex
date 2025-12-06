package ratatui.style

/**
 * ANSI Color
 *
 * All colors from the [ANSI color table](https://en.wikipedia.org/wiki/ANSI_escape_code#Colors)
 * are supported (though some names are not exactly the same).
 *
 * | Color Name     | Color                   | Foreground | Background |
 * |----------------|-------------------------|------------|------------|
 * | `black`        | [Color.Black]           | 30         | 40         |
 * | `red`          | [Color.Red]             | 31         | 41         |
 * | `green`        | [Color.Green]           | 32         | 42         |
 * | `yellow`       | [Color.Yellow]          | 33         | 43         |
 * | `blue`         | [Color.Blue]            | 34         | 44         |
 * | `magenta`      | [Color.Magenta]         | 35         | 45         |
 * | `cyan`         | [Color.Cyan]            | 36         | 46         |
 * | `gray`*        | [Color.Gray]            | 37         | 47         |
 * | `darkgray`*    | [Color.DarkGray]        | 90         | 100        |
 * | `lightred`     | [Color.LightRed]        | 91         | 101        |
 * | `lightgreen`   | [Color.LightGreen]      | 92         | 102        |
 * | `lightyellow`  | [Color.LightYellow]     | 93         | 103        |
 * | `lightblue`    | [Color.LightBlue]       | 94         | 104        |
 * | `lightmagenta` | [Color.LightMagenta]    | 95         | 105        |
 * | `lightcyan`    | [Color.LightCyan]       | 96         | 106        |
 * | `white`*       | [Color.White]           | 97         | 107        |
 *
 * - `gray` is sometimes called `white` - this is not supported as we use `white` for bright white
 * - `gray` is sometimes called `silver` - this is supported
 * - `darkgray` is sometimes called `light black` or `bright black` (both are supported)
 * - `white` is sometimes called `light white` or `bright white` (both are supported)
 * - we support `bright` and `light` prefixes for all colors
 * - we support `-` and `_` and ` ` as separators for all colors
 * - we support both `gray` and `grey` spellings
 *
 * `Color.toStyle()` is implemented by creating a style with the foreground color set to the
 * given color. This allows you to use colors anywhere that accepts a Style.
 *
 * Example:
 * ```kotlin
 * assertEquals(Color.fromStr("red"), Color.Red)
 * assertEquals(Color.fromStr("lightred"), Color.LightRed)
 * assertEquals(Color.fromStr("light red"), Color.LightRed)
 * assertEquals(Color.fromStr("light-red"), Color.LightRed)
 * assertEquals(Color.fromStr("light_red"), Color.LightRed)
 * assertEquals(Color.fromStr("bright red"), Color.LightRed)
 * assertEquals(Color.fromStr("silver"), Color.Gray)
 * assertEquals(Color.fromStr("dark-grey"), Color.DarkGray)
 * assertEquals(Color.fromStr("white"), Color.White)
 * ```
 */
sealed class Color {
    /** Resets the foreground or background color */
    data object Reset : Color()

    /** ANSI Color: Black. Foreground: 30, Background: 40 */
    data object Black : Color()

    /** ANSI Color: Red. Foreground: 31, Background: 41 */
    data object Red : Color()

    /** ANSI Color: Green. Foreground: 32, Background: 42 */
    data object Green : Color()

    /** ANSI Color: Yellow. Foreground: 33, Background: 43 */
    data object Yellow : Color()

    /** ANSI Color: Blue. Foreground: 34, Background: 44 */
    data object Blue : Color()

    /** ANSI Color: Magenta. Foreground: 35, Background: 45 */
    data object Magenta : Color()

    /** ANSI Color: Cyan. Foreground: 36, Background: 46 */
    data object Cyan : Color()

    /**
     * ANSI Color: White. Foreground: 37, Background: 47
     *
     * Note that this is sometimes called `silver` or `white` but we use `white` for bright white
     */
    data object Gray : Color()

    /**
     * ANSI Color: Bright Black. Foreground: 90, Background: 100
     *
     * Note that this is sometimes called `light black` or `bright black` but we use `dark gray`
     */
    data object DarkGray : Color()

    /** ANSI Color: Bright Red. Foreground: 91, Background: 101 */
    data object LightRed : Color()

    /** ANSI Color: Bright Green. Foreground: 92, Background: 102 */
    data object LightGreen : Color()

    /** ANSI Color: Bright Yellow. Foreground: 93, Background: 103 */
    data object LightYellow : Color()

    /** ANSI Color: Bright Blue. Foreground: 94, Background: 104 */
    data object LightBlue : Color()

    /** ANSI Color: Bright Magenta. Foreground: 95, Background: 105 */
    data object LightMagenta : Color()

    /** ANSI Color: Bright Cyan. Foreground: 96, Background: 106 */
    data object LightCyan : Color()

    /**
     * ANSI Color: Bright White. Foreground: 97, Background: 107
     * Sometimes called `bright white` or `light white` in some terminals
     */
    data object White : Color()

    /**
     * An RGB color.
     *
     * Note that only terminals that support 24-bit true color will display this correctly.
     * Notably versions of Windows Terminal prior to Windows 10 and macOS Terminal.app do not
     * support this.
     *
     * See also: https://en.wikipedia.org/wiki/ANSI_escape_code#24-bit
     */
    data class Rgb(val r: UByte, val g: UByte, val b: UByte) : Color()

    /**
     * An 8-bit 256 color.
     *
     * See also: https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit
     */
    data class Indexed(val index: UByte) : Color()

    companion object {
        /** Default color (Reset) */
        fun default(): Color = Reset

        /**
         * Convert a UInt to a Color
         *
         * The UInt should be in the format 0x00RRGGBB.
         */
        fun fromU32(u: UInt): Color {
            val r = ((u shr 16) and 0xFFu).toUByte()
            val g = ((u shr 8) and 0xFFu).toUByte()
            val b = (u and 0xFFu).toUByte()
            return Rgb(r, g, b)
        }

        /**
         * Parse a string to a Color.
         *
         * Supports named colors, RGB hex values (#RRGGBB), and indexed colors (0-255).
         *
         * @throws ParseColorError if the string cannot be parsed
         */
        fun fromStr(s: String): Color {
            // There is a mix of different color names and formats in the wild.
            // This is an attempt to support as many as possible.
            val normalized = s.lowercase()
                .replace(" ", "")
                .replace("-", "")
                .replace("_", "")
                .replace("bright", "light")
                .replace("grey", "gray")
                .replace("silver", "gray")
                .replace("lightblack", "darkgray")
                .replace("lightwhite", "white")
                .replace("lightgray", "white")

            return when (normalized) {
                "reset" -> Reset
                "black" -> Black
                "red" -> Red
                "green" -> Green
                "yellow" -> Yellow
                "blue" -> Blue
                "magenta" -> Magenta
                "cyan" -> Cyan
                "gray" -> Gray
                "darkgray" -> DarkGray
                "lightred" -> LightRed
                "lightgreen" -> LightGreen
                "lightyellow" -> LightYellow
                "lightblue" -> LightBlue
                "lightmagenta" -> LightMagenta
                "lightcyan" -> LightCyan
                "white" -> White
                else -> {
                    // Try parsing as indexed color (0-255)
                    s.toUByteOrNull()?.let { return Indexed(it) }

                    // Try parsing as hex color
                    parseHexColor(s)?.let { (r, g, b) -> return Rgb(r, g, b) }

                    throw ParseColorError()
                }
            }
        }

        /**
         * Try to parse a string to a Color, returning null on failure.
         */
        fun fromStrOrNull(s: String): Color? = try {
            fromStr(s)
        } catch (e: ParseColorError) {
            null
        }
    }

    override fun toString(): String = when (this) {
        is Reset -> "Reset"
        is Black -> "Black"
        is Red -> "Red"
        is Green -> "Green"
        is Yellow -> "Yellow"
        is Blue -> "Blue"
        is Magenta -> "Magenta"
        is Cyan -> "Cyan"
        is Gray -> "Gray"
        is DarkGray -> "DarkGray"
        is LightRed -> "LightRed"
        is LightGreen -> "LightGreen"
        is LightYellow -> "LightYellow"
        is LightBlue -> "LightBlue"
        is LightMagenta -> "LightMagenta"
        is LightCyan -> "LightCyan"
        is White -> "White"
        is Rgb -> "#${r.toString(16).padStart(2, '0').uppercase()}${g.toString(16).padStart(2, '0').uppercase()}${b.toString(16).padStart(2, '0').uppercase()}"
        is Indexed -> index.toString()
    }
}

/** Error type indicating a failure to parse a color string. */
class ParseColorError : Exception("Failed to parse Colors")

private fun parseHexColor(input: String): Triple<UByte, UByte, UByte>? {
    if (!input.startsWith('#') || input.length != 7) {
        return null
    }
    val r = input.substring(1, 3).toUByteOrNull(16) ?: return null
    val g = input.substring(3, 5).toUByteOrNull(16) ?: return null
    val b = input.substring(5, 7).toUByteOrNull(16) ?: return null
    return Triple(r, g, b)
}

// Extension functions for Color conversion
fun Triple<UByte, UByte, UByte>.toColor(): Color = Color.Rgb(first, second, third)
fun UByte.toIndexedColor(): Color = Color.Indexed(this)

// =============================================================================
// Tests
// =============================================================================
// Note: HSL/Hsluv palette tests are not ported as we don't have the palette library.
// Serde tests are not ported as serialization is handled differently in Kotlin.

/**
 * Unit tests for Color parsing and display.
 *
 * In Kotlin, tests would typically be in a separate test source set.
 * These are included here as reference implementations matching the Rust tests.
 */
internal object ColorTests {

    // -------------------------------------------------------------------------
    // fromU32 tests
    // -------------------------------------------------------------------------

    fun testFromU32() {
        check(Color.fromU32(0x000000u) == Color.Rgb(0u, 0u, 0u))
        check(Color.fromU32(0xFF0000u) == Color.Rgb(255u, 0u, 0u))
        check(Color.fromU32(0x00FF00u) == Color.Rgb(0u, 255u, 0u))
        check(Color.fromU32(0x0000FFu) == Color.Rgb(0u, 0u, 255u))
        check(Color.fromU32(0xFFFFFFu) == Color.Rgb(255u, 255u, 255u))
    }

    // -------------------------------------------------------------------------
    // fromStr RGB color test
    // -------------------------------------------------------------------------

    fun testFromRgbColor() {
        val color: Color = Color.fromStr("#FF0000")
        check(color == Color.Rgb(255u, 0u, 0u))
    }

    // -------------------------------------------------------------------------
    // fromStr indexed color test
    // -------------------------------------------------------------------------

    fun testFromIndexedColor() {
        val color: Color = Color.fromStr("10")
        check(color == Color.Indexed(10u))
    }

    // -------------------------------------------------------------------------
    // fromStr ANSI color tests
    // -------------------------------------------------------------------------

    fun testFromAnsiColor() {
        check(Color.fromStr("reset") == Color.Reset)
        check(Color.fromStr("black") == Color.Black)
        check(Color.fromStr("red") == Color.Red)
        check(Color.fromStr("green") == Color.Green)
        check(Color.fromStr("yellow") == Color.Yellow)
        check(Color.fromStr("blue") == Color.Blue)
        check(Color.fromStr("magenta") == Color.Magenta)
        check(Color.fromStr("cyan") == Color.Cyan)
        check(Color.fromStr("gray") == Color.Gray)
        check(Color.fromStr("darkgray") == Color.DarkGray)
        check(Color.fromStr("lightred") == Color.LightRed)
        check(Color.fromStr("lightgreen") == Color.LightGreen)
        check(Color.fromStr("lightyellow") == Color.LightYellow)
        check(Color.fromStr("lightblue") == Color.LightBlue)
        check(Color.fromStr("lightmagenta") == Color.LightMagenta)
        check(Color.fromStr("lightcyan") == Color.LightCyan)
        check(Color.fromStr("white") == Color.White)

        // aliases
        check(Color.fromStr("lightblack") == Color.DarkGray)
        check(Color.fromStr("lightwhite") == Color.White)
        check(Color.fromStr("lightgray") == Color.White)

        // silver = grey = gray
        check(Color.fromStr("grey") == Color.Gray)
        check(Color.fromStr("silver") == Color.Gray)

        // spaces are ignored
        check(Color.fromStr("light black") == Color.DarkGray)
        check(Color.fromStr("light white") == Color.White)
        check(Color.fromStr("light gray") == Color.White)

        // dashes are ignored
        check(Color.fromStr("light-black") == Color.DarkGray)
        check(Color.fromStr("light-white") == Color.White)
        check(Color.fromStr("light-gray") == Color.White)

        // underscores are ignored
        check(Color.fromStr("light_black") == Color.DarkGray)
        check(Color.fromStr("light_white") == Color.White)
        check(Color.fromStr("light_gray") == Color.White)

        // bright = light
        check(Color.fromStr("bright-black") == Color.DarkGray)
        check(Color.fromStr("bright-white") == Color.White)

        // bright = light
        check(Color.fromStr("brightblack") == Color.DarkGray)
        check(Color.fromStr("brightwhite") == Color.White)
    }

    // -------------------------------------------------------------------------
    // Invalid color parsing tests
    // -------------------------------------------------------------------------

    fun testFromInvalidColors() {
        val badColors = listOf(
            "invalid_color", // not a color string
            "abcdef0",       // 7 chars is not a color
            " bcdefa",       // doesn't start with a '#'
            "#abcdef00",     // too many chars
            "#1\uD83E\uDD80" + "2", // len 7 but on char boundaries shouldn't panic (crab emoji)
            "resets",        // typo
            "lightblackk",   // typo
        )

        for (badColor in badColors) {
            val result = Color.fromStrOrNull(badColor)
            check(result == null) { "bad color: '$badColor'" }
        }
    }

    // -------------------------------------------------------------------------
    // toString (display) tests
    // -------------------------------------------------------------------------

    fun testDisplay() {
        check(Color.Black.toString() == "Black")
        check(Color.Red.toString() == "Red")
        check(Color.Green.toString() == "Green")
        check(Color.Yellow.toString() == "Yellow")
        check(Color.Blue.toString() == "Blue")
        check(Color.Magenta.toString() == "Magenta")
        check(Color.Cyan.toString() == "Cyan")
        check(Color.Gray.toString() == "Gray")
        check(Color.DarkGray.toString() == "DarkGray")
        check(Color.LightRed.toString() == "LightRed")
        check(Color.LightGreen.toString() == "LightGreen")
        check(Color.LightYellow.toString() == "LightYellow")
        check(Color.LightBlue.toString() == "LightBlue")
        check(Color.LightMagenta.toString() == "LightMagenta")
        check(Color.LightCyan.toString() == "LightCyan")
        check(Color.White.toString() == "White")
        check(Color.Indexed(10u).toString() == "10")
        check(Color.Rgb(255u, 0u, 0u).toString() == "#FF0000")
        check(Color.Reset.toString() == "Reset")
    }

    // -------------------------------------------------------------------------
    // Array and tuple conversion tests
    // -------------------------------------------------------------------------

    fun testFromArrayAndTupleConversions() {
        // From Triple (Kotlin equivalent of tuple)
        val fromTriple = Triple(123.toUByte(), 45.toUByte(), 67.toUByte()).toColor()
        check(fromTriple == Color.Rgb(123u, 45u, 67u))

        // From list/array (takes first 3 elements)
        val fromList = listOf(89.toUByte(), 76.toUByte(), 54.toUByte()).toColor()
        check(fromList == Color.Rgb(89u, 76u, 54u))

        // From list with 4 elements (alpha is ignored, takes first 3)
        val fromList4 = listOf(10.toUByte(), 20.toUByte(), 30.toUByte(), 255.toUByte()).toColor()
        check(fromList4 == Color.Rgb(10u, 20u, 30u))
    }

    // -------------------------------------------------------------------------
    // Run all tests
    // -------------------------------------------------------------------------

    fun runAll() {
        testFromU32()
        testFromRgbColor()
        testFromIndexedColor()
        testFromAnsiColor()
        testFromInvalidColors()
        testDisplay()
        testFromArrayAndTupleConversions()
        println("All Color tests passed!")
    }
}

// Extension function to convert a list of UBytes to Color (takes first 3)
fun List<UByte>.toColor(): Color {
    require(size >= 3) { "List must have at least 3 elements" }
    return Color.Rgb(this[0], this[1], this[2])
}
