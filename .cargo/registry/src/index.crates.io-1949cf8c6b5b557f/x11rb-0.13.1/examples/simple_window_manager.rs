// A very simple reparenting window manager.
// This WM does NOT follow ICCCM!

extern crate x11rb;

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::process::exit;

use x11rb::connection::Connection;
use x11rb::errors::{ReplyError, ReplyOrIdError};
use x11rb::protocol::xproto::*;
use x11rb::protocol::{ErrorKind, Event};
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME};

const TITLEBAR_HEIGHT: u16 = 20;
const DRAG_BUTTON: Button = 1;

/// The state of a single window that we manage
#[derive(Debug)]
struct WindowState {
    window: Window,
    frame_window: Window,
    x: i16,
    y: i16,
    width: u16,
}

impl WindowState {
    fn new(window: Window, frame_window: Window, geom: &GetGeometryReply) -> WindowState {
        WindowState {
            window,
            frame_window,
            x: geom.x,
            y: geom.y,
            width: geom.width,
        }
    }

    fn close_x_position(&self) -> i16 {
        std::cmp::max(0, self.width - TITLEBAR_HEIGHT) as _
    }
}

/// The state of the full WM
#[derive(Debug)]
struct WmState<'a, C: Connection> {
    conn: &'a C,
    screen_num: usize,
    black_gc: Gcontext,
    windows: Vec<WindowState>,
    pending_expose: HashSet<Window>,
    wm_protocols: Atom,
    wm_delete_window: Atom,
    sequences_to_ignore: BinaryHeap<Reverse<u16>>,
    // If this is Some, we are currently dragging the given window with the given offset relative
    // to the mouse.
    drag_window: Option<(Window, (i16, i16))>,
}

impl<'a, C: Connection> WmState<'a, C> {
    fn new(conn: &'a C, screen_num: usize) -> Result<WmState<'a, C>, ReplyOrIdError> {
        let screen = &conn.setup().roots[screen_num];
        let black_gc = conn.generate_id()?;
        let font = conn.generate_id()?;
        conn.open_font(font, b"9x15")?;

        let gc_aux = CreateGCAux::new()
            .graphics_exposures(0)
            .background(screen.white_pixel)
            .foreground(screen.black_pixel)
            .font(font);
        conn.create_gc(black_gc, screen.root, &gc_aux)?;
        conn.close_font(font)?;

        let wm_protocols = conn.intern_atom(false, b"WM_PROTOCOLS")?;
        let wm_delete_window = conn.intern_atom(false, b"WM_DELETE_WINDOW")?;

        Ok(WmState {
            conn,
            screen_num,
            black_gc,
            windows: Vec::default(),
            pending_expose: HashSet::default(),
            wm_protocols: wm_protocols.reply()?.atom,
            wm_delete_window: wm_delete_window.reply()?.atom,
            sequences_to_ignore: Default::default(),
            drag_window: None,
        })
    }

    /// Scan for already existing windows and manage them
    fn scan_windows(&mut self) -> Result<(), ReplyOrIdError> {
        // Get the already existing top-level windows.
        let screen = &self.conn.setup().roots[self.screen_num];
        let tree_reply = self.conn.query_tree(screen.root)?.reply()?;

        // For each window, request its attributes and geometry *now*
        let mut cookies = Vec::with_capacity(tree_reply.children.len());
        for win in tree_reply.children {
            let attr = self.conn.get_window_attributes(win)?;
            let geom = self.conn.get_geometry(win)?;
            cookies.push((win, attr, geom));
        }
        // Get the replies and manage windows
        for (win, attr, geom) in cookies {
            if let (Ok(attr), Ok(geom)) = (attr.reply(), geom.reply()) {
                if !attr.override_redirect && attr.map_state != MapState::UNMAPPED {
                    self.manage_window(win, &geom)?;
                }
            } else {
                // Just skip this window
            }
        }

        Ok(())
    }

    /// Add a new window that should be managed by the WM
    fn manage_window(
        &mut self,
        win: Window,
        geom: &GetGeometryReply,
    ) -> Result<(), ReplyOrIdError> {
        println!("Managing window {:?}", win);
        let screen = &self.conn.setup().roots[self.screen_num];
        assert!(self.find_window_by_id(win).is_none());

        let frame_win = self.conn.generate_id()?;
        let win_aux = CreateWindowAux::new()
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::SUBSTRUCTURE_NOTIFY
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION
                    | EventMask::ENTER_WINDOW,
            )
            .background_pixel(screen.white_pixel);
        self.conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            frame_win,
            screen.root,
            geom.x,
            geom.y,
            geom.width,
            geom.height + TITLEBAR_HEIGHT,
            1,
            WindowClass::INPUT_OUTPUT,
            0,
            &win_aux,
        )?;

