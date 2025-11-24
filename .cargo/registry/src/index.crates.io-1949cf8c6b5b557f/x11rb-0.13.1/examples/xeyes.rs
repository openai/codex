// This is (quite loosely) based on the original xeyes

extern crate x11rb;

use x11rb::connection::{Connection, RequestConnection as _};
use x11rb::errors::{ConnectionError, ReplyOrIdError};
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;

const PUPIL_SIZE: i16 = 50;
const EYE_SIZE: i16 = 50;

// Draw the big background of the eyes
fn draw_eyes<C: Connection>(
    conn: &C,
    win_id: Window,
    black: Gcontext,
    white: Gcontext,
    window_size: (u16, u16),
) -> Result<(), ConnectionError> {
    // Draw the black outlines
    let mut arc1 = Arc {
        x: 0,
        y: 0,
        width: window_size.0 / 2,
        height: window_size.1,
        angle1: 0,
        angle2: 360 * 64,
    };
    let mut arc2 = arc1;
    arc2.x = arc2.width as _;
    conn.poly_fill_arc(win_id, black, &[arc1, arc2])?;

    // Draw the white inner part
    for arc in [&mut arc1, &mut arc2].iter_mut() {
        arc.x += EYE_SIZE;
        arc.y += EYE_SIZE;
        arc.width -= 2 * EYE_SIZE as u16;
        arc.height -= 2 * EYE_SIZE as u16;
    }
    conn.poly_fill_arc(win_id, white, &[arc1, arc2])?;

    Ok(())
}

// Draw the pupils inside the eye
fn draw_pupils<C: Connection>(
    conn: &C,
    win_id: Window,
    gc: Gcontext,
    ((x1, y1), (x2, y2)): ((i16, i16), (i16, i16)),
) -> Result<(), ConnectionError> {
    // Transform center to top left corner
    let (x1, y1) = (x1 - PUPIL_SIZE / 2, y1 - PUPIL_SIZE / 2);
    let (x2, y2) = (x2 - PUPIL_SIZE / 2, y2 - PUPIL_SIZE / 2);

    let arc1 = Arc {
        x: x1,
        y: y1,
        width: PUPIL_SIZE as _,
        height: PUPIL_SIZE as _,
        angle1: 90 * 64,
        angle2: 360 * 64,
    };
    let mut arc2 = arc1;
    arc2.x = x2;
    arc2.y = y2;

    // Do the drawing
    conn.poly_fill_arc(win_id, gc, &[arc1, arc2])?;
    Ok(())
}

// Given two points, return their squared distance
fn distance_squared(p1: (f64, f64), p2: (f64, f64)) -> f64 {
    let dx = p1.0 - p2.0;
    let dy = p1.1 - p2.1;
    dx * dx + dy * dy
}

// Compute the position of a pupil inside of the given area.
fn compute_pupil(area: (i16, i16, i16, i16), mouse: (i16, i16)) -> (i16, i16) {
    // What is the center of the eye?
    let center_x = area.0 + area.2 / 2;
    let center_y = area.1 + area.3 / 2;
    let (w, h) = (f64::from(area.2) / 2.0, f64::from(area.3) / 2.0);

    // Is the mouse exactly on the center?
    if (center_x, center_y) == mouse {
        return mouse;
    }

    let center = (f64::from(center_x), f64::from(center_y));
    let mouse = (f64::from(mouse.0), f64::from(mouse.1));

    // Calculate the offset of the mouse position from the center
    let diff = (mouse.0 - center.0, mouse.1 - center.1);

    // An eclipse is described by this equation, where the angle 'a' is varied over all values, but
    // does not actually describe the angle from the center due to the different scaling in x and y
    // direction.
    //
    //  x = w * cos(a)
    //  y = h * sin(a)
    //
    // With tan(a) = sin(a)/cos(a), we get
    //
    //  tan(a) * x = w * sin(a) => sin(a) = tan(a) * x / w
    //  y = h * sin(a)          => sin(a) = y / h
    //
    // and thus
    //
    //   tan(a) * x / w = y / h
    //
    // which we can rearrange to
    //
    //   tan(a) = (y * w) / (x * h)
    //
    // And thus, the angle we are looking for is:
    //
    //   a = arctan((y * w) / (x * h))
    //
    // However, due to tan() being the way it is, we actually need:
    let angle = (diff.1 * w).atan2(diff.0 * h);

    // Now compute the corresponding point on the ellipse (relative to the center)
    let (cx, cy) = (w * angle.cos(), h * angle.sin());

    // ...and also compute the actual point
    let (x, y) = ((center.0 + cx) as _, (center.1 + cy) as _);

    // Return the point that is closer to the center
    if distance_squared(center, mouse) < distance_squared(center, (x, y)) {
        (mouse.0 as _, mouse.1 as _)
    } else {
        (x as _, y as _)
    }
}

// Compute the position of both pupils.
fn compute_pupils(window_size: (u16, u16), mouse_position: (i16, i16)) -> ((i16, i16), (i16, i16)) {
    let border = PUPIL_SIZE + EYE_SIZE;
    let half_width = window_size.0 as i16 / 2;
    let width = half_width - 2 * border;
    let height = window_size.1 as i16 - 2 * border;

    (
        compute_pupil((border, border, width, height), mouse_position),
        compute_pupil((border + half_width, border, width, height), mouse_position),
    )
}

