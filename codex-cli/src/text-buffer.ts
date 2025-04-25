/* eslint‑disable no-bitwise */

export type Direction =
  | "left"
  | "right"
  | "up"
  | "down"
  | "wordLeft"
  | "wordRight"
  | "home"
  | "end";

// Simple helper for word‑wise ops.
function isWordChar(ch: string | undefined): boolean {
  if (ch === undefined) {
    return false;
  }
  return !/[\s,.;!?]/.test(ch);
}

export interface Viewport {
  height: number;
  width: number;
}

function clamp(v: number, min: number, max: number): number {
  return v < min ? min : v > max ? max : v;
}

/*
 * -------------------------------------------------------------------------
 *  Unicode‑aware helpers (work at the code‑point level rather than UTF‑16
 *  code units so that surrogate‑pair emoji count as one "column".)
 * ---------------------------------------------------------------------- */

function toCodePoints(str: string): Array<string> {
  if (typeof Intl !== "undefined" && "Segmenter" in Intl) {
    const seg = new Intl.Segmenter();
    return [...seg.segment(str)].map((seg) => seg.segment);
  }
  // [...str] or Array.from both iterate by UTF‑32 code point, handling
  // surrogate pairs correctly.
  return Array.from(str);
}

function cpLen(str: string): number {
  return toCodePoints(str).length;
}

function cpSlice(str: string, start: number, end?: number): string {
  // Slice by code‑point indices and re‑join.
  const arr = toCodePoints(str).slice(start, end);
  return arr.join("");
}

/* -------------------------------------------------------------------------
 *  Debug helper – enable verbose logging by setting env var TEXTBUFFER_DEBUG=1
 * ---------------------------------------------------------------------- */

// Enable verbose logging only when requested via env var.
const DEBUG =
  process.env["TEXTBUFFER_DEBUG"] === "1" ||
  process.env["TEXTBUFFER_DEBUG"] === "true";

function dbg(...args: Array<unknown>): void {
  if (DEBUG) {
    // eslint-disable-next-line no-console
    console.log("[TextBuffer]", ...args);
  }
}

/* ────────────────────────────────────────────────────────────────────────── */

export default class TextBuffer {
  private lines: Array<string>;
  private cursorRow = 0;
  private cursorCol = 0;
  private scrollRow = 0;
  private scrollCol = 0;

  /**
   * When the user moves the caret vertically we try to keep their original
   * horizontal column even when passing through shorter lines.  We remember
   * that *preferred* column in this field while the user is still travelling
   * vertically.  Any explicit horizontal movement resets the preference.
   */
  private preferredCol: number | null = null;

  /* a single integer that bumps every time text changes */
  private version = 0;

  /* ------------------------------------------------------------------
   *  History & clipboard
   * ---------------------------------------------------------------- */
  private undoStack: Array<{ lines: Array<string>; row: number; col: number }> =
    [];
  private redoStack: Array<{ lines: Array<string>; row: number; col: number }> =
    [];
  private historyLimit = 100;

  private clipboard: string | null = null;

  constructor(text = "") {
    this.lines = text.split("\n");
    if (this.lines.length === 0) {
      this.lines = [""];
    }
  }

  /* =====================================================================
   *  External editor integration (git‑style $EDITOR workflow)
   * =================================================================== */