        self.conn.grab_server()?;
        self.conn.change_save_set(SetMode::INSERT, win)?;
        let cookie = self
            .conn
            .reparent_window(win, frame_win, 0, TITLEBAR_HEIGHT as _)?;
        self.conn.map_window(win)?;
        self.conn.map_window(frame_win)?;
        self.conn.ungrab_server()?;

        self.windows.push(WindowState::new(win, frame_win, geom));

        // Ignore all events caused by reparent_window(). All those events have the sequence number
        // of the reparent_window() request, thus remember its sequence number. The
        // grab_server()/ungrab_server() is done so that the server does not handle other clients
        // in-between, which could cause other events to get the same sequence number.
        self.sequences_to_ignore
            .push(Reverse(cookie.sequence_number() as u16));
        Ok(())
    }

    /// Draw the titlebar of a window
    fn redraw_titlebar(&self, state: &WindowState) -> Result<(), ReplyError> {
        let close_x = state.close_x_position();
        self.conn.poly_line(
            CoordMode::ORIGIN,
            state.frame_window,
            self.black_gc,
            &[
                Point { x: close_x, y: 0 },
                Point {
                    x: state.width as _,
                    y: TITLEBAR_HEIGHT as _,
                },
            ],
        )?;
        self.conn.poly_line(
            CoordMode::ORIGIN,
            state.frame_window,
            self.black_gc,
            &[
                Point {
                    x: close_x,
                    y: TITLEBAR_HEIGHT as _,
                },
                Point {
                    x: state.width as _,
                    y: 0,
                },
            ],
        )?;
        let reply = self
            .conn
            .get_property(
                false,
                state.window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                0,
                std::u32::MAX,
            )?
            .reply()?;
        self.conn
            .image_text8(state.frame_window, self.black_gc, 1, 10, &reply.value)?;
        Ok(())
    }

    /// Do all pending work that was queued while handling some events
    fn refresh(&mut self) {
        while let Some(&win) = self.pending_expose.iter().next() {
            self.pending_expose.remove(&win);
            if let Some(state) = self.find_window_by_id(win) {
                if let Err(err) = self.redraw_titlebar(state) {
                    eprintln!(
                        "Error while redrawing window {:x?}: {:?}",
                        state.window, err
                    );
                }
            }
        }
    }

    fn find_window_by_id(&self, win: Window) -> Option<&WindowState> {
        self.windows
            .iter()
            .find(|state| state.window == win || state.frame_window == win)
    }

    fn find_window_by_id_mut(&mut self, win: Window) -> Option<&mut WindowState> {
        self.windows
            .iter_mut()
            .find(|state| state.window == win || state.frame_window == win)
    }

    /// Handle the given event
    fn handle_event(&mut self, event: Event) -> Result<(), ReplyOrIdError> {
        let mut should_ignore = false;
        if let Some(seqno) = event.wire_sequence_number() {
            // Check sequences_to_ignore and remove entries with old (=smaller) numbers.
            while let Some(&Reverse(to_ignore)) = self.sequences_to_ignore.peek() {
                // Sequence numbers can wrap around, so we cannot simply check for
                // "to_ignore <= seqno". This is equivalent to "to_ignore - seqno <= 0", which is what we
                // check instead. Since sequence numbers are unsigned, we need a trick: We decide
                // that values from [MAX/2, MAX] count as "<= 0" and the rest doesn't.
                if to_ignore.wrapping_sub(seqno) <= u16::max_value() / 2 {
                    // If the two sequence numbers are equal, this event should be ignored.
                    should_ignore = to_ignore == seqno;
                    break;
                }
                self.sequences_to_ignore.pop();
            }
        }

        println!("Got event {:?}", event);
        if should_ignore {
            println!("  [ignored]");
            return Ok(());
        }
        match event {
            Event::UnmapNotify(event) => self.handle_unmap_notify(event),
            Event::ConfigureRequest(event) => self.handle_configure_request(event)?,
            Event::MapRequest(event) => self.handle_map_request(event)?,
            Event::Expose(event) => self.handle_expose(event),
            Event::EnterNotify(event) => self.handle_enter(event)?,
            Event::ButtonPress(event) => self.handle_button_press(event),
            Event::ButtonRelease(event) => self.handle_button_release(event)?,
            Event::MotionNotify(event) => self.handle_motion_notify(event)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_unmap_notify(&mut self, event: UnmapNotifyEvent) {
        let root = self.conn.setup().roots[self.screen_num].root;
        let conn = self.conn;
        self.windows.retain(|state| {
            if state.window != event.window {
                return true;
            }
            conn.change_save_set(SetMode::DELETE, state.window).unwrap();
            conn.reparent_window(state.window, root, state.x, state.y)
                .unwrap();
            conn.destroy_window(state.frame_window).unwrap();
            false
        });
    }

    fn handle_configure_request(&mut self, event: ConfigureRequestEvent) -> Result<(), ReplyError> {
        if let Some(state) = self.find_window_by_id_mut(event.window) {
            let _ = state;
            unimplemented!();
        }
        // Allow clients to change everything, except sibling / stack mode
        let aux = ConfigureWindowAux::from_configure_request(&event)
            .sibling(None)
            .stack_mode(None);
        println!("Configure: {:?}", aux);
        self.conn.configure_window(event.window, &aux)?;
        Ok(())
    }

    fn handle_map_request(&mut self, event: MapRequestEvent) -> Result<(), ReplyOrIdError> {
        self.manage_window(
            event.window,
            &self.conn.get_geometry(event.window)?.reply()?,
        )
    }

    fn handle_expose(&mut self, event: ExposeEvent) {
        self.pending_expose.insert(event.window);
    }

    fn handle_enter(&mut self, event: EnterNotifyEvent) -> Result<(), ReplyError> {
        if let Some(state) = self.find_window_by_id(event.event) {
            // Set the input focus (ignoring ICCCM's WM_PROTOCOLS / WM_TAKE_FOCUS)
            self.conn
                .set_input_focus(InputFocus::PARENT, state.window, CURRENT_TIME)?;
            // Also raise the window to the top of the stacking order
            self.conn.configure_window(
                state.frame_window,
                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
            )?;
        }
        Ok(())
    }

    fn handle_button_press(&mut self, event: ButtonPressEvent) {
        if event.detail != DRAG_BUTTON || u16::from(event.state) != 0 {
            return;
        }
        if let Some(state) = self.find_window_by_id(event.event) {
            if self.drag_window.is_none() && event.event_x < state.close_x_position() {
                let (x, y) = (-event.event_x, -event.event_y);
                self.drag_window = Some((state.frame_window, (x, y)));
            }
        }
    }

    fn handle_button_release(&mut self, event: ButtonReleaseEvent) -> Result<(), ReplyError> {
        if event.detail == DRAG_BUTTON {
            self.drag_window = None;
        }
        if let Some(state) = self.find_window_by_id(event.event) {
            if event.event_x >= state.close_x_position() {
                let event = ClientMessageEvent::new(
                    32,
                    state.window,
                    self.wm_protocols,
                    [self.wm_delete_window, 0, 0, 0, 0],
                );
                self.conn
                    .send_event(false, state.window, EventMask::NO_EVENT, event)?;
            }
        }
        Ok(())
    }

    fn handle_motion_notify(&mut self, event: MotionNotifyEvent) -> Result<(), ReplyError> {
        if let Some((win, (x, y))) = self.drag_window {
            let (x, y) = (x + event.root_x, y + event.root_y);
            // Sigh, X11 and its mixing up i16 and i32
            let (x, y) = (x as i32, y as i32);
            self.conn
                .configure_window(win, &ConfigureWindowAux::new().x(x).y(y))?;
        }
        Ok(())
    }
}

fn become_wm<C: Connection>(conn: &C, screen: &Screen) -> Result<(), ReplyError> {
    // Try to become the window manager. This causes an error if there is already another WM.
    let change = ChangeWindowAttributesAux::default()
        .event_mask(EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY);
    let res = conn.change_window_attributes(screen.root, &change)?.check();
    if let Err(ReplyError::X11Error(ref error)) = res {
        if error.error_kind == ErrorKind::Access {
            eprintln!("Another WM is already running.");
            exit(1);
        } else {
            res
        }
    } else {
        res
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = connect(None)?;

    // The following is only needed for start_timeout_thread(), which is used for 'tests'
    let conn1 = std::sync::Arc::new(conn);
    let conn = &*conn1;

    let screen = &conn.setup().roots[screen_num];

    become_wm(conn, screen)?;

    let mut wm_state = WmState::new(conn, screen_num)?;
    wm_state.scan_windows()?;

    util::start_timeout_thread(conn1.clone(), screen.root);

    loop {
        wm_state.refresh();
        conn.flush()?;

        let event = conn.wait_for_event()?;
        let mut event_option = Some(event);
        while let Some(event) = event_option {
            if let Event::ClientMessage(_) = event {
                // This is start_timeout_thread() signaling us to close (most likely).
                return Ok(());
            }

            wm_state.handle_event(event)?;
            event_option = conn.poll_for_event()?;
        }
    }
}

include!("integration_test_util/connect.rs");
include!("integration_test_util/util.rs");
