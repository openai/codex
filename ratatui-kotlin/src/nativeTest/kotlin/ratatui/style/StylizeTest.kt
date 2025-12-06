package ratatui.style

import ratatui.text.Span
import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * Tests for the Stylize extension functions.
 *
 * These tests validate that:
 * - String types implement styling methods correctly
 * - Span types can be chained with styling methods
 * - Primitive types can be styled
 * - Colors and modifiers apply correctly
 * - Method chaining works as expected
 * - ColorDebug produces correct debug output
 */
class StylizeTest {

    /**
     * Tests that string slices properly implement the Stylize extension functions.
     */
    @Test
    fun strStyled() {
        val s = "hello"

        // Test foreground colors
        assertEquals(Span.styled(s, Style.default().fg(Color.Black)), s.black())
        assertEquals(Span.styled(s, Style.default().fg(Color.Red)), s.red())
        assertEquals(Span.styled(s, Style.default().fg(Color.Green)), s.green())
        assertEquals(Span.styled(s, Style.default().fg(Color.Yellow)), s.yellow())
        assertEquals(Span.styled(s, Style.default().fg(Color.Blue)), s.blue())
        assertEquals(Span.styled(s, Style.default().fg(Color.Magenta)), s.magenta())
        assertEquals(Span.styled(s, Style.default().fg(Color.Cyan)), s.cyan())
        assertEquals(Span.styled(s, Style.default().fg(Color.Gray)), s.gray())
        assertEquals(Span.styled(s, Style.default().fg(Color.DarkGray)), s.darkGray())
        assertEquals(Span.styled(s, Style.default().fg(Color.LightRed)), s.lightRed())
        assertEquals(Span.styled(s, Style.default().fg(Color.LightGreen)), s.lightGreen())
        assertEquals(Span.styled(s, Style.default().fg(Color.LightYellow)), s.lightYellow())
        assertEquals(Span.styled(s, Style.default().fg(Color.LightBlue)), s.lightBlue())
        assertEquals(Span.styled(s, Style.default().fg(Color.LightMagenta)), s.lightMagenta())
        assertEquals(Span.styled(s, Style.default().fg(Color.LightCyan)), s.lightCyan())
        assertEquals(Span.styled(s, Style.default().fg(Color.White)), s.white())

        // Test background colors
        assertEquals(Span.styled(s, Style.default().bg(Color.Black)), s.onBlack())
        assertEquals(Span.styled(s, Style.default().bg(Color.Red)), s.onRed())
        assertEquals(Span.styled(s, Style.default().bg(Color.Green)), s.onGreen())
        assertEquals(Span.styled(s, Style.default().bg(Color.Yellow)), s.onYellow())
        assertEquals(Span.styled(s, Style.default().bg(Color.Blue)), s.onBlue())
        assertEquals(Span.styled(s, Style.default().bg(Color.Magenta)), s.onMagenta())
        assertEquals(Span.styled(s, Style.default().bg(Color.Cyan)), s.onCyan())
        assertEquals(Span.styled(s, Style.default().bg(Color.Gray)), s.onGray())
        assertEquals(Span.styled(s, Style.default().bg(Color.DarkGray)), s.onDarkGray())
        assertEquals(Span.styled(s, Style.default().bg(Color.LightRed)), s.onLightRed())
        assertEquals(Span.styled(s, Style.default().bg(Color.LightGreen)), s.onLightGreen())
        assertEquals(Span.styled(s, Style.default().bg(Color.LightYellow)), s.onLightYellow())
        assertEquals(Span.styled(s, Style.default().bg(Color.LightBlue)), s.onLightBlue())
        assertEquals(Span.styled(s, Style.default().bg(Color.LightMagenta)), s.onLightMagenta())
        assertEquals(Span.styled(s, Style.default().bg(Color.LightCyan)), s.onLightCyan())
        assertEquals(Span.styled(s, Style.default().bg(Color.White)), s.onWhite())

        // Test modifiers
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.BOLD)), s.bold())
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.DIM)), s.dim())
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.ITALIC)), s.italic())
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.UNDERLINED)), s.underlined())
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.SLOW_BLINK)), s.slowBlink())
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.RAPID_BLINK)), s.rapidBlink())
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.REVERSED)), s.reversed())
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.HIDDEN)), s.hidden())
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.CROSSED_OUT)), s.crossedOut())

        // Test not-modifiers
        assertEquals(Span.styled(s, Style.default().removeModifier(Modifier.BOLD)), s.notBold())
        assertEquals(Span.styled(s, Style.default().removeModifier(Modifier.DIM)), s.notDim())
        assertEquals(Span.styled(s, Style.default().removeModifier(Modifier.ITALIC)), s.notItalic())
        assertEquals(Span.styled(s, Style.default().removeModifier(Modifier.UNDERLINED)), s.notUnderlined())
        assertEquals(Span.styled(s, Style.default().removeModifier(Modifier.SLOW_BLINK)), s.notSlowBlink())
        assertEquals(Span.styled(s, Style.default().removeModifier(Modifier.RAPID_BLINK)), s.notRapidBlink())
        assertEquals(Span.styled(s, Style.default().removeModifier(Modifier.REVERSED)), s.notReversed())
        assertEquals(Span.styled(s, Style.default().removeModifier(Modifier.HIDDEN)), s.notHidden())
        assertEquals(Span.styled(s, Style.default().removeModifier(Modifier.CROSSED_OUT)), s.notCrossedOut())
    }

    /**
     * Tests that owned String types properly implement the Stylize extension functions.
     */
    @Test
    fun stringStyled() {
        val s: String = "hello"
        // Verify String styling produces same result as str styling
        assertEquals(Span.styled(s, Style.default().fg(Color.Red)), s.red())
        assertEquals(Span.styled(s, Style.default().bg(Color.Blue)), s.onBlue())
        assertEquals(Span.styled(s, Style.default().addModifier(Modifier.BOLD)), s.bold())
    }

    /**
     * Tests that temporary strings created via toString() can be styled.
     * This verifies that extension functions work with temporary values.
     */
    @Test
    fun temporaryStringStyled() {
        // Test styling on temporary strings
        assertEquals(
            Span.styled("hello", Style.default().fg(Color.Red)),
            "hello".red()
        )

        // Test with format-like behavior
        val name = "world"
        val greeting = "hello $name"
        assertEquals(
            Span.styled(greeting, Style.default().fg(Color.Green)),
            greeting.green()
        )
    }

    /**
     * Tests that primitive types can be styled.
     */
    @Test
    fun otherPrimitivesStyled() {
        // Boolean
        assertEquals(Span.styled("true", Style.default().fg(Color.Red)), true.red())
        assertEquals(Span.styled("false", Style.default().fg(Color.Green)), false.green())

        // Char
        assertEquals(Span.styled("a", Style.default().fg(Color.Red)), 'a'.red())

        // Int
        assertEquals(Span.styled("42", Style.default().fg(Color.Red)), 42.red())
        assertEquals(Span.styled("-1", Style.default().fg(Color.Blue)), (-1).blue())

        // Long
        assertEquals(Span.styled("9999999999", Style.default().fg(Color.Yellow)), 9999999999L.yellow())

        // Float
        assertEquals(Span.styled("3.14", Style.default().fg(Color.Cyan)), 3.14f.cyan())

        // Double
        assertEquals(Span.styled("2.718281828", Style.default().fg(Color.Magenta)), 2.718281828.magenta())
    }

    /**
     * Tests that styleReset() returns a span with Style.reset().
     */
    @Test
    fun reset() {
        val s = "hello"
        assertEquals(Span.styled(s, Style.reset()), s.styleReset())
    }

    /**
     * Tests foreground color application.
     */
    @Test
    fun fg() {
        val s = "hello"
        assertEquals(Span.styled(s, Style.default().fg(Color.Red)), s.fg(Color.Red))
    }

    /**
     * Tests background color application.
     */
    @Test
    fun bg() {
        val s = "hello"
        assertEquals(Span.styled(s, Style.default().bg(Color.Blue)), s.bg(Color.Blue))
    }

    /**
     * Tests combining foreground color with modifier.
     */
    @Test
    fun colorModifier() {
        val s = "hello"
        val expected = Span.styled(
            s,
            Style.default()
                .fg(Color.Red)
                .addModifier(Modifier.BOLD)
        )
        assertEquals(expected, s.red().bold())
    }

    /**
     * Tests simultaneous foreground and background color application.
     */
    @Test
    fun fgBg() {
        val s = "hello"
        val expected = Span.styled(
            s,
            Style.default()
                .fg(Color.Red)
                .bg(Color.Blue)
        )
        assertEquals(expected, s.red().onBlue())
    }

    /**
     * Tests that when attributes repeat, the final value takes precedence.
     */
    @Test
    fun repeatedAttributes() {
        val s = "hello"
        // Red is applied first, then green overwrites it
        val expected = Span.styled(s, Style.default().fg(Color.Green))
        assertEquals(expected, s.red().green())

        // Similarly for background
        val expectedBg = Span.styled(s, Style.default().bg(Color.Yellow))
        assertEquals(expectedBg, s.onBlue().onYellow())
    }

    /**
     * Tests comprehensive method chaining combining multiple colors and modifiers.
     */
    @Test
    fun allChained() {
        val s = "hello"
        val expected = Span.styled(
            s,
            Style.default()
                .fg(Color.Red)
                .bg(Color.Blue)
                .addModifier(Modifier.BOLD)
                .addModifier(Modifier.ITALIC)
                .addModifier(Modifier.UNDERLINED)
        )
        assertEquals(expected, s.red().onBlue().bold().italic().underlined())
    }

    /**
     * Tests that Span can be chained with additional styling methods.
     */
    @Test
    fun spanChaining() {
        val span = Span.raw("hello")
        val styled = span.red().onBlue().bold()

        val expected = Span.styled(
            "hello",
            Style.default()
                .fg(Color.Red)
                .bg(Color.Blue)
                .addModifier(Modifier.BOLD)
        )
        assertEquals(expected, styled)
    }

    /**
     * Tests Span extension methods for all colors and modifiers.
     */
    @Test
    fun spanExtensions() {
        val span = Span.raw("test")

        // Foreground colors
        assertEquals(
            Span.styled("test", Style.default().fg(Color.Red)),
            span.red()
        )
        assertEquals(
            Span.styled("test", Style.default().fg(Color.LightBlue)),
            span.lightBlue()
        )

        // Background colors
        assertEquals(
            Span.styled("test", Style.default().bg(Color.Green)),
            span.onGreen()
        )
        assertEquals(
            Span.styled("test", Style.default().bg(Color.LightYellow)),
            span.onLightYellow()
        )

        // Modifiers
        assertEquals(
            Span.styled("test", Style.default().addModifier(Modifier.BOLD)),
            span.bold()
        )
        assertEquals(
            Span.styled("test", Style.default().addModifier(Modifier.ITALIC)),
            span.italic()
        )
    }

    /**
     * Tests ColorDebug toString() for foreground colors.
     */
    @Test
    fun stylizeDebugForeground() {
        // Named colors use method-style format
        assertEquals(".black()", Color.Black.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".red()", Color.Red.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".green()", Color.Green.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".yellow()", Color.Yellow.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".blue()", Color.Blue.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".magenta()", Color.Magenta.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".cyan()", Color.Cyan.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".gray()", Color.Gray.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".dark_gray()", Color.DarkGray.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".light_red()", Color.LightRed.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".light_green()", Color.LightGreen.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".light_yellow()", Color.LightYellow.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".light_blue()", Color.LightBlue.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".light_magenta()", Color.LightMagenta.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".light_cyan()", Color.LightCyan.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".white()", Color.White.stylizeDebug(ColorDebugKind.Foreground).toString())

        // Special colors use fg() format
        assertEquals(".fg(Color.Reset)", Color.Reset.stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".fg(Color.Indexed(42))", Color.Indexed(42u).stylizeDebug(ColorDebugKind.Foreground).toString())
        assertEquals(".fg(Color.Rgb(255, 128, 0))", Color.Rgb(255u, 128u, 0u).stylizeDebug(ColorDebugKind.Foreground).toString())
    }

    /**
     * Tests ColorDebug toString() for background colors.
     */
    @Test
    fun stylizeDebugBackground() {
        // Named colors use on_method() format
        assertEquals(".on_black()", Color.Black.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_red()", Color.Red.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_green()", Color.Green.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_yellow()", Color.Yellow.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_blue()", Color.Blue.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_magenta()", Color.Magenta.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_cyan()", Color.Cyan.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_gray()", Color.Gray.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_dark_gray()", Color.DarkGray.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_light_red()", Color.LightRed.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_light_green()", Color.LightGreen.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_light_yellow()", Color.LightYellow.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_light_blue()", Color.LightBlue.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_light_magenta()", Color.LightMagenta.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_light_cyan()", Color.LightCyan.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".on_white()", Color.White.stylizeDebug(ColorDebugKind.Background).toString())

        // Special colors use bg() format
        assertEquals(".bg(Color.Reset)", Color.Reset.stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".bg(Color.Indexed(42))", Color.Indexed(42u).stylizeDebug(ColorDebugKind.Background).toString())
        assertEquals(".bg(Color.Rgb(255, 128, 0))", Color.Rgb(255u, 128u, 0u).stylizeDebug(ColorDebugKind.Background).toString())
    }

    /**
     * Tests ColorDebug toString() for underline colors.
     * Underline colors always use the .underlineColor() format.
     */
    @Test
    fun stylizeDebugUnderline() {
        assertEquals(".underlineColor(Color.Black)", Color.Black.stylizeDebug(ColorDebugKind.Underline).toString())
        assertEquals(".underlineColor(Color.Red)", Color.Red.stylizeDebug(ColorDebugKind.Underline).toString())
        assertEquals(".underlineColor(Color.Reset)", Color.Reset.stylizeDebug(ColorDebugKind.Underline).toString())
        assertEquals(".underlineColor(Color.Indexed(42))", Color.Indexed(42u).stylizeDebug(ColorDebugKind.Underline).toString())
        assertEquals(".underlineColor(Color.Rgb(255, 128, 0))", Color.Rgb(255u, 128u, 0u).stylizeDebug(ColorDebugKind.Underline).toString())
    }
}
