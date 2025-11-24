// As far as I can see, I cannot easily share code between different examples. The following code
// is used by several examples to react to the $X11RB_EXAMPLE_TIMEOUT variable. This code is
// include!()d in the examples

mod util {
    use std::env;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{
        ClientMessageEvent, ConnectionExt as _, EventMask, Window,
    };

    pub fn start_timeout_thread<C>(conn: Arc<C>, window: Window)
    where
        C: Connection + Send + Sync + 'static,
    {
        let timeout = match env::var("X11RB_EXAMPLE_TIMEOUT")
            .ok()
            .and_then(|str| str.parse().ok())
        {
            None => return,
            Some(timeout) => timeout,
        };

        thread::spawn(move || {
            let wm_protocols = conn.intern_atom(false, b"WM_PROTOCOLS").unwrap();
            let wm_delete_window = conn.intern_atom(false, b"WM_DELETE_WINDOW").unwrap();

            thread::sleep(Duration::from_secs(timeout));

            let event = ClientMessageEvent::new(
                32,
                window,
                wm_protocols.reply().unwrap().atom,
                [wm_delete_window.reply().unwrap().atom, 0, 0, 0, 0],
            );

            if let Err(err) = conn.send_event(false, window, EventMask::NO_EVENT, event) {
                eprintln!("Error while sending event: {:?}", err);
            }
            if let Err(err) = conn.send_event(
                false,
                window,
                EventMask::SUBSTRUCTURE_REDIRECT,
                event,
            ) {
                eprintln!("Error while sending event: {:?}", err);
            }
            conn.flush().unwrap();
        });
    }
}
