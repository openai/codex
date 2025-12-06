/**
 * ANSI SGR (Select Graphic Rendition) code definitions.
 *
 * This module defines the [AnsiCode] sealed class hierarchy representing
 * the various ANSI escape codes for text styling.
 */
package ansitotui

import ratatui.style.Color

/**
 * ANSI SGR (Select Graphic Rendition) codes.
 *
 * This sealed class stores most types of ANSI escape sequences.
 *
 * You can convert an escape sequence code to an [AnsiCode] using [AnsiCode.from].
 * This doesn't support all codes but does support most of them.
 */
sealed class AnsiCode {
    /** Reset the terminal */
    data object Reset : AnsiCode()
    /** Set font to bold */
    data object Bold : AnsiCode()
    /** Set font to faint */
    data object Faint : AnsiCode()
    /** Set font to italic */
    data object Italic : AnsiCode()
    /** Set font to underline */
    data object Underline : AnsiCode()
    /** Set cursor to slow blink */
    data object SlowBlink : AnsiCode()
    /** Set cursor to rapid blink */
    data object RapidBlink : AnsiCode()
    /** Invert the colors */
    data object Reverse : AnsiCode()
    /** Conceal text */
    data object Conceal : AnsiCode()
    /** Display crossed out text */
    data object CrossedOut : AnsiCode()
    /** Choose primary font */
    data object PrimaryFont : AnsiCode()
    /** Choose alternate font */
    data object AlternateFont : AnsiCode()
    /** Choose alternate fonts 1-9 */
    data class AlternateFonts(val font: UByte) : AnsiCode()
    /** Fraktur */
    data object Fraktur : AnsiCode()
    /** Turn off bold */
    data object BoldOff : AnsiCode()
    /** Set text to normal */
    data object Normal : AnsiCode()
    /** Turn off italic */
    data object NotItalic : AnsiCode()
    /** Turn off underline */
    data object UnderlineOff : AnsiCode()
    /** Turn off blinking */
    data object BlinkOff : AnsiCode()
    /** Don't invert colors */
    data object InvertOff : AnsiCode()
    /** Reveal text */
    data object Reveal : AnsiCode()
    /** Turn off crossed out text */
    data object CrossedOutOff : AnsiCode()
    /** Set foreground color (4-bit) */
    data class ForegroundColor(val color: Color) : AnsiCode()
    /** Set foreground color (8-bit and 24-bit) */
    data object SetForegroundColor : AnsiCode()
    /** Default foreground color */
    data object DefaultForegroundColor : AnsiCode()
    /** Set background color (4-bit) */
    data class BackgroundColor(val color: Color) : AnsiCode()
    /** Set background color (8-bit and 24-bit) */
    data object SetBackgroundColor : AnsiCode()
    /** Default background color */
    data object DefaultBackgroundColor : AnsiCode()
    /** Other / non-supported escape codes */
    data class Code(val bytes: List<UByte>) : AnsiCode()

    companion object {
        /**
         * Convert a byte code to an [AnsiCode].
         */
        fun from(code: UByte): AnsiCode = when (code.toInt()) {
            0 -> Reset
            1 -> Bold
            2 -> Faint
            3 -> Italic
            4 -> Underline
            5 -> SlowBlink
            6 -> RapidBlink
            7 -> Reverse
            8 -> Conceal
            9 -> CrossedOut
            10 -> PrimaryFont
            11 -> AlternateFont
            // AlternateFonts = 12..19
            20 -> Fraktur
            21 -> BoldOff
            22 -> Normal
            23 -> NotItalic
            24 -> UnderlineOff
            25 -> BlinkOff
            // 26 ?
            27 -> InvertOff
            28 -> Reveal
            29 -> CrossedOutOff
            30 -> ForegroundColor(Color.Black)
            31 -> ForegroundColor(Color.Red)
            32 -> ForegroundColor(Color.Green)
            33 -> ForegroundColor(Color.Yellow)
            34 -> ForegroundColor(Color.Blue)
            35 -> ForegroundColor(Color.Magenta)
            36 -> ForegroundColor(Color.Cyan)
            37 -> ForegroundColor(Color.Gray)
            38 -> SetForegroundColor
            39 -> DefaultForegroundColor
            40 -> BackgroundColor(Color.Black)
            41 -> BackgroundColor(Color.Red)
            42 -> BackgroundColor(Color.Green)
            43 -> BackgroundColor(Color.Yellow)
            44 -> BackgroundColor(Color.Blue)
            45 -> BackgroundColor(Color.Magenta)
            46 -> BackgroundColor(Color.Cyan)
            47 -> BackgroundColor(Color.Gray)
            48 -> SetBackgroundColor
            49 -> DefaultBackgroundColor
            90 -> ForegroundColor(Color.DarkGray)
            91 -> ForegroundColor(Color.LightRed)
            92 -> ForegroundColor(Color.LightGreen)
            93 -> ForegroundColor(Color.LightYellow)
            94 -> ForegroundColor(Color.LightBlue)
            95 -> ForegroundColor(Color.LightMagenta)
            96 -> ForegroundColor(Color.LightCyan)
            97 -> ForegroundColor(Color.White)
            100 -> BackgroundColor(Color.DarkGray)
            101 -> BackgroundColor(Color.LightRed)
            102 -> BackgroundColor(Color.LightGreen)
            103 -> BackgroundColor(Color.LightYellow)
            104 -> BackgroundColor(Color.LightBlue)
            105 -> BackgroundColor(Color.LightMagenta)
            106 -> BackgroundColor(Color.LightCyan)
            107 -> ForegroundColor(Color.White)
            else -> Code(listOf(code))
        }
    }
}