  /**
   * Opens the current buffer contents in the user’s preferred terminal text
   * editor ($VISUAL or $EDITOR, falling back to "vi").  The method blocks
   * until the editor exits, then reloads the file and replaces the in‑memory
   * buffer with whatever the user saved.
   *
   * The operation is treated as a single undoable edit – we snapshot the
   * previous state *once* before launching the editor so one `undo()` will
   * revert the entire change set.
   *
   * Note: We purposefully rely on the *synchronous* spawn API so that the
   * calling process genuinely waits for the editor to close before
   * continuing.  This mirrors Git’s behaviour and simplifies downstream
   * control‑flow (callers can simply `await` the Promise).
   */
  async openInExternalEditor(opts: { editor?: string } = {}): Promise<void> {
    // Deliberately use `require()` so that unit tests can stub the
    // respective modules with `vi.spyOn(require("node:child_process"), …)`.
    // Dynamic `import()` would circumvent those CommonJS stubs.
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const pathMod = require("node:path");
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const fs = require("node:fs");
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const os = require("node:os");
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const { spawnSync } = require("node:child_process");

    const editor =
      opts.editor ??
      process.env["VISUAL"] ??
      process.env["EDITOR"] ??
      (process.platform === "win32" ? "notepad" : "vi");

    // Prepare a temporary file with the current contents.  We use mkdtempSync
    // to obtain an isolated directory and avoid name collisions.
    const tmpDir = fs.mkdtempSync(pathMod.join(os.tmpdir(), "codex-edit-"));
    const filePath = pathMod.join(tmpDir, "buffer.txt");

    fs.writeFileSync(filePath, this.getText(), "utf8");

    // One snapshot for undo semantics *before* we mutate anything.
    this.pushUndo();

    // The child inherits stdio so the user can interact with the editor as if
    // they had launched it directly.
    const { status, error } = spawnSync(editor, [filePath], {
      stdio: "inherit",
    });

    if (error) {
      throw error;
    }
    if (typeof status === "number" && status !== 0) {
      throw new Error(`External editor exited with status ${status}`);
    }

    // Read the edited contents back in – normalise line endings to \n.
    let newText = fs.readFileSync(filePath, "utf8");
    newText = newText.replace(/\r\n?/g, "\n");

    // Update buffer.
    this.lines = newText.split("\n");
    if (this.lines.length === 0) {
      this.lines = [""];
    }

    // Position the caret at EOF.
    this.cursorRow = this.lines.length - 1;
    this.cursorCol = cpLen(this.line(this.cursorRow));

    // Reset scroll offsets so the new end is visible.
    this.scrollRow = Math.max(0, this.cursorRow - 1);
    this.scrollCol = 0;

    this.version++;
  }

  /* =======================================================================
   *  Geometry helpers
   * ===================================================================== */
  private line(r: number): string {
    return this.lines[r] ?? "";
  }
  private lineLen(r: number): number {
    return cpLen(this.line(r));
  }

  private ensureCursorInRange(): void {
    this.cursorRow = clamp(this.cursorRow, 0, this.lines.length - 1);
    this.cursorCol = clamp(this.cursorCol, 0, this.lineLen(this.cursorRow));
  }

  /* =====================================================================
   *  History helpers
   * =================================================================== */
  private snapshot() {
    return {
      lines: this.lines.slice(),
      row: this.cursorRow,
      col: this.cursorCol,
    };
  }

  private pushUndo() {
    dbg("pushUndo", { cursor: this.getCursor(), text: this.getText() });
    this.undoStack.push(this.snapshot());
    if (this.undoStack.length > this.historyLimit) {
      this.undoStack.shift();
    }
    // once we mutate we clear redo
    this.redoStack.length = 0;
  }

  /**
   * Restore a snapshot and return true if restoration happened.
   */
  private restore(
    state: { lines: Array<string>; row: number; col: number } | undefined,
  ): boolean {
    if (!state) {
      return false;
    }
    this.lines = state.lines.slice();
    this.cursorRow = state.row;
    this.cursorCol = state.col;
    this.ensureCursorInRange();
    return true;
  }

  /* =======================================================================
   *  Scrolling helpers
   * ===================================================================== */
  private ensureCursorVisible(vp: Viewport) {
    const { height, width } = vp;

    if (this.cursorRow < this.scrollRow) {
      this.scrollRow = this.cursorRow;
    } else if (this.cursorRow >= this.scrollRow + height) {
      this.scrollRow = this.cursorRow - height + 1;
    }

    if (this.cursorCol < this.scrollCol) {
      this.scrollCol = this.cursorCol;
    } else if (this.cursorCol >= this.scrollCol + width) {
      this.scrollCol = this.cursorCol - width + 1;
    }
  }

  /* =======================================================================
   *  Public read‑only accessors
   * ===================================================================== */
  getVersion(): number {
    return this.version;
  }
  getCursor(): [number, number] {
    return [this.cursorRow, this.cursorCol];
  }
  getVisibleLines(vp: Viewport): Array<string> {
    // Whenever the viewport dimensions change (e.g. on a terminal resize) we
    // need to re‑evaluate whether the current scroll offset still keeps the
    // caret visible.  Calling `ensureCursorVisible` here guarantees that mere
    // re‑renders – even when not triggered by user input – will adjust the
    // horizontal and vertical scroll positions so the cursor remains in view.
    this.ensureCursorVisible(vp);

    return this.lines.slice(this.scrollRow, this.scrollRow + vp.height);
  }
  getText(): string {
    return this.lines.join("\n");
  }
  getLines(): Array<string> {
    return this.lines.slice();
  }

