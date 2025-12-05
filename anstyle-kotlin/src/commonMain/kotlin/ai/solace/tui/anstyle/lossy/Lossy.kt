package ai.solace.tui.anstyle.lossy

import ai.solace.tui.anstyle.Ansi256Color
import ai.solace.tui.anstyle.AnsiColor
import ai.solace.tui.anstyle.Color
import ai.solace.tui.anstyle.RgbColor

/**
 * Lossy conversion between ANSI Color Codes.
 *
 * These functions allow converting between different color representations,
 * potentially losing precision in the process.
 */

/**
 * Lossily convert from any color to RGB.
 *
 * As the palette for 4-bit colors is terminal/user defined, a [Palette] must be
 * provided to match against.
 */
fun colorToRgb(color: Color, palette: Palette = Palette.DEFAULT): RgbColor {
    return when (color) {
        is Color.Ansi -> ansiToRgb(color.color, palette)
        is Color.Ansi256 -> xtermToRgb(color.color, palette)
        is Color.Rgb -> color.color
    }
}

/**
 * Lossily convert from any color to 256-color.
 *
 * As the palette for 4-bit colors is terminal/user defined, a [Palette] must be
 * provided to match against.
 */
fun colorToXterm(color: Color): Ansi256Color {
    return when (color) {
        is Color.Ansi -> Ansi256Color.fromAnsi(color.color)
        is Color.Ansi256 -> color.color
        is Color.Rgb -> rgbToXterm(color.color)
    }
}

/**
 * Lossily convert from any color to 4-bit color.
 *
 * As the palette for 4-bit colors is terminal/user defined, a [Palette] must be
 * provided to match against.
 */
fun colorToAnsi(color: Color, palette: Palette = Palette.DEFAULT): AnsiColor {
    return when (color) {
        is Color.Ansi -> color.color
        is Color.Ansi256 -> xtermToAnsi(color.color, palette)
        is Color.Rgb -> rgbToAnsi(color.color, palette)
    }
}

/**
 * Lossily convert from 4-bit color to RGB.
 *
 * As the palette for 4-bit colors is terminal/user defined, a [Palette] must be
 * provided to match against.
 */
fun ansiToRgb(color: AnsiColor, palette: Palette = Palette.DEFAULT): RgbColor {
    return palette.rgbFromAnsi(color)
}

/**
 * Lossily convert from 256-color to RGB.
 *
 * As 256-color palette is a superset of 4-bit colors and since the palette for 4-bit colors is
 * terminal/user defined, a [Palette] must be provided to match against.
 */
fun xtermToRgb(color: Ansi256Color, palette: Palette = Palette.DEFAULT): RgbColor {
    val rgb = palette.getByIndex(color.index)
    return rgb ?: XTERM_COLORS[color.index.toInt()]
}

/**
 * Lossily convert from the 256-color palette to 4-bit color.
 *
 * As the palette for 4-bit colors is terminal/user defined, a [Palette] must be
 * provided to match against.
 */
fun xtermToAnsi(color: Ansi256Color, palette: Palette = Palette.DEFAULT): AnsiColor {
    return when (val index = color.index.toInt()) {
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
        else -> {
            val rgb = XTERM_COLORS[index]
            palette.findMatch(rgb)
        }
    }
}

/**
 * Lossily convert an RGB value to a 4-bit color.
 *
 * As the palette for 4-bit colors is terminal/user defined, a [Palette] must be
 * provided to match against.
 */
fun rgbToAnsi(color: RgbColor, palette: Palette = Palette.DEFAULT): AnsiColor {
    return palette.findMatch(color)
}

/**
 * Lossily convert an RGB value to the 256-color palette.
 */
fun rgbToXterm(color: RgbColor): Ansi256Color {
    val index = findXtermMatch(color)
    return Ansi256Color(index.toUByte())
}

private fun findXtermMatch(color: RgbColor): Int {
    var bestIndex = 16
    var bestDistance = distance(color, XTERM_COLORS[bestIndex])

    for (index in (bestIndex + 1) until XTERM_COLORS.size) {
        val dist = distance(color, XTERM_COLORS[index])
        if (dist < bestDistance) {
            bestIndex = index
            bestDistance = dist
        }
    }

    return bestIndex
}

