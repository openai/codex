package ratatui.buffer

import ratatui.layout.Position
import ratatui.layout.Rect
import ratatui.style.Style
import ratatui.text.Line
import ratatui.text.Span

/**
 * A buffer cell.
 *
 * Each cell in the buffer contains a symbol (grapheme cluster), foreground color,
 * background color, and text modifiers.
 *
 * This is a minimal stub implementation. The full implementation will be ported
 * from the Rust ratatui-core crate.
 */
data class Cell(
    /** The string to be drawn in the cell */
    var symbol: String = " ",
    /** The foreground color of the cell */
    var fg: ratatui.style.Color = ratatui.style.Color.Reset,
    /** The background color of the cell */
    var bg: ratatui.style.Color = ratatui.style.Color.Reset,
    /** The modifier of the cell */
    var modifier: ratatui.style.Modifier = ratatui.style.Modifier.empty(),
    /** Whether the cell should be skipped when diffing */
    var skip: Boolean = false
) {
    companion object {
        /** An empty Cell */
        val EMPTY: Cell = Cell()

        /** Creates a new Cell with the given symbol */
        fun new(symbol: String): Cell = Cell(symbol = symbol)
    }

    /** Gets the symbol of the cell */
    fun symbol(): String = symbol

    /** Sets the symbol of the cell */
    fun setSymbol(symbol: String): Cell {
        this.symbol = symbol
        return this
    }

    /** Appends a symbol to the cell (for zero-width characters) */
    fun appendSymbol(symbol: String): Cell {
        this.symbol += symbol
        return this
    }

    /** Sets the style of the cell */
    fun setStyle(style: Style): Cell {
        style.fg?.let { this.fg = it }
        style.bg?.let { this.bg = it }
        // TODO: handle modifiers
        return this
    }

    /** Resets the cell to empty state */
    fun reset() {
        symbol = " "
        fg = ratatui.style.Color.Reset
        bg = ratatui.style.Color.Reset
        modifier = ratatui.style.Modifier.empty()
        skip = false
    }
}

/**
 * A buffer that maps to the desired content of the terminal after the draw call.
 *
 * No widget in the library interacts directly with the terminal. Instead each of them
 * is required to draw their state to an intermediate buffer. It is basically a grid
 * where each cell contains a grapheme, a foreground color and a background color.
 *
 * This is a minimal stub implementation. The full implementation will be ported
 * from the Rust ratatui-core crate.
 */
class Buffer(
    /** The area represented by this buffer */
    var area: Rect,
    /** The content of the buffer */
    val content: MutableList<Cell>
) {
    companion object {
        /** Returns a Buffer with all cells set to the default one */
        fun empty(area: Rect): Buffer {
            val size = area.area().toInt()
            val content = MutableList(size) { Cell.EMPTY.copy() }
            return Buffer(area, content)
        }

        /** Returns a Buffer with all cells initialized with the given Cell */
        fun filled(area: Rect, cell: Cell): Buffer {
            val size = area.area().toInt()
            val content = MutableList(size) { cell.copy() }
            return Buffer(area, content)
        }

        /** Returns a Buffer containing the given lines */
        fun withLines(lines: List<Line>): Buffer {
            val height = lines.size.toUShort()
            val width = lines.maxOfOrNull { it.width() }?.toUShort() ?: 0u
            val buffer = empty(Rect.new(0u, 0u, width, height))
            for ((y, line) in lines.withIndex()) {
                buffer.setLine(0u, y.toUShort(), line, width)
            }
            return buffer
        }

        /** Returns a Buffer containing the given string lines */
        fun withLines(vararg lines: String): Buffer {
            return withLines(lines.map { Line.from(it) })
        }
    }

    /** Returns the content of the buffer as a list */
    fun content(): List<Cell> = content

    /** Returns the area covered by this buffer */
    fun area(): Rect = area

    /** Returns the index in the content list for the given coordinates */
    fun indexOf(x: UShort, y: UShort): Int {
        require(area.contains(Position(x, y))) {
            "index outside of buffer: the area is $area but index is ($x, $y)"
        }
        val relY = (y - area.y).toInt()
        val relX = (x - area.x).toInt()
        val width = area.width.toInt()
        return relY * width + relX
    }

    /** Returns the cell at the given position, or null if outside bounds */
    fun cell(position: Position): Cell? {
        if (!area.contains(position)) return null
        val index = indexOf(position.x, position.y)
        return content.getOrNull(index)
    }

    /** Returns the mutable cell at the given position, or null if outside bounds */
    fun cellMut(position: Position): Cell? = cell(position)

    /** Indexing operator for (x, y) pairs */
    operator fun get(x: UShort, y: UShort): Cell {
        val index = indexOf(x, y)
        return content[index]
    }

    /** Indexing operator for Position */
    operator fun get(position: Position): Cell = get(position.x, position.y)

    /** Print a string, starting at the position (x, y) */
    fun setString(x: UShort, y: UShort, string: String, style: Style) {
        setStringn(x, y, string, Int.MAX_VALUE, style)
    }

    /** Print at most the first n characters of a string */
    fun setStringn(x: UShort, y: UShort, string: String, maxWidth: Int, style: Style): Pair<UShort, UShort> {
        var currentX = x
        val right = area.right()
        val remainingWidth = (right - x).toInt().coerceAtMost(maxWidth)

        var used = 0
        for (char in string) {
            if (used >= remainingWidth) break
            if (char.isISOControl()) continue

            val index = indexOf(currentX, y)
            content[index].setSymbol(char.toString()).setStyle(style)
            currentX = (currentX + 1u).toUShort()
            used++
        }
        return Pair(currentX, y)
    }

    /** Print a line, starting at the position (x, y) */
    fun setLine(x: UShort, y: UShort, line: Line, maxWidth: UShort): Pair<UShort, UShort> {
        var remainingWidth = maxWidth.toInt()
        var currentX = x
        for (span in line) {
            if (remainingWidth == 0) break
            val (newX, _) = setStringn(
                currentX,
                y,
                span.content,
                remainingWidth,
                line.style.patch(span.style)
            )
            val w = (newX - currentX).toInt()
            currentX = newX
            remainingWidth = (remainingWidth - w).coerceAtLeast(0)
        }
        return Pair(currentX, y)
    }

    /** Print a span, starting at the position (x, y) */
    fun setSpan(x: UShort, y: UShort, span: Span, maxWidth: UShort): Pair<UShort, UShort> {
        return setStringn(x, y, span.content, maxWidth.toInt(), span.style)
    }

    /** Set the style of all cells in the given area */
    fun setStyle(area: Rect, style: Style) {
        val intersection = this.area.intersection(area)
        for (y in intersection.top().toInt()..<intersection.bottom().toInt()) {
            for (x in intersection.left().toInt()..<intersection.right().toInt()) {
                this[x.toUShort(), y.toUShort()].setStyle(style)
            }
        }
    }

    /** Resize the buffer */
    fun resize(area: Rect) {
        val length = area.area().toInt()
        if (content.size > length) {
            while (content.size > length) content.removeLast()
        } else {
            while (content.size < length) content.add(Cell.EMPTY.copy())
        }
        this.area = area
    }

    /** Reset all cells in the buffer */
    fun reset() {
        for (cell in content) {
            cell.reset()
        }
    }
}
