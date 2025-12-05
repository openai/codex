package anstyle.lossy

import anstyle.Ansi256Color
import anstyle.AnsiColor
import anstyle.RgbColor

/**
 * A color palette for rendering 4-bit [AnsiColor].
 *
 * Based on [wikipedia](https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit)
 */
data class Palette(val colors: Array<RgbColor>) {

    init {
        require(colors.size == 16) { "Palette must have exactly 16 colors" }
    }

    /**
     * Look up the [RgbColor] in the palette for the given [AnsiColor].
     */
    operator fun get(color: AnsiColor): RgbColor {
        val index = Ansi256Color.fromAnsi(color).index.toInt()
        return colors[index]
    }

    /**
     * Look up the [RgbColor] in the palette for the given 256-color index.
     */
    fun getByIndex(index: UByte): RgbColor? {
        val idx = index.toInt()
        return if (idx < colors.size) colors[idx] else null
    }

    /**
     * Convert an [AnsiColor] to its RGB representation using this palette.
     */
    fun rgbFromAnsi(color: AnsiColor): RgbColor = get(color)

    /**
     * Find the closest [AnsiColor] match for the given RGB color.
     */
    fun findMatch(color: RgbColor): AnsiColor {
        var bestIndex = 0
        var bestDistance = distance(color, colors[bestIndex])

        for (index in 1 until colors.size) {
            val dist = distance(color, colors[index])
            if (dist < bestDistance) {
                bestIndex = index
                bestDistance = dist
            }
        }

        return Ansi256Color(bestIndex.toUByte()).intoAnsi()
            ?: error("bestIndex $bestIndex is out of bounds for AnsiColor")
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is Palette) return false
        return colors.contentEquals(other.colors)
    }

    override fun hashCode(): Int = colors.contentHashCode()

    companion object {
        /**
         * Typical colors that are used when booting PCs and leaving them in text mode.
         */
        val VGA: Palette = Palette(
            arrayOf(
                RgbColor(0u, 0u, 0u),
                RgbColor(170u, 0u, 0u),
                RgbColor(0u, 170u, 0u),
                RgbColor(170u, 85u, 0u),
                RgbColor(0u, 0u, 170u),
                RgbColor(170u, 0u, 170u),
                RgbColor(0u, 170u, 170u),
                RgbColor(170u, 170u, 170u),
                RgbColor(85u, 85u, 85u),
                RgbColor(255u, 85u, 85u),
                RgbColor(85u, 255u, 85u),
                RgbColor(255u, 255u, 85u),
                RgbColor(85u, 85u, 255u),
                RgbColor(255u, 85u, 255u),
                RgbColor(85u, 255u, 255u),
                RgbColor(255u, 255u, 255u),
            )
        )

        /**
         * Campbell theme, used as of Windows 10 version 1709.
         */
        val WIN10_CONSOLE: Palette = Palette(
            arrayOf(
                RgbColor(12u, 12u, 12u),
                RgbColor(197u, 15u, 31u),
                RgbColor(19u, 161u, 14u),
                RgbColor(193u, 156u, 0u),
                RgbColor(0u, 55u, 218u),
                RgbColor(136u, 23u, 152u),
                RgbColor(58u, 150u, 221u),
                RgbColor(204u, 204u, 204u),
                RgbColor(118u, 118u, 118u),
                RgbColor(231u, 72u, 86u),
                RgbColor(22u, 198u, 12u),
                RgbColor(249u, 241u, 165u),
                RgbColor(59u, 120u, 255u),
                RgbColor(180u, 0u, 158u),
                RgbColor(97u, 214u, 214u),
                RgbColor(242u, 242u, 242u),
            )
        )

        /**
         * Platform-specific default palette.
         */
        val DEFAULT: Palette = VGA
    }
}
