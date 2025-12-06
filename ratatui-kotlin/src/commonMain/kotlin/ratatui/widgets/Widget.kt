package ratatui.widgets

import ratatui.buffer.Buffer
import ratatui.layout.Rect

/**
 * A Widget is a type that can be rendered into a [Buffer] within a given [Rect] area.
 *
 * Widgets are the main building blocks of a Ratatui application. They are responsible for
 * rendering themselves into a buffer, which is then displayed on the terminal.
 *
 * The simplest approach is to implement the [render] method directly. The [render] method
 * takes the area to render into and the buffer to render to.
 *
 * # Examples
 *
 * ```kotlin
 * class Greeting(private val name: String) : Widget {
 *     override fun render(area: Rect, buf: Buffer) {
 *         val greeting = "Hello $name!"
 *         buf.setString(area.x, area.y, greeting, Style.default())
 *     }
 * }
 * ```
 *
 * This is a minimal stub interface. The full implementation will be ported from the
 * Rust ratatui-core crate.
 */
interface Widget {
    /**
     * Renders the widget into the given buffer.
     *
     * @param area The area to render into
     * @param buf The buffer to render to
     */
    fun render(area: Rect, buf: Buffer)
}

/**
 * A StatefulWidget is a Widget that can be rendered with state.
 *
 * This allows widgets to maintain state between render calls, which is useful for
 * widgets like lists that need to track selection state.
 */
interface StatefulWidget<State> {
    /**
     * Renders the widget into the given buffer with the given state.
     *
     * @param area The area to render into
     * @param buf The buffer to render to
     * @param state The mutable state of the widget
     */
    fun render(area: Rect, buf: Buffer, state: State)
}