fn shape_window<C: Connection>(
    conn: &C,
    win_id: Window,
    window_size: (u16, u16),
) -> Result<(), ReplyOrIdError> {
    // Create a pixmap for the shape
    let pixmap = PixmapWrapper::create_pixmap(conn, 1, win_id, window_size.0, window_size.1)?;

    // Fill the pixmap with what will indicate "transparent"
    let gc = create_gc_with_foreground(conn, pixmap.pixmap(), 0)?;

    let rect = Rectangle {
        x: 0,
        y: 0,
        width: window_size.0,
        height: window_size.1,
    };
    conn.poly_fill_rectangle(pixmap.pixmap(), gc.gcontext(), &[rect])?;

    // Draw the eyes as "not transparent"
    let values = ChangeGCAux::new().foreground(1);
    conn.change_gc(gc.gcontext(), &values)?;
    draw_eyes(
        conn,
        pixmap.pixmap(),
        gc.gcontext(),
        gc.gcontext(),
        window_size,
    )?;

    // Set the shape of the window
    conn.shape_mask(shape::SO::SET, shape::SK::BOUNDING, win_id, 0, 0, &pixmap)?;
    Ok(())
}

fn setup_window<C: Connection>(
    conn: &C,
    screen: &Screen,
    window_size: (u16, u16),
    wm_protocols: Atom,
    wm_delete_window: Atom,
) -> Result<Window, ReplyOrIdError> {
    let win_id = conn.generate_id()?;
    let win_aux = CreateWindowAux::new()
        .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY | EventMask::POINTER_MOTION)
        .background_pixel(screen.white_pixel);

    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win_id,
        screen.root,
        0,
        0,
        window_size.0,
        window_size.1,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &win_aux,
    )?;

    let title = "xeyes";
    conn.change_property8(
        PropMode::REPLACE,
        win_id,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        title.as_bytes(),
    )?;
    conn.change_property32(
        PropMode::REPLACE,
        win_id,
        wm_protocols,
        AtomEnum::ATOM,
        &[wm_delete_window],
    )?;

    conn.map_window(win_id)?;

    Ok(win_id)
}

fn create_gc_with_foreground<C: Connection>(
    conn: C,
    win_id: Window,
    foreground: u32,
) -> Result<GcontextWrapper<C>, ReplyOrIdError> {
    GcontextWrapper::create_gc(
        conn,
        win_id,
        &CreateGCAux::new()
            .graphics_exposures(0)
            .foreground(foreground),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = connect(None).expect("Failed to connect to the X11 server");

    // The following is only needed for start_timeout_thread(), which is used for 'tests'
    let conn = std::sync::Arc::new(conn);

    let screen = &conn.setup().roots[screen_num];

    let wm_protocols = conn.intern_atom(false, b"WM_PROTOCOLS")?;
    let wm_delete_window = conn.intern_atom(false, b"WM_DELETE_WINDOW")?;

    let mut window_size = (700, 500);
    let has_shape = conn
        .extension_information(shape::X11_EXTENSION_NAME)
        .expect("failed to get extension information")
        .is_some();
    let (wm_protocols, wm_delete_window) =
        (wm_protocols.reply()?.atom, wm_delete_window.reply()?.atom);
    let win_id = setup_window(&conn, screen, window_size, wm_protocols, wm_delete_window)?;
    let mut pixmap = PixmapWrapper::create_pixmap(
        conn.clone(),
        screen.root_depth,
        win_id,
        window_size.0,
        window_size.1,
    )?;

    let black_gc = create_gc_with_foreground(&conn, win_id, screen.black_pixel)?;
    let white_gc = create_gc_with_foreground(&conn, win_id, screen.white_pixel)?;

    conn.flush()?;

    let mut need_repaint = false;
    let mut need_reshape = false;
    let mut mouse_position = (0, 0);

    util::start_timeout_thread(conn.clone(), win_id);

    loop {
        let event = conn.wait_for_event()?;
        let mut event_option = Some(event);
        while let Some(event) = event_option {
            match event {
                Event::Expose(event) => {
                    if event.count == 0 {
                        need_repaint = true;
                    }
                }
                Event::ConfigureNotify(event) => {
                    window_size = (event.width, event.height);
                    pixmap = PixmapWrapper::create_pixmap(
                        conn.clone(),
                        screen.root_depth,
                        win_id,
                        window_size.0,
                        window_size.1,
                    )?;
                    need_reshape = true;
                }
                Event::MotionNotify(event) => {
                    mouse_position = (event.event_x, event.event_y);
                    need_repaint = true;
                }
                Event::MapNotify(_) => {
                    need_reshape = true;
                }
                Event::ClientMessage(event) => {
                    let data = event.data.as_data32();
                    if event.format == 32 && event.window == win_id && data[0] == wm_delete_window {
                        println!("Window was asked to close");
                        return Ok(());
                    }
                }
                Event::Error(error) => {
                    println!("Unknown error {:?}", error);
                }
                event => {
                    println!("Unknown event {:?}", event);
                }
            }

            event_option = conn.poll_for_event()?;
        }

        if need_reshape && has_shape {
            shape_window(&conn, win_id, window_size)?;
            need_reshape = false;
        }
        if need_repaint {
            // Draw new pupils
            let pos = compute_pupils(window_size, mouse_position);
            draw_eyes(
                &conn,
                pixmap.pixmap(),
                black_gc.gcontext(),
                white_gc.gcontext(),
                window_size,
            )?;
            draw_pupils(&conn, pixmap.pixmap(), black_gc.gcontext(), pos)?;

            // Copy drawing from pixmap to window
            conn.copy_area(
                pixmap.pixmap(),
                win_id,
                white_gc.gcontext(),
                0,
                0,
                0,
                0,
                window_size.0,
                window_size.1,
            )?;

            conn.flush()?;
            need_repaint = false;
        }
    }
}

include!("integration_test_util/connect.rs");
include!("integration_test_util/util.rs");
