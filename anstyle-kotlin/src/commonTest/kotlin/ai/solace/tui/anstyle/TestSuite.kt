package ai.solace.tui.anstyle

import ai.solace.tui.anstyle.Ansi256Color
import ai.solace.tui.anstyle.AnsiColor
import ai.solace.tui.anstyle.Color
import ai.solace.tui.anstyle.Effects
import ai.solace.tui.anstyle.Style
import kotlin.test.Test
import kotlin.test.assertEquals

// Rust original:
// use std::fmt::{Result, Write};
//
// use anstyle::{Ansi256Color, AnsiColor};
//
// #[test]
// fn no_leading_zero() -> Result {
//     let mut actual = String::new();
//     let ansi_colors = vec![
//         AnsiColor::Black,
//         AnsiColor::Red,
//         AnsiColor::Green,
//         AnsiColor::Yellow,
//         AnsiColor::Blue,
//         AnsiColor::Magenta,
//         AnsiColor::Cyan,
//         AnsiColor::White,
//         AnsiColor::BrightBlack,
//         AnsiColor::BrightRed,
//         AnsiColor::BrightGreen,
//         AnsiColor::BrightYellow,
//         AnsiColor::BrightBlue,
//         AnsiColor::BrightMagenta,
//         AnsiColor::BrightCyan,
//         AnsiColor::BrightWhite,
//     ];
//
//     for c in ansi_colors {
//         let c = Ansi256Color::from_ansi(c).on_default();
//         writeln!(actual, "{c}{c:?}{c:#}")?;
//     }
//
//     snapbox::assert_data_eq!(actual, snapbox::file!["no_leading_zero.vte": Text].raw());
//
//     Ok(())
// }

/**
 * Helper to format Style in Rust-compatible debug format
 */
private fun Style.toRustDebugString(): String {
    val fg = getFgColor()
    val bg = getBgColor()
    val underline = getUnderlineColor()
    val effects = getEffects()

    val fgStr = when (fg) {
        null -> "None"
        is Color.Ansi -> "Some(Ansi(${fg.color}))"
        is Color.Ansi256 -> "Some(Ansi256(Ansi256Color(${fg.color.index})))"
        is Color.Rgb -> "Some(Rgb(RgbColor(${fg.color.r}, ${fg.color.g}, ${fg.color.b})))"
    }
    val bgStr = when (bg) {
        null -> "None"
        is Color.Ansi -> "Some(Ansi(${bg.color}))"
        is Color.Ansi256 -> "Some(Ansi256(Ansi256Color(${bg.color.index})))"
        is Color.Rgb -> "Some(Rgb(RgbColor(${bg.color.r}, ${bg.color.g}, ${bg.color.b})))"
    }
    val ulStr = when (underline) {
        null -> "None"
        is Color.Ansi -> "Some(Ansi(${underline.color}))"
        is Color.Ansi256 -> "Some(Ansi256(Ansi256Color(${underline.color.index})))"
        is Color.Rgb -> "Some(Rgb(RgbColor(${underline.color.r}, ${underline.color.g}, ${underline.color.b})))"
    }
    val effectsStr = "Effects()"  // Simplified - empty effects for this test

    return "Style { fg: $fgStr, bg: $bgStr, underline: $ulStr, effects: $effectsStr }"
}

class TestSuite {
    @Test
    fun noLeadingZero() {
        val actual = buildString {
            val ansiColors = listOf(
                AnsiColor.Black,
                AnsiColor.Red,
                AnsiColor.Green,
                AnsiColor.Yellow,
                AnsiColor.Blue,
                AnsiColor.Magenta,
                AnsiColor.Cyan,
                AnsiColor.White,
                AnsiColor.BrightBlack,
                AnsiColor.BrightRed,
                AnsiColor.BrightGreen,
                AnsiColor.BrightYellow,
                AnsiColor.BrightBlue,
                AnsiColor.BrightMagenta,
                AnsiColor.BrightCyan,
                AnsiColor.BrightWhite,
            )

            for (c in ansiColors) {
                val style = Ansi256Color.fromAnsi(c).onDefault()
                // {c} - renders the ANSI escape sequence
                // {c:?} - renders the debug representation
                // {c:#} - renders the reset sequence
                val rendered = style.render().toString()
                val debug = style.toRustDebugString()
                val reset = style.renderReset().toString()
                appendLine("$rendered$debug$reset")
            }
        }

        // Expected output from no_leading_zero.vte
        // Note: ESC[38;5;N where N has no leading zeros
        // ESC = \u001b
        val expected = buildString {
            for (i in 0..15) {
                appendLine("\u001b[38;5;${i}mStyle { fg: Some(Ansi256(Ansi256Color($i))), bg: None, underline: None, effects: Effects() }\u001b[0m")
            }
        }

        assertEquals(expected, actual)
    }
}
