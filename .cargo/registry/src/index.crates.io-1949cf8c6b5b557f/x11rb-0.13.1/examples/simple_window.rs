extern crate x11rb;

use x11rb::connection::Connection;
use x11rb::cursor::Handle as CursorHandle;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::resource_manager::new_from_default;
use x11rb::wrapper::ConnectionExt as _;

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        _NET_WM_NAME,
        UTF8_STRING,
    }
}

fn init_tracing() {
    use tracing_subscriber::prelude::*;

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let (conn, screen_num) = x11rb::connect(None)?;

    // The following is only needed for start_timeout_thread(), which is used for 'tests'
    let conn1 = std::sync::Arc::new(conn);
    let conn = &*conn1;

    let screen = &conn.setup().roots[screen_num];
    let win_id = conn.generate_id()?;
    let gc_id = conn.generate_id()?;
    let resource_db = new_from_default(conn)?;
    let cursor_handle = CursorHandle::new(conn, screen_num, &resource_db)?;

    let atoms = Atoms::new(conn)?.reply()?;
    let cursor_handle = cursor_handle.reply()?;

    let win_aux = CreateWindowAux::new()
        .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY | EventMask::NO_EVENT)
        .background_pixel(screen.white_pixel)
        .win_gravity(Gravity::NORTH_WEST)
        // Just because, we set the cursor to "wait"
        .cursor(cursor_handle.load_cursor(conn, "wait")?);

    let gc_aux = CreateGCAux::new().foreground(screen.black_pixel);

    let (mut width, mut height) = (100, 100);

    conn.create_window(
        screen.root_depth,
        win_id,
        screen.root,
        0,
        0,
        width,
        height,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &win_aux,
    )?;

    util::start_timeout_thread(conn1.clone(), win_id);

    let title = "Simple Window";
    conn.change_property8(
        PropMode::REPLACE,
        win_id,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        title.as_bytes(),
    )?;
    conn.change_property8(
        PropMode::REPLACE,
        win_id,
        atoms._NET_WM_NAME,
        atoms.UTF8_STRING,
        title.as_bytes(),
    )?;
    conn.change_property32(
        PropMode::REPLACE,
        win_id,
        atoms.WM_PROTOCOLS,
        AtomEnum::ATOM,
        &[atoms.WM_DELETE_WINDOW],
    )?;
    conn.change_property8(
        PropMode::REPLACE,
        win_id,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        b"simple_window\0simple_window\0",
    )?;
    conn.change_property8(
        PropMode::REPLACE,
        win_id,
        AtomEnum::WM_CLIENT_MACHINE,
        AtomEnum::STRING,
        // Text encoding in X11 is complicated. Let's use UTF-8 and hope for the best.
        gethostname::gethostname()
            .to_str()
            .unwrap_or("[Invalid]")
            .as_bytes(),
    )?;

    let reply = conn
        .get_property(false, win_id, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024)?
        .reply()?;
    assert_eq!(reply.value, title.as_bytes());

    conn.create_gc(gc_id, win_id, &gc_aux)?;

    conn.map_window(win_id)?;

    conn.flush()?;

    loop {
        let event = conn.wait_for_event()?;
        tracing::debug!("Got event {event:?}");
        match event {
            Event::Expose(event) => {
                if event.count == 0 {
                    // There already is a white background because we set background_pixel to white
                    // when creating the window.
                    let (width, height): (i16, i16) = (width as _, height as _);
                    let points = [
                        Point {
                            x: width,
                            y: height,
                        },
                        Point { x: -10, y: -10 },
                        Point {
                            x: -10,
                            y: height + 10,
                        },
                        Point {
                            x: width + 10,
                            y: -10,
                        },
                    ];
                    conn.poly_line(CoordMode::ORIGIN, win_id, gc_id, &points)?;
                    conn.flush()?;
                }
            }
            Event::ConfigureNotify(event) => {
                width = event.width;
                height = event.height;
            }
            Event::ClientMessage(event) => {
                let data = event.data.as_data32();
                if event.format == 32 && event.window == win_id && data[0] == atoms.WM_DELETE_WINDOW
                {
                    tracing::info!("Window was asked to close");
                    return Ok(());
                }
            }
            Event::Error(err) => {
                tracing::error!("Got an unexpected error: {err:?}")
            }
            event => tracing::info!("Got an unhandled event: {event:?}"),
        }
    }
}

include!("integration_test_util/util.rs");