  /* =====================================================================
   *  History public API – undo / redo
   * =================================================================== */
  undo(): boolean {
    const state = this.undoStack.pop();
    if (!state) {
      return false;
    }
    // push current to redo before restore
    this.redoStack.push(this.snapshot());
    this.restore(state);
    this.version++;
    return true;
  }

  redo(): boolean {
    const state = this.redoStack.pop();
    if (!state) {
      return false;
    }
    // push current to undo before restore
    this.undoStack.push(this.snapshot());
    this.restore(state);
    this.version++;
    return true;
  }

  /* =======================================================================
   *  Editing operations
   * ===================================================================== */
  /**
   * Insert a single character or string without newlines. If the string
   * contains a newline we delegate to insertStr so that line splitting
   * logic is shared.
   */
  insert(ch: string): void {
    // Handle pasted blocks that may contain newline sequences (\n, \r or
    // Windows‑style \r\n).  Delegate to `insertStr` so the splitting logic is
    // centralised.
    if (/[\n\r]/.test(ch)) {
      this.insertStr(ch);
      return;
    }

    dbg("insert", { ch, beforeCursor: this.getCursor() });

    this.pushUndo();

    const line = this.line(this.cursorRow);
    this.lines[this.cursorRow] =
      cpSlice(line, 0, this.cursorCol) + ch + cpSlice(line, this.cursorCol);
    this.cursorCol += ch.length;
    this.version++;

    dbg("insert:after", {
      cursor: this.getCursor(),
      line: this.line(this.cursorRow),
    });
  }

  newline(): void {
    dbg("newline", { beforeCursor: this.getCursor() });
    this.pushUndo();

    const l = this.line(this.cursorRow);
    const before = cpSlice(l, 0, this.cursorCol);
    const after = cpSlice(l, this.cursorCol);

    this.lines[this.cursorRow] = before;
    this.lines.splice(this.cursorRow + 1, 0, after);

    this.cursorRow += 1;
    this.cursorCol = 0;
    this.version++;

    dbg("newline:after", {
      cursor: this.getCursor(),
      lines: [this.line(this.cursorRow - 1), this.line(this.cursorRow)],
    });
  }

  backspace(): void {
    dbg("backspace", { beforeCursor: this.getCursor() });
    if (this.cursorCol === 0 && this.cursorRow === 0) {
      return;
    } // nothing to delete

    this.pushUndo();

    if (this.cursorCol > 0) {
      const line = this.line(this.cursorRow);
      this.lines[this.cursorRow] =
        cpSlice(line, 0, this.cursorCol - 1) + cpSlice(line, this.cursorCol);
      this.cursorCol--;
    } else if (this.cursorRow > 0) {
      // merge with previous
      const prev = this.line(this.cursorRow - 1);
      const cur = this.line(this.cursorRow);
      const newCol = cpLen(prev);
      this.lines[this.cursorRow - 1] = prev + cur;
      this.lines.splice(this.cursorRow, 1);
      this.cursorRow--;
      this.cursorCol = newCol;
    }
    this.version++;

    dbg("backspace:after", {
      cursor: this.getCursor(),
      line: this.line(this.cursorRow),
    });
  }

  del(): void {
    dbg("delete", { beforeCursor: this.getCursor() });
    const line = this.line(this.cursorRow);
    if (this.cursorCol < this.lineLen(this.cursorRow)) {
      this.pushUndo();
      this.lines[this.cursorRow] =
        cpSlice(line, 0, this.cursorCol) + cpSlice(line, this.cursorCol + 1);
    } else if (this.cursorRow < this.lines.length - 1) {
      this.pushUndo();
      const next = this.line(this.cursorRow + 1);
      this.lines[this.cursorRow] = line + next;
      this.lines.splice(this.cursorRow + 1, 1);
    }
    this.version++;

    dbg("delete:after", {
      cursor: this.getCursor(),
      line: this.line(this.cursorRow),
    });
  }

  /* ------------------------------------------------------------------
   *  Word‑wise deletion helpers – exposed publicly so tests (and future
   *  key‑bindings) can invoke them directly.
   * ---------------------------------------------------------------- */

