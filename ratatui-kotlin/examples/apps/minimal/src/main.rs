//! A minimal example of a Ratatui application.
//!
//! This is a bare minimum example. There are many approaches to running an application loop,
//! so this is not meant to be prescriptive. See the [examples] folder for more complete
//! examples. In particular, the [hello-world] example is a good starting point.
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//! [examples]: https://github.com/ratatui/ratatui/blob/main/examples
//! [hello-world]: https://github.com/ratatui/ratatui/blob/main/examples/apps/hello-world
fn main() -> Result<(), Box<dyn std::error::Error>> {
    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| frame.render_widget("Hello World!", frame.area()))?;
            if crossterm::event::read()?.is_key_press() {
                break Ok(());
            }
        }
    })
}
