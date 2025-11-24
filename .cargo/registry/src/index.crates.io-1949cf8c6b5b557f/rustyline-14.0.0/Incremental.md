## Incremental computation

We would like to avoid redrawing all row(s) when an event occurs.
Currently, we redraw all row(s) except when:

- a character is inserted at the end of input (and there is no hint and no `highlight_char`),
- only the cursor is moved (input is not touched and no `highlight_char`).

Ideally, we would like to redraw only impacted row(s) / cell(s).

### Observable values

Currently, we assume that highlighting does not impact layout / rendered text size.
So only the following observables impact layout:

- prompt (interactive search, [input mode indicator](https://github.com/kkawakam/rustyline/pull/369)),
- [input mode](https://github.com/kkawakam/rustyline/pull/369),
- line(s) buffer,
- cursor position,
- hint / input validation message,
- screen size (line wrapping),
- [prompt continuation](https://github.com/kkawakam/rustyline/pull/372)s,
- row/wrap count.

Some other values may impact layout but they are/should be constant:

- tab stop,

### Line wrapping and highlighting

Currently, there is no word wrapping (only grapheme wrapping).
But we highlight the whole input at once.
So there is no issue: for example, even if a keyword is wrapped, style is preserved.

With [prompt continuation](https://github.com/kkawakam/rustyline/pull/372)s,
we (will) interleave user input with continuations.
So we need to preserve style.

TODO How prompt_toolkit handle this problem ?
Maybe using ANSI sequence directly was a bad idea. If `Highlighter` returns style ranges,
applying style on input slice is easy (and also supporting styles on Windows < 10).

### Impacts

Current granularity:

- PUSH_CHAR at end of input
- BEEP
- MOVE_CURSOR
- REFRESH whole input / rows
- CLEAR_SCREEN (+REFRESH)

Wanted additional granularity:

- PUSH_STRING at end of input
- REFRESH_DIRTY only rows / cells