  /** Delete the word to the *left* of the caret, mirroring common
   *  Ctrl/Alt+Backspace behaviour in editors & terminals.  Both the adjacent
   *  whitespace *and* the word characters immediately preceding the caret are
   *  removed.  If the caret is already at column‑0 this becomes a no-op. */
  deleteWordLeft(): void {
    dbg("deleteWordLeft", { beforeCursor: this.getCursor() });

    if (this.cursorCol === 0 && this.cursorRow === 0) {
      return;
    } // Nothing to delete

    // When at column‑0 but *not* on the first row we merge with the previous
    // line – matching the behaviour of `backspace` for uniform UX.
    if (this.cursorCol === 0) {
      this.backspace();
      return;
    }

    this.pushUndo();

    const line = this.line(this.cursorRow);
    const arr = toCodePoints(line);

    // If the cursor is just after a space (or several spaces), we only delete the separators
    // then, on the next call, the previous word. We should never delete the entire line.
    let start = this.cursorCol;
    let onlySpaces = true;
    for (let i = 0; i < start; i++) {
      if (isWordChar(arr[i])) {
        onlySpaces = false;
        break;
      }
    }

    // If the line contains only spaces up to the cursor, delete just one space
    if (onlySpaces && start > 0) {
      start--;
    } else {
      // Step 1 – skip over any separators sitting *immediately* to the left of the caret
      while (start > 0 && !isWordChar(arr[start - 1])) {
        start--;
      }
      // Step 2 – skip the word characters themselves
      while (start > 0 && isWordChar(arr[start - 1])) {
        start--;
      }
    }

    this.lines[this.cursorRow] =
      cpSlice(line, 0, start) + cpSlice(line, this.cursorCol);
    this.cursorCol = start;
    this.version++;

    dbg("deleteWordLeft:after", {
      cursor: this.getCursor(),
      line: this.line(this.cursorRow),
    });
  }

  /** Delete the word to the *right* of the caret, akin to many editors'
   *  Ctrl/Alt+Delete shortcut.  Removes any whitespace/punctuation that
   *  follows the caret and the next contiguous run of word characters. */
  deleteWordRight(): void {
    dbg("deleteWordRight", { beforeCursor: this.getCursor() });

    const line = this.line(this.cursorRow);
    const arr = toCodePoints(line);
    if (
      this.cursorCol >= arr.length &&
      this.cursorRow === this.lines.length - 1
    ) {
      return;
    } // nothing to delete

    // At end‑of‑line ➜ merge with next row (mirrors `del` behaviour).
    if (this.cursorCol >= arr.length) {
      this.del();
      return;
    }

    this.pushUndo();

    let end = this.cursorCol;

    // Skip separators *first* so that consecutive calls gradually chew
    // through whitespace then whole words.
    while (end < arr.length && !isWordChar(arr[end])) {
      end++;
    }

    // Skip the word characters.
    while (end < arr.length && isWordChar(arr[end])) {
      end++;
    }

    this.lines[this.cursorRow] =
      cpSlice(line, 0, this.cursorCol) + cpSlice(line, end);
    // caret stays in place
    this.version++;

    dbg("deleteWordRight:after", {
      cursor: this.getCursor(),
      line: this.line(this.cursorRow),
    });
  }

  move(dir: Direction): void {
    const before = this.getCursor();
    switch (dir) {
      case "left":
        this.preferredCol = null;
        if (this.cursorCol > 0) {
          this.cursorCol--;
        } else if (this.cursorRow > 0) {
          this.cursorRow--;
          this.cursorCol = this.lineLen(this.cursorRow);
        }
        break;
      case "right":
        this.preferredCol = null;
        if (this.cursorCol < this.lineLen(this.cursorRow)) {
          this.cursorCol++;
        } else if (this.cursorRow < this.lines.length - 1) {
          this.cursorRow++;
          this.cursorCol = 0;
        }
        break;
      case "up":
        if (this.cursorRow > 0) {
          if (this.preferredCol == null) {
            this.preferredCol = this.cursorCol;
          }
          this.cursorRow--;
          this.cursorCol = clamp(
            this.preferredCol,
            0,
            this.lineLen(this.cursorRow),
          );
        }
        break;
      case "down":
        if (this.cursorRow < this.lines.length - 1) {
          if (this.preferredCol == null) {
            this.preferredCol = this.cursorCol;
          }
          this.cursorRow++;
          this.cursorCol = clamp(
            this.preferredCol,
            0,
            this.lineLen(this.cursorRow),
          );
        }
        break;
      case "home":
        this.preferredCol = null;
        this.cursorCol = 0;
        break;
      case "end":
        this.preferredCol = null;
        this.cursorCol = this.lineLen(this.cursorRow);
        break;
      case "wordLeft": {
        this.preferredCol = null;
        const regex = /[\s,.;!?]+/g;
        const slice = cpSlice(
          this.line(this.cursorRow),
          0,
          this.cursorCol,
        ).replace(/[\s,.;!?]+$/, "");
        let lastIdx = 0;
        let m;
        while ((m = regex.exec(slice)) != null) {
          lastIdx = m.index;
        }
        const last = cpLen(slice.slice(0, lastIdx));
        this.cursorCol = last === 0 ? 0 : last + 1;
        break;
      }
      case "wordRight": {
        this.preferredCol = null;
        const regex = /[\s,.;!?]+/g;
        const l = this.line(this.cursorRow);
        let moved = false;
        let m;
        while ((m = regex.exec(l)) != null) {
          const cpIdx = cpLen(l.slice(0, m.index));
          if (cpIdx > this.cursorCol) {
            // We want to land *at the beginning* of the separator run so that a
            // subsequent move("right") behaves naturally.
            this.cursorCol = cpIdx;
            moved = true;
            break;
          }
        }
        if (!moved) {
          // No boundary to the right – jump to EOL.
          this.cursorCol = this.lineLen(this.cursorRow);
        }
        break;
      }
    }

    if (DEBUG) {
      dbg("move", { dir, before, after: this.getCursor() });
    }

    /*
     * If the user performed any movement other than a consecutive vertical
     * traversal we clear the preferred column so the next vertical run starts
     * afresh.  The cases that keep the preference already returned earlier.
     */
    if (dir !== "up" && dir !== "down") {
      this.preferredCol = null;
    }
  }