/**
 * Low-cost color distance approximation.
 *
 * Based on https://www.compuphase.com/cmetric.htm, modified to avoid sqrt.
 */
internal fun distance(c1: RgbColor, c2: RgbColor): UInt {
    val c1R = c1.r.toInt()
    val c1G = c1.g.toInt()
    val c1B = c1.b.toInt()
    val c2R = c2.r.toInt()
    val c2G = c2.g.toInt()
    val c2B = c2.b.toInt()

    val rSum = c1R + c2R
    val rDelta = c1R - c2R
    val gDelta = c1G - c2G
    val bDelta = c1B - c2B

    val r = (2 * 512 + rSum) * rDelta * rDelta
    val g = 4 * gDelta * gDelta * (1 shl 8)
    val b = (2 * 767 - rSum) * bDelta * bDelta

    return (r + g + b).toUInt()
}

/**
 * The standard 256-color xterm palette.
 *
 * Indices 0-15 are placeholders (use Palette for those).
 * Indices 16-231 are the 6x6x6 color cube.
 * Indices 232-255 are the grayscale ramp.
 */
@Suppress("ktlint:standard:max-line-length")
internal val XTERM_COLORS: Array<RgbColor> = arrayOf(
    // Placeholders for indices 0-15 (use Palette for these)
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 0u),
    // 6x6x6 cube (indices 16-231)
    RgbColor(0u, 0u, 0u),
    RgbColor(0u, 0u, 95u),
    RgbColor(0u, 0u, 135u),
    RgbColor(0u, 0u, 175u),
    RgbColor(0u, 0u, 215u),
    RgbColor(0u, 0u, 255u),
    RgbColor(0u, 95u, 0u),
    RgbColor(0u, 95u, 95u),
    RgbColor(0u, 95u, 135u),
    RgbColor(0u, 95u, 175u),
    RgbColor(0u, 95u, 215u),
    RgbColor(0u, 95u, 255u),
    RgbColor(0u, 135u, 0u),
    RgbColor(0u, 135u, 95u),
    RgbColor(0u, 135u, 135u),
    RgbColor(0u, 135u, 175u),
    RgbColor(0u, 135u, 215u),
    RgbColor(0u, 135u, 255u),
    RgbColor(0u, 175u, 0u),
    RgbColor(0u, 175u, 95u),
    RgbColor(0u, 175u, 135u),
    RgbColor(0u, 175u, 175u),
    RgbColor(0u, 175u, 215u),
    RgbColor(0u, 175u, 255u),
    RgbColor(0u, 215u, 0u),
    RgbColor(0u, 215u, 95u),
    RgbColor(0u, 215u, 135u),
    RgbColor(0u, 215u, 175u),
    RgbColor(0u, 215u, 215u),
    RgbColor(0u, 215u, 255u),
    RgbColor(0u, 255u, 0u),
    RgbColor(0u, 255u, 95u),
    RgbColor(0u, 255u, 135u),
    RgbColor(0u, 255u, 175u),
    RgbColor(0u, 255u, 215u),
    RgbColor(0u, 255u, 255u),
    RgbColor(95u, 0u, 0u),
    RgbColor(95u, 0u, 95u),
    RgbColor(95u, 0u, 135u),
    RgbColor(95u, 0u, 175u),
    RgbColor(95u, 0u, 215u),
    RgbColor(95u, 0u, 255u),
    RgbColor(95u, 95u, 0u),
    RgbColor(95u, 95u, 95u),
    RgbColor(95u, 95u, 135u),
    RgbColor(95u, 95u, 175u),
    RgbColor(95u, 95u, 215u),
    RgbColor(95u, 95u, 255u),
    RgbColor(95u, 135u, 0u),
    RgbColor(95u, 135u, 95u),
    RgbColor(95u, 135u, 135u),
    RgbColor(95u, 135u, 175u),
    RgbColor(95u, 135u, 215u),
    RgbColor(95u, 135u, 255u),
    RgbColor(95u, 175u, 0u),
    RgbColor(95u, 175u, 95u),
    RgbColor(95u, 175u, 135u),
    RgbColor(95u, 175u, 175u),
    RgbColor(95u, 175u, 215u),
    RgbColor(95u, 175u, 255u),
    RgbColor(95u, 215u, 0u),
    RgbColor(95u, 215u, 95u),
    RgbColor(95u, 215u, 135u),
    RgbColor(95u, 215u, 175u),
    RgbColor(95u, 215u, 215u),
    RgbColor(95u, 215u, 255u),
    RgbColor(95u, 255u, 0u),
    RgbColor(95u, 255u, 95u),
    RgbColor(95u, 255u, 135u),
    RgbColor(95u, 255u, 175u),
    RgbColor(95u, 255u, 215u),
    RgbColor(95u, 255u, 255u),
    RgbColor(135u, 0u, 0u),
    RgbColor(135u, 0u, 95u),
    RgbColor(135u, 0u, 135u),
    RgbColor(135u, 0u, 175u),
    RgbColor(135u, 0u, 215u),
    RgbColor(135u, 0u, 255u),
    RgbColor(135u, 95u, 0u),
    RgbColor(135u, 95u, 95u),
    RgbColor(135u, 95u, 135u),
    RgbColor(135u, 95u, 175u),
    RgbColor(135u, 95u, 215u),
    RgbColor(135u, 95u, 255u),
    RgbColor(135u, 135u, 0u),
    RgbColor(135u, 135u, 95u),
    RgbColor(135u, 135u, 135u),
    RgbColor(135u, 135u, 175u),
    RgbColor(135u, 135u, 215u),
    RgbColor(135u, 135u, 255u),
    RgbColor(135u, 175u, 0u),
    RgbColor(135u, 175u, 95u),
    RgbColor(135u, 175u, 135u),
    RgbColor(135u, 175u, 175u),
    RgbColor(135u, 175u, 215u),
    RgbColor(135u, 175u, 255u),
    RgbColor(135u, 215u, 0u),
    RgbColor(135u, 215u, 95u),
    RgbColor(135u, 215u, 135u),
    RgbColor(135u, 215u, 175u),
    RgbColor(135u, 215u, 215u),
    RgbColor(135u, 215u, 255u),
    RgbColor(135u, 255u, 0u),
    RgbColor(135u, 255u, 95u),
    RgbColor(135u, 255u, 135u),
    RgbColor(135u, 255u, 175u),
    RgbColor(135u, 255u, 215u),
    RgbColor(135u, 255u, 255u),
    RgbColor(175u, 0u, 0u),
    RgbColor(175u, 0u, 95u),
    RgbColor(175u, 0u, 135u),
    RgbColor(175u, 0u, 175u),
    RgbColor(175u, 0u, 215u),
    RgbColor(175u, 0u, 255u),
    RgbColor(175u, 95u, 0u),
    RgbColor(175u, 95u, 95u),
    RgbColor(175u, 95u, 135u),
    RgbColor(175u, 95u, 175u),
    RgbColor(175u, 95u, 215u),
    RgbColor(175u, 95u, 255u),
    RgbColor(175u, 135u, 0u),
    RgbColor(175u, 135u, 95u),
    RgbColor(175u, 135u, 135u),
    RgbColor(175u, 135u, 175u),
    RgbColor(175u, 135u, 215u),
    RgbColor(175u, 135u, 255u),
    RgbColor(175u, 175u, 0u),
    RgbColor(175u, 175u, 95u),
    RgbColor(175u, 175u, 135u),
    RgbColor(175u, 175u, 175u),
    RgbColor(175u, 175u, 215u),
    RgbColor(175u, 175u, 255u),
    RgbColor(175u, 215u, 0u),
    RgbColor(175u, 215u, 95u),
    RgbColor(175u, 215u, 135u),
    RgbColor(175u, 215u, 175u),
    RgbColor(175u, 215u, 215u),
    RgbColor(175u, 215u, 255u),
    RgbColor(175u, 255u, 0u),
    RgbColor(175u, 255u, 95u),
    RgbColor(175u, 255u, 135u),
    RgbColor(175u, 255u, 175u),
    RgbColor(175u, 255u, 215u),
    RgbColor(175u, 255u, 255u),
    RgbColor(215u, 0u, 0u),
    RgbColor(215u, 0u, 95u),
    RgbColor(215u, 0u, 135u),
    RgbColor(215u, 0u, 175u),
    RgbColor(215u, 0u, 215u),
    RgbColor(215u, 0u, 255u),
    RgbColor(215u, 95u, 0u),
    RgbColor(215u, 95u, 95u),
    RgbColor(215u, 95u, 135u),
    RgbColor(215u, 95u, 175u),
    RgbColor(215u, 95u, 215u),
    RgbColor(215u, 95u, 255u),
    RgbColor(215u, 135u, 0u),
    RgbColor(215u, 135u, 95u),
    RgbColor(215u, 135u, 135u),
    RgbColor(215u, 135u, 175u),
    RgbColor(215u, 135u, 215u),
    RgbColor(215u, 135u, 255u),
    RgbColor(215u, 175u, 0u),
    RgbColor(215u, 175u, 95u),
    RgbColor(215u, 175u, 135u),
    RgbColor(215u, 175u, 175u),
    RgbColor(215u, 175u, 215u),
    RgbColor(215u, 175u, 255u),
    RgbColor(215u, 215u, 0u),
    RgbColor(215u, 215u, 95u),
    RgbColor(215u, 215u, 135u),
    RgbColor(215u, 215u, 175u),
    RgbColor(215u, 215u, 215u),
    RgbColor(215u, 215u, 255u),
    RgbColor(215u, 255u, 0u),
    RgbColor(215u, 255u, 95u),
    RgbColor(215u, 255u, 135u),
    RgbColor(215u, 255u, 175u),
    RgbColor(215u, 255u, 215u),
    RgbColor(215u, 255u, 255u),
    RgbColor(255u, 0u, 0u),
    RgbColor(255u, 0u, 95u),
    RgbColor(255u, 0u, 135u),
    RgbColor(255u, 0u, 175u),
    RgbColor(255u, 0u, 215u),
    RgbColor(255u, 0u, 255u),
    RgbColor(255u, 95u, 0u),
    RgbColor(255u, 95u, 95u),
    RgbColor(255u, 95u, 135u),
    RgbColor(255u, 95u, 175u),
    RgbColor(255u, 95u, 215u),
    RgbColor(255u, 95u, 255u),
    RgbColor(255u, 135u, 0u),
    RgbColor(255u, 135u, 95u),
    RgbColor(255u, 135u, 135u),
    RgbColor(255u, 135u, 175u),
    RgbColor(255u, 135u, 215u),
    RgbColor(255u, 135u, 255u),
    RgbColor(255u, 175u, 0u),
    RgbColor(255u, 175u, 95u),
    RgbColor(255u, 175u, 135u),
    RgbColor(255u, 175u, 175u),
    RgbColor(255u, 175u, 215u),
    RgbColor(255u, 175u, 255u),
    RgbColor(255u, 215u, 0u),
    RgbColor(255u, 215u, 95u),
    RgbColor(255u, 215u, 135u),
    RgbColor(255u, 215u, 175u),
    RgbColor(255u, 215u, 215u),
    RgbColor(255u, 215u, 255u),
    RgbColor(255u, 255u, 0u),
    RgbColor(255u, 255u, 95u),
    RgbColor(255u, 255u, 135u),
    RgbColor(255u, 255u, 175u),
    RgbColor(255u, 255u, 215u),
    RgbColor(255u, 255u, 255u),
    // Grayscale ramp (indices 232-255)
    RgbColor(8u, 8u, 8u),
    RgbColor(18u, 18u, 18u),
    RgbColor(28u, 28u, 28u),
    RgbColor(38u, 38u, 38u),
    RgbColor(48u, 48u, 48u),
    RgbColor(58u, 58u, 58u),
    RgbColor(68u, 68u, 68u),
    RgbColor(78u, 78u, 78u),
    RgbColor(88u, 88u, 88u),
    RgbColor(98u, 98u, 98u),
    RgbColor(108u, 108u, 108u),
    RgbColor(118u, 118u, 118u),
    RgbColor(128u, 128u, 128u),
    RgbColor(138u, 138u, 138u),
    RgbColor(148u, 148u, 148u),
    RgbColor(158u, 158u, 158u),
    RgbColor(168u, 168u, 168u),
    RgbColor(178u, 178u, 178u),
    RgbColor(188u, 188u, 188u),
    RgbColor(198u, 198u, 198u),
    RgbColor(208u, 208u, 208u),
    RgbColor(218u, 218u, 218u),
    RgbColor(228u, 228u, 228u),
    RgbColor(238u, 238u, 238u),
)
