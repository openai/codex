// This example tests support for generic events (XGE). It generates a window and uses the PRESENT
// extension to cause an XGE event to be sent.

use std::process::exit;

use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::protocol::xproto::{
    ConfigureWindowAux, ConnectionExt as _, CreateWindowAux, WindowClass,
};
use x11rb::protocol::{present, Event};
use x11rb::COPY_DEPTH_FROM_PARENT;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = connect(None)?;
    let screen = &conn.setup().roots[screen_num];

    if conn
        .extension_information(present::X11_EXTENSION_NAME)?
        .is_none()
    {
        eprintln!("Present extension is not supported");
        exit(1);
    }

    // Create a window
    let win_id = conn.generate_id()?;
    let win_aux = CreateWindowAux::new().background_pixel(screen.white_pixel);
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win_id,
        screen.root,
        0,
        0,
        10,
        10,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &win_aux,
    )?;

    // Ask for present ConfigureNotify events
    let event_id = conn.generate_id()?;
    present::select_input(
        &conn,
        event_id,
        win_id,
        present::EventMask::CONFIGURE_NOTIFY,
    )?;

    // Cause an event
    conn.configure_window(win_id, &ConfigureWindowAux::new().width(20))?;

    // Wait for the event
    conn.flush()?;
    let event = conn.wait_for_event()?;

    // Now check that the event really is what we wanted to get
    let event = match event {
        Event::PresentConfigureNotify(event) => event,
        other => panic!("Unexpected event {:?}", other),
    };
    println!(
        "Got a Present ConfigureNotify event for event ID 0x{:x} and window 0x{:x}.",
        event.event, event.window
    );
    println!(
        "x={}, y={}, width={}, height={}, off_x={}, off_y={}, pixmap_width={}, pixmap_height={}, \
         pixmap_flags={:x}",
        event.x,
        event.y,
        event.width,
        event.height,
        event.off_x,
        event.off_y,
        event.pixmap_width,
        event.pixmap_height,
        event.pixmap_flags,
    );
    assert_eq!(
        (20, 10, 0),
        (event.pixmap_width, event.pixmap_height, event.pixmap_flags)
    );

    Ok(())
}

include!("integration_test_util/connect.rs");