  /* =====================================================================
   *  Higher‑level helpers
   * =================================================================== */

  /**
   * Insert an arbitrary string, possibly containing internal newlines.
   * Returns true if the buffer was modified.
   */
  insertStr(str: string): boolean {
    dbg("insertStr", { str, beforeCursor: this.getCursor() });
    if (str === "") {
      return false;
    }

    // Normalise all newline conventions (\r, \n, \r\n) to a single '\n'.
    const normalised = str.replace(/\r\n/g, "\n").replace(/\r/g, "\n");

    // Fast path: resulted in single‑line string ➜ delegate back to insert
    if (!normalised.includes("\n")) {
      this.insert(normalised);
      return true;
    }

    this.pushUndo();

    const parts = normalised.split("\n");
    const before = cpSlice(this.line(this.cursorRow), 0, this.cursorCol);
    const after = cpSlice(this.line(this.cursorRow), this.cursorCol);

    // Replace current line with first part combined with before text
    this.lines[this.cursorRow] = before + parts[0];

    // Middle lines (if any) are inserted verbatim after current row
    if (parts.length > 2) {
      const middle = parts.slice(1, -1);
      this.lines.splice(this.cursorRow + 1, 0, ...middle);
    }

    // Smart handling of the *final* inserted part:
    //   • When the caret is mid‑line we preserve existing behaviour – merge
    //     the last part with the text to the **right** of the caret so that
    //     inserting in the middle of a line keeps the remainder on the same
    //     row (e.g. "he|llo" → paste "x\ny" ⇒ "he x", "y llo").
    //   • When the caret is at column‑0 we instead treat the current line as
    //     a *separate* row that follows the inserted block.  This mirrors
    //     common editor behaviour and avoids the unintuitive merge that led
    //     to "cd"+"ef" → "cdef" in the failing tests.

    // Append the last part combined with original after text as a new line
    const last = parts[parts.length - 1] + after;
    this.lines.splice(this.cursorRow + (parts.length - 1), 0, last);

    // Update cursor position to end of last inserted part (before 'after')
    this.cursorRow += parts.length - 1;
    // `parts` is guaranteed to have at least one element here because
    // `split("\n")` always returns an array with ≥1 entry.  Tell the
    // compiler so we can pass a plain `string` to `cpLen`.
    this.cursorCol = cpLen(parts[parts.length - 1]!);

    this.version++;
    return true;
  }

  /* =====================================================================
   *  Selection & clipboard helpers (minimal)
   * =================================================================== */

  private selectionAnchor: [number, number] | null = null;

  startSelection(): void {
    this.selectionAnchor = [this.cursorRow, this.cursorCol];
  }

  endSelection(): void {
    // no-op for now, kept for API symmetry
    // we rely on anchor + current cursor to compute selection
  }

