# Input Form example

This example demonstrates how to handle input across several form fields (2 strings and an number).
It uses an enum to track the focused field, and sends keyboard events to one which is current.

Run this example with:

```shell
cargo run -p input-form
```

This example does not handle things like cursor movement within the line (just keys and backspace).
Most apps would benefit from using the following crates for text input rather than directly using
strings:

- [`tui-input`](https://crates.io/crates/tui-input)
- [`tui-prompts`](https://crates.io/crates/tui-prompts)
- [`tui-textarea`](https://crates.io/crates/tui-textarea)
- [`rat-salsa`](https://crates.io/crates/rat-salsa)

Some more ideas for handling focus can be found in:

- [`focusable`](https://crates.io/crates/focusable) (see also [Ratatui forum
  post](https://forum.ratatui.rs/t/focusable-crate-manage-focus-state-for-your-widgets/73))
- [`rat-focus`](https://crates.io/crates/rat-focus)
- A useful [`Bevy` discussion](https://github.com/bevyengine/bevy/discussions/15374) about focus
  more generally.
