//! Uses the `inotify` crate to watch for changes in the current directory.
//!
//! Run with:
//!
//! ```
//! cargo run --example linux-inotify
//! ```

#[cfg(target_os = "linux")]
fn main() -> std::io::Result<()> {
    use std::ffi::OsString;
    use std::io;

    use async_io::Async;
    use futures_lite::future;
    use inotify::{EventMask, Inotify, WatchMask};

    type Event = (OsString, EventMask);

    /// Reads some events without blocking.
    ///
    /// If there are no events, an [`io::ErrorKind::WouldBlock`] error is returned.
    fn read_op(inotify: &mut Inotify) -> io::Result<Vec<Event>> {
        let mut buffer = [0; 1024];
        let events = inotify
            .read_events(&mut buffer)?
            .filter_map(|ev| ev.name.map(|name| (name.to_owned(), ev.mask)))
            .collect::<Vec<_>>();

        if events.is_empty() {
            Err(io::ErrorKind::WouldBlock.into())
        } else {
            Ok(events)
        }
    }

    future::block_on(async {
        // Watch events in the current directory.
        let mut inotify = Async::new(Inotify::init()?)?;

        // SAFETY: We do not move the inner file descriptor out.
        unsafe {
            inotify
                .get_mut()
                .watches()
                .add(".", WatchMask::ALL_EVENTS)?;
        }
        println!("Watching for filesystem events in the current directory...");
        println!("Try opening a file to trigger some events.");
        println!();

        // Wait for events in a loop and print them on the screen.
        loop {
            for event in unsafe { inotify.read_with_mut(read_op).await? } {
                println!("{event:?}");
            }
        }
    })
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("This example works only on Linux!");
}