  /** Extract selected text. Returns null if no valid selection. */
  private getSelectedText(): string | null {
    if (!this.selectionAnchor) {
      return null;
    }
    const [ar, ac] = this.selectionAnchor;
    const [br, bc] = [this.cursorRow, this.cursorCol];

    // Determine ordering
    if (ar === br && ac === bc) {
      return null;
    } // empty selection

    const topBefore = ar < br || (ar === br && ac < bc);
    const [sr, sc, er, ec] = topBefore ? [ar, ac, br, bc] : [br, bc, ar, ac];

    if (sr === er) {
      return cpSlice(this.line(sr), sc, ec);
    }

    const parts: Array<string> = [];
    parts.push(cpSlice(this.line(sr), sc));
    for (let r = sr + 1; r < er; r++) {
      parts.push(this.line(r));
    }
    parts.push(cpSlice(this.line(er), 0, ec));
    return parts.join("\n");
  }

  copy(): string | null {
    const txt = this.getSelectedText();
    if (txt == null) {
      return null;
    }
    this.clipboard = txt;
    return txt;
  }

  paste(): boolean {
    if (this.clipboard == null) {
      return false;
    }
    return this.insertStr(this.clipboard);
  }

  /* =======================================================================
   *  High level "handleInput" – receives what Ink gives us
   *  Returns true when buffer mutated (=> re‑render)
   * ===================================================================== */
  handleInput(
    input: string | undefined,
    key: Record<string, boolean>,
    vp: Viewport,
  ): boolean {
    if (DEBUG) {
      dbg("handleInput", { input, key, cursor: this.getCursor() });
    }
    const beforeVer = this.version;
    const [beforeRow, beforeCol] = this.getCursor();

    if (key["escape"]) {
      return false;
    }

    /* new line — Ink sets either `key.return` *or* passes a literal "\n" */
    if (key["return"] || input === "\r" || input === "\n") {
      this.newline();
    } else if (
      key["leftArrow"] &&
      !key["meta"] &&
      !key["ctrl"] &&
      !key["alt"]
    ) {
      /* navigation */
      this.move("left");
    } else if (
      key["rightArrow"] &&
      !key["meta"] &&
      !key["ctrl"] &&
      !key["alt"]
    ) {
      this.move("right");
    } else if (key["upArrow"]) {
      this.move("up");
    } else if (key["downArrow"]) {
      this.move("down");
    } else if ((key["meta"] || key["ctrl"] || key["alt"]) && key["leftArrow"]) {
      this.move("wordLeft");
    } else if (
      (key["meta"] || key["ctrl"] || key["alt"]) &&
      key["rightArrow"]
    ) {
      this.move("wordRight");
    } else if (key["home"]) {
      this.move("home");
    } else if (key["end"]) {
      this.move("end");
    }
    /* delete */
    // In raw terminal mode many frameworks (Ink included) surface a physical
    // Backspace key‑press as the single DEL (0x7f) byte placed in `input` with
    // no `key.backspace` flag set.  Treat that byte exactly like an ordinary
    // Backspace for parity with textarea.rs and to make interactive tests
    // feedable through the simpler `(ch, {}, vp)` path.
    else if (
      (key["meta"] || key["ctrl"] || key["alt"]) &&
      (key["backspace"] || input === "\x7f")
    ) {
      this.deleteWordLeft();
    } else if ((key["meta"] || key["ctrl"] || key["alt"]) && key["delete"]) {
      this.deleteWordRight();
    } else if (
      key["backspace"] ||
      input === "\x7f" ||
      (key["delete"] && !key["shift"])
    ) {
      // Treat un‑modified "delete" (the common Mac backspace key) as a
      // standard backspace.  Holding Shift+Delete continues to perform a
      // forward deletion so we don't lose that capability on keyboards that
      // expose both behaviours.
      this.backspace();
    }
    // Forward deletion (Fn+Delete on macOS, or Delete key with Shift held after
    // the branch above) – remove the character *under / to the right* of the
    // caret, merging lines when at EOL similar to many editors.
    else if (key["delete"]) {
      this.del();
    } else if (input && !key["ctrl"] && !key["meta"]) {
      this.insert(input);
    }

    /* printable */

    /* clamp + scroll */
    this.ensureCursorInRange();
    this.ensureCursorVisible(vp);

    const cursorMoved =
      this.cursorRow !== beforeRow || this.cursorCol !== beforeCol;

    if (DEBUG) {
      dbg("handleInput:after", {
        cursor: this.getCursor(),
        text: this.getText(),
      });
    }
    return this.version !== beforeVer || cursorMoved;
  }
}
