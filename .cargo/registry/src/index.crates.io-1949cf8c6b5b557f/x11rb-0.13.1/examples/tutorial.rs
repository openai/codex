///////////////////////////////////////////////////////////////////////////////////////////////////
//
// This file contains a tutorial of both this crate and the X11 protocol. It was created by porting
// an existing libxcb tutorial that is available here:
//
//  https://www.x.org/releases/X11R7.7/doc/libxcb/tutorial/index.html
//
// References to libxcb in the text were retained. Only the code examples were adapted, where
// possible. Some detailed explanations of libxcb required heavier editing.
//
// Since the libxcb tutorial is part of the libxcb source code, I assume that its MIT LICENSE
// applies. The exact situation is a bit unclear (libxcb's COPYING file has "Copyright (C)
// 2001-2006 Bart Massey, Jamey Sharp, and Josh Triplett.", but the git history says that the
// tutorial was only started in 2006 and was last touched in 2014).
//
// This tutorial is in a Rust file to ensure that the contained code actually compiles and
// everything works as intended. Nothing is worse than bitrotted tutorial. Here are all the
// imports:

extern crate x11rb;

use std::error::Error;

use x11rb::connection::Connection;
use x11rb::errors::{ConnectionError, ReplyError, ReplyOrIdError};
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;
use x11rb_protocol::SequenceNumber;

///////////////////////////////////////////////////////////////////////////////////////////////////
//
//     Introduction
//     ============
//
// This tutorial is based on the [Xlib Tutorial] written by Guy Keren. The author allowed me to
// take some parts of his text, mainly the text which deals with the X Windows generality.
//
// [Xlib Tutorial](http://users.actcom.co.il/~choo/lupg/tutorials/xlib-programming/xlib-programming.html)
//
// This tutorial is intended for people who want to start to program with the [XCB] library. keep
// in mind that XCB, like the [Xlib] library, isn't what most programmers wanting to write X
// applications are looking for. They should use a much higher level GUI toolkit like Motif,
// [LessTiff], [GTK], [QT], [EWL], [ETK], or use [Cairo]. However, we need to start somewhere. More
// than this, knowing how things work down below is never a bad idea.
//
// [XCB](http://xcb.freedesktop.org)
// [Xlib](http://tronche.com/gui/x/xlib/introduction)
// [LessTiff](http://www.lesstif.org)
// [GTK](http://www.gtk.org)
// [QT](http://www.trolltech.com)
// [EWL](http://www.enlightenment.org)
// [ETK](http://www.enlightenment.org)
// [Cairo](http://cairographics.org)
//
// After reading this tutorial, one should be able to write very simple graphical programs, but not
// programs with decent user interfaces. For such programs, one of the previously mentioned
// libraries should be used.
//
// But what is XCB? Xlib has been the standard C binding for the [X Window System] protocol for
// many years now. It is an excellent piece of work, but there are applications for which it is not
// ideal, for example:
//
// [X Window System](http://www.x.org)
//
// *   **Small platforms**: Xlib is a large piece of code, and it's difficult to make it smaller
// *   **Latency hiding**: Xlib requests requiring a reply are effectively synchronous: they block
//                         until the reply appears, whether the result is needed immediately or
//                         not.
// *   **Direct access to the protocol**: Xlib does quite a bit of caching, layering, and similar
//                         optimizations. While this is normally a feature, it makes it difficult
//                         to simply emit specified X protocol requests and process specific
//                         responses.
// *   **Threaded applications**: While Xlib does attempt to support multithreading, the API makes
//                         this difficult and error-prone.
// *   **New extensions**: The Xlib infrastructure provides limited support for the new creation of
//                         X extension client side code.
//
// For these reasons, among others, XCB, an X C binding, has been designed to solve the above
// problems and thus provide a base for
//
// *   Toolkit implementation.
// *   Direct protocol-level programming.
// *   Lightweight emulation of commonly used portions of the Xlib API.
//
//
//     The client and server model of the X window system
//     ==================================================
//
// The X Window System was developed with one major goal: flexibility. The idea was that the way
// things look is one thing, but the way things work is another matter. Thus, the lower levels
// provide the tools required to draw windows, handle user input, allow drawing graphics using
// colors (or black and white screens), etc. To this point, a decision was made to separate the
// system into two parts. A client that decides what to do, and a server that actually draws on the
// screen and reads user input in order to send it to the client for processing.
//
// This model is the complete opposite of what is used to when dealing with clients and servers. In
// our case, the user sits near the machine controlled by the server, while the client might be
// running on a remote machine. The server controls the screens, mouse and keyboard. A client may
// connect to the server, request that it draws a window (or several windows), and ask the server
// to send it any input the user sends to these windows. Thus, several clients may connect to a
// single X server (one might be running mail software, one running a WWW browser, etc). When input
// is sent by the user to some window, the server sends a message to the client controlling this
// window for processing. The client decides what to do with this input, and sends the server
// requests for drawing in the window.
//
// The whole session is carried out using the X message protocol. This protocol was originally
// carried over the TCP/IP protocol suite, allowing the client to run on any machine connected to
// the same network that the server is. Later on, the X servers were extended to allow clients
// running on the local machine with more optimized access to the server (note that an X protocol
// message may be several hundreds of KB in size), such as using shared memory, or using Unix
// domain sockets (a method for creating a logical channel on a Unix system between two processes).
//
//
//     GUI programming: the asynchronous model
//     =======================================
//
// Unlike conventional computer programs, that carry some serial nature, a GUI program usually uses
// an asynchronous programming model, also known as "event-driven programming". This means that
// that program mostly sits idle, waiting for events sent by the X server, and then acts upon these
// events. An event may say "The user pressed the 1st button mouse in spot (x,y)", or "The window
// you control needs to be redrawn". In order for the program to be responsive to the user input,
// as well as to refresh requests, it needs to handle each event in a rather short period of time
// (e.g. less that 200 milliseconds, as a rule of thumb).
//
// This also implies that the program may not perform operations that might take a long time while
// handling an event (such as opening a network connection to some remote server, or connecting to
// a database server, or even performing a long file copy operation). Instead, it needs to perform
// all these operations in an asynchronous manner. This may be done by using various asynchronous
// models to perform the longish operations, or by performing them in a different process or
// thread.
//
// So the way a GUI program looks is something like that:

// 1.  Perform initialization routines.
// 2.  Connect to the X server.
// 3.  Perform X-related initialization.
// 4.  While not finished:
//     1.  Receive the next event from the X server.
//     2.  Handle the event, possibly sending various drawing requests to the X server.
//     3.  If the event was a quit message, exit the loop.
// 5.  Close down the connection to the X server.
// 6.  Perform cleanup operations.
//
//
//     Basic XCB notions
//     =================
//
// XCB has been created to eliminate the need for programs to actually implement the X protocol
// layer. This library gives a program a very low-level access to any X server. Since the protocol
// is standardized, a client using any implementation of XCB may talk with any X server (the same
// occurs for Xlib, of course). We now give a brief description of the basic XCB notions. They will
// be detailed later.
//
//
//     The X Connection
//     ----------------
//
// The major notion of using XCB is the X Connection. This is a structure representing the
// connection we have open with a given X server. It hides a queue of messages coming from the
// server, and a queue of pending requests that our client intends to send to the server. In XCB,
// this structure is named 'xcb_connection_t'. It is analogous to the Xlib Display. When we open a
// connection to an X server, the library returns a pointer to such a structure. Later, we supply
// this pointer to any XCB function that should send messages to the X server or receive messages
// from this server.
//
//
//     Requests and replies: the Xlib killers
//     --------------------------------------
//
// To ask for information from the X server, we have to make a request and ask for a reply. With
// Xlib, these two tasks are automatically done: Xlib locks the system, sends a request, waits for
// a reply from the X server and unlocks. This is annoying, especially if one makes a lot of
// requests to the X server. Indeed, Xlib has to wait for the end of a reply before asking for the
// next request (because of the locks that Xlib sends). For example, here is a time-line of N=4
// requests/replies with Xlib, with a round-trip latency **T_round_trip** that is 5 times long as
// the time required to write or read a request/reply (**T_write/T_read**):
//
//       W-----RW-----RW-----RW-----R
//
// *   W: Writing request
// *   -: Stalled, waiting for data
// *   R: Reading reply
//
// The total time is N * (T_write + T_round_trip + T_read).
//
// With XCB, we can suppress most of the round-trips as the requests and the replies are not
// locked. We usually send a request, then XCB returns to us a **cookie**, which is an identifier.
// Then, later, we ask for a reply using this **cookie** and XCB returns a pointer to that reply.
// Hence, with XCB, we can send a lot of requests, and later in the program, ask for all the
// replies when we need them. Here is the time-line for 4 requests/replies when we use this
// property of XCB:
//
//       WWWW--RRRR
//
// The total time is N * T_write + max (0, T_round_trip - (N-1) * T_write) + N * T_read.
// Which can be considerably faster than all those Xlib round-trips.
//
// Here is a program that computes the time to create 500 atoms with Xlib and XCB. It shows the Xlib way, the bad XCB way (which is similar to Xlib) and the good XCB way. On my computer, XCB is 25 times faster than Xlib.
//

#[allow(clippy::needless_collect)]
fn example1() -> Result<(), Box<dyn Error>> {
    use std::time::Instant;

    let (conn, _) = x11rb::connect(None)?;
    const COUNT: usize = 500;
    let mut atoms = [Into::<u32>::into(AtomEnum::NONE); COUNT];

    // Init names
    let names = (0..COUNT).map(|i| format!("NAME{}", i)).collect::<Vec<_>>();

    // Bad use
    let start = Instant::now();
    for i in 0..COUNT {
        atoms[i] = conn.intern_atom(false, names[i].as_bytes())?.reply()?.atom;
    }
    let diff = start.elapsed();
    println!("bad use time:  {:?}", diff);

    // Good use
    let start = Instant::now();
    // The `collect` is needed to make sure that the closure passed to
    // `map` is called for every iteration before executing the for loop.
    let cookies = names
        .iter()
        .map(|name| conn.intern_atom(false, name.as_bytes()))
        .collect::<Vec<_>>();
    for (i, atom) in cookies.into_iter().enumerate() {
        atoms[i] = atom?.reply()?.atom;
    }
    let diff2 = start.elapsed();
    println!("good use time: {:?}", diff2);
    println!(
        "ratio:         {:?}",
        diff.as_nanos() as f64 / diff2.as_nanos() as f64
    );

    Ok(())
}

//
//     The Graphic Context
//     -------------------
//
// When we perform various drawing operations (graphics, text, etc), we may specify various options
// for controlling how the data will be drawn (what foreground and background colors to use, how
// line edges will be connected, what font to use when drawing some text, etc). In order to avoid
// the need to supply hundreds of parameters to each drawing function, a graphical context
// structure is used. We set the various drawing options in this structure, and then we pass a
// pointer to this structure to any drawing routines. This is rather handy, as we often need to
// perform several drawing requests with the same options. Thus, we would initialize a graphical
// context, set the desired options, and pass this structure to all drawing functions.
//
// Note that graphic contexts have no client-side structure in XCB, they're just XIDs. Xlib has a
// client-side structure because it caches the GC contents so it can avoid making redundant
// requests, but of course XCB doesn't do that.
//
//
//     Events
//     ------
//
// A structure is used to pass events received from the X server. XCB supports exactly the events
// specified in the protocol (33 events). This structure contains the type of event received
// (including a bit for whether it came from the server or another client), as well as the data
// associated with the event (e.g. position on the screen where the event was generated, mouse
// button associated with the event, region of the screen associated with a "redraw" event, etc).
// The way to read the event's data depends on the event type.
//
//
//     Using XCB-based programs
//     ========================
//
// [This part of the tutorial was not translated. Go read https://www.x.org/releases/X11R7.7/doc/libxcb/tutorial/index.html#use if you want.
//
//
//     Opening and closing the connection to an X server
//     =================================================
//
// An X program first needs to open the connection to the X server. There is a function that opens
// a connection. It requires the display name, or None. In the latter case, the display name will
// be the one in the environment variable DISPLAY.
//
// pub fn connect(dpy_name: Option<&str>) -> Result<([...], usize), [...]>;
//
// The second tuple value provides the screen number used for the connection. The returned
// structure describes an X11 connection and is opaque. Here is how the connection can be opened:

fn example2() -> Result<(), Box<dyn Error>> {
    let (conn, _screen) = x11rb::connect(None)?;

    // To close a connection, it suffices to drop the connection object

    drop(conn);

    Ok(())
}

//     Checking basic information about a connection
//     ---------------------------------------------
//
// Once we have opened a connection to an X server, we should check some basic information about
// it: what screens it has, what is the size (width and height) of the screen, how many colors it
// supports (black and white ? grey scale ?, 256 colors ? more ?), and so on. We get such
// information from the x11rb::xproto::Screen structure:

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct RenamedScreen {
    pub root: u32,
    pub default_colormap: u32,
    pub white_pixel: u32,
    pub black_pixel: u32,
    pub current_input_masks: u32,
    pub width_in_pixels: u16,
    pub height_in_pixels: u16,
    pub width_in_millimeters: u16,
    pub height_in_millimeters: u16,
    pub min_installed_maps: u16,
    pub max_installed_maps: u16,
    pub root_visual: u32,
    pub backing_stores: u8,
    pub save_unders: u8,
    pub root_depth: u8,
    pub allowed_depths: Vec<Depth>,
}

// Here is a small program that shows how to use get this struct:

fn example3() -> Result<(), Box<dyn Error>> {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None)?;

    // Get the screen #screen_num
    let screen = &conn.setup().roots[screen_num];

    println!();
    println!("Informations of screen {}:", screen.root);
    println!("  width.........: {}", screen.width_in_pixels);
    println!("  height........: {}", screen.height_in_pixels);
    println!("  white pixel...: {}", screen.white_pixel);
    println!("  black pixel...: {}", screen.black_pixel);
    println!();
    Ok(())
}

//     Creating a basic window - the "hello world" program
//     ===================================================
//
// After we got some basic information about our screen, we can create our first window. In the X
// Window System, a window is characterized by an Id. So, in XCB, a window is of type:
//

#[allow(unused)]
pub type Window = u32;

//  We first ask for a new Id for our window, with this function:
//
// trait Connection {
//    fn generate_id(&self) -> u32;
// }
//
// Then, XCB supplies the following function to create new windows:

#[allow(unused, clippy::too_many_arguments)]
fn own_create_window<A: Connection, B>(
    c: &A,             // The connection to use
    depth: u8,         // Depth of the screen
    wid: u32,          // Id of the window
    parent: u32,       // Id of an existing window that should be the parent of the new window
    x: i16,            // X position of the top-left corner of the window (in pixels)
    y: i16,            // Y position of the top-left corner of the window (in pixels)
    width: u16,        // Width of the window (in pixels)
    height: u16,       // Height of the window (in pixels)
    border_width: u16, // Width of the window's border (in pixels)
    class: B,
    visual: u32,
    value_list: &CreateWindowAux,
) -> Result<SequenceNumber, ConnectionError>
where
    B: Into<u16>,
{
    unimplemented!();
}

// The fact that we created the window does not mean that it will be drawn on screen. By default,
// newly created windows are not mapped on the screen (they are invisible). In order to make our
// window visible, we use the function `map_window()`, whose prototype is
//
//   fn map_window(&self, window: u32) -> Result<SequenceNumber, ConnectionError>;
//
// Finally, here is a small program to create a window of size 150x150 pixels, positioned at the top-left corner of the screen:

fn example4() -> Result<(), Box<dyn Error>> {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None)?;

    // Get the screen #screen_num
    let screen = &conn.setup().roots[screen_num];

    // Ask for our window's Id
    let win = conn.generate_id()?;

    // Create the window
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,    // depth (same as root)
        win,                       // window Id
        screen.root,               // parent window
        0,                         // x
        0,                         // y
        150,                       // width
        150,                       // height
        10,                        // border width
        WindowClass::INPUT_OUTPUT, // class
        screen.root_visual,        // visual
        &Default::default(),
    )?; // masks, not used yet

    // Map the window on the screen
    conn.map_window(win)?;

    // Make sure commands are sent before the sleep, so window is shown
    conn.flush()?;

    std::thread::sleep(std::time::Duration::from_secs(10));

    Ok(())
}

// In this code, you see one more function - flush(), not explained yet. It is used to flush
// all the pending requests. More precisely, there are 2 functions that do such things. The first
// one is flush():
//
// trait Connection {
//     fn flush(&self) -> Result<(), ConnectionError>;
// }
//
// This function flushes all pending requests to the X server (much like the fflush() function is
// used to flush standard output). The second function is xcb_aux_sync() / sync():
//
// trait ConnectionExt {
//     fn sync(&self) -> Result<(), ReplyError>;
// }
//
// This functions also flushes all pending requests to the X server, and then waits until the X
// server finishing processing these requests. In a normal program, this will not be necessary
// (we'll see why when we get to write a normal X program), but for now, we put it there.
//
// The window that is created by the above code has a non defined background. This one can be set
// to a specific color, thanks to the two last parameters of `xcb_create_window()`, which are not
// described yet. See the subsections [Configuring a window] or [Registering for event types using
// event masks] for examples on how to use these parameters. In addition, as no events are handled,
// you have to make a Ctrl-C to interrupt the program.
//
//
//     Drawing in a window
//     ===================
//
// Drawing in a window can be done using various graphical functions (drawing pixels, lines,
// rectangles, etc). In order to draw in a window, we first need to define various general drawing
// parameters (what line width to use, which color to draw with, etc). This is done using a
// graphical context.
//
//
//     Allocating a Graphics Context
//     -----------------------------
//
// As we said, a graphical context defines several attributes to be used with the various drawing
// functions. For this, we define a graphical context. We can use more than one graphical context
// with a single window, in order to draw in multiple styles (different colors, different line
// widths, etc). In XCB, a Graphics Context is, as a window, characterized by an Id:
//
//      pub type Gcontext = u32;
//
// We first ask the X server to attribute an Id to our graphic context with this function:
//
// trait Connection {
//    fn generate_id(&self) -> u32;
// }
//
// Then, we set the attributes of the graphic context with this function:
//

#[allow(unused)]
fn my_create_gc<A: Connection>(
    c: &A,
    cid: u32,
    drawable: u32,
    value_list: &CreateGCAux,
) -> Result<SequenceNumber, ConnectionError> {
    unimplemented!();
}

// The CreateGCAux parameter of this function is specific to x11rb. It contains all the optional
// arguments of the request. It is defined like this:

#[allow(unused)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RenamedCreateGcAux {
    pub function: Option<u32>,
    pub plane_mask: Option<u32>,
    pub foreground: Option<u32>,
    pub background: Option<u32>,
    pub line_width: Option<u32>,
    pub line_style: Option<u32>,
    pub cap_style: Option<u32>,
    pub join_style: Option<u32>,
    pub fill_style: Option<u32>,
    pub fill_rule: Option<u32>,
    pub tile: Option<u32>,
    pub stipple: Option<u32>,
    pub tile_stipple_x_origin: Option<i32>,
    pub tile_stipple_y_origin: Option<i32>,
    pub font: Option<u32>,
    pub subwindow_mode: Option<u32>,
    pub graphics_exposures: Option<u32>,
    pub clip_x_origin: Option<i32>,
    pub clip_y_origin: Option<i32>,
    pub clip_mask: Option<u32>,
    pub dash_offset: Option<u32>,
    pub dashes: Option<u32>,
    pub arc_mode: Option<u32>,
}

// We give now an example on how to allocate a graphic context that specifies that each drawing
// function that uses it will draw in foreground with a black color.

fn example5() -> Result<(), Box<dyn Error>> {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None)?;

    // Get the screen #screen_num
    let screen = &conn.setup().roots[screen_num];

    // Create a black graphic context for drawing in the foreground.
    let win = screen.root;
    let black = conn.generate_id()?;
    let values = CreateGCAux::default().foreground(screen.black_pixel);
    conn.create_gc(black, win, &values)?;

    Ok(())
}

// [The following is a bit XCB-specific, because CreateGCAux does not exist in libxcb.]
//
// Note should be taken regarding the role of "value_mask" and "value_list" in the prototype of
// `xcb_create_gc()`. Since a graphic context has many attributes, and since we often just want to
// define a few of them, we need to be able to tell the `xcb_create_gc()` which attributes we want
// to set. This is what the "value_mask" parameter is for. We then use the "value_list" parameter
// to specify actual values for the attribute we defined in "value_mask". Thus, for each constant
// used in "value_list", we will use the matching constant in "value_mask". In this case, we define
// a graphic context with one attribute: when drawing (a point, a line, etc), the foreground color
// will be black. The rest of the attributes of this graphic context will be set to their default
// values.
//
// See the next Subsection for more details.
//
//     Changing the attributes of a Graphics Context
//     ---------------------------------------------
//
// Once we have allocated a Graphic Context, we may need to change its attributes (for example,
// changing the foreground color we use to draw a line, or changing the attributes of the font we
// use to display strings. See Subsections Drawing with a color and [Assigning a Font to a Graphic
// Context]. This is done by using this function:
//
//   fn change_gc(&self, gc: u32, value_list: &ChangeGCAux) -> Result<SequenceNumber, ConnectionError>;
//
// [Some more XCB-specific explanations skipped]
//
//     Drawing primitives: point, line, box, circle,...
//     ------------------------------------------------
//
// After we have created a Graphic Context, we can draw on a window using this Graphic Context,
// with a set of XCB functions, collectively called "drawing primitives". Let see how they are
// used.
//
// To draw a point, or several points, we use
//
//    fn poly_point<B>(&self, coordinate_mode: B, drawable: u32, gc: u32,
//                     points: &[Point]) -> Result<SequenceNumber, ConnectionError>
//    where B: Into<u8>;
//
// The `coordinate_mode` parameter specifies the coordinate mode. Available values are
//
//    pub enum CoordMode {
//        Origin,
//        Previous,
//    }
//
// If XCB_COORD_MODE_PREVIOUS is used, then all points but the first one are relative to the
// immediately previous point.
//
// The `Point` type is just a structure with two fields (the coordinates of the point):
//
//    #[derive(Debug, Clone, Copy)]
//    pub struct Point {
//        pub x: i16,
//        pub y: i16,
//    }
//
// You could see an example in xpoints.c. **TODO** Set the link.
//
// To draw a line, or a polygonal line, we use
//
//     fn poly_line<A>(&self, coordinate_mode: A, drawable: u32, gc: u32, points: &[Point])
//     -> Result<SequenceNumber, ConnectionError>
//     where A: Into<u8>
//
// This function will draw the line between the first and the second points, then the line between
// the second and the third points, and so on.
//
// To draw a segment, or several segments, we use
//
//     fn poly_segment(&self, drawable: u32, gc: u32, segments: &[Segment]) -> Result<SequenceNumber, ConnectionError>;
//
// The `xcb_segment_t` type is just a structure with four fields (the coordinates of the two points that define the segment):
//
//    pub struct Segment {
//        pub x1: i16,
//        pub y1: i16,
//        pub x2: i16,
//        pub y2: i16,
//    }
//
// To draw a rectangle, or several rectangles, we use
//
//     fn poly_rectangle(&self, drawable: u32, gc: u32, rectangles: &[Rectangle]) -> Result<SequenceNumber, ConnectionError>;
//
// The `xcb_rectangle_t` type is just a structure with four fields (the coordinates of the top-left
// corner of the rectangle, and its width and height):
//
//    pub struct Rectangle {
//        pub x: i16,
//        pub y: i16,
//        pub width: u16,
//        pub height: u16,
//    }
//
// To draw an elliptical arc, or several elliptical arcs, we use
//
//     fn poly_arc(&self, drawable: u32, gc: u32, arcs: &[Arc]) -> Result<SequenceNumber, ConnectionError>;
//
// The `xcb_arc_t` type is a structure with six fields:
//
//    pub struct Arc {
//        pub x: i16,
//        pub y: i16,
//        pub width: u16,
//        pub height: u16,
//        pub angle1: i16,
//        pub angle2: i16,
//    }
//
// Note: the angles are expressed in units of 1/64 of a degree, so to have an angle of 90 degrees,
// starting at 0,`angle1 = 0` and `angle2 = 90 << 6`. Positive angles indicate counterclockwise
// motion, while negative angles indicate clockwise motion.
//
// The corresponding function which fill inside the geometrical object are listed below, without
// further explanation, as they are used as the above functions.
//
// To Fill a polygon defined by the points given as arguments , we use
//
//  fn fill_poly<A, B>(&self, drawable: u32, gc: u32, shape: A, coordinate_mode: B, points: &[Point]) -> Result<SequenceNumber, ConnectionError>
//  where A: Into<u8>, B: Into<u8>
//
// The `shape` parameter specifies a shape that helps the server to improve performance. Available values are
//
//    pub enum PolyShape {
//        Complex,
//        Nonconvex,
//        Convex,
//    }
//
// To fill one or several rectangles, we use
//
//    fn poly_fill_rectangle(&self, drawable: u32, gc: u32, rectangles: &[Rectangle]) -> Result<SequenceNumber, ConnectionError>;
//
// To fill one or several arcs, we use
//
//    fn poly_fill_arc(&self, drawable: u32, gc: u32, arcs: &[Arc]) -> Result<SequenceNumber, ConnectionError>;
//
// To illustrate these functions, here is an example that draws four points, a polygonal line, two
// segments, two rectangles and two arcs. Remark that we use events for the first time, as an
// introduction to the next section.

fn example6() -> Result<(), Box<dyn Error>> {
    // geometric objects
    let points = [
        Point { x: 10, y: 10 },
        Point { x: 10, y: 20 },
        Point { x: 20, y: 10 },
        Point { x: 20, y: 20 },
    ];
    let polyline = [
        Point { x: 50, y: 10 },
        Point { x: 5, y: 20 }, // Rest of points are relative
        Point { x: 25, y: -20 },
        Point { x: 10, y: 10 },
    ];
    let segments = [
        Segment {
            x1: 100,
            y1: 10,
            x2: 140,
            y2: 30,
        },
        Segment {
            x1: 110,
            y1: 25,
            x2: 130,
            y2: 60,
        },
    ];
    let rectangles = [
        Rectangle {
            x: 10,
            y: 50,
            width: 40,
            height: 20,
        },
        Rectangle {
            x: 80,
            y: 50,
            width: 10,
            height: 40,
        },
    ];
    let arcs = [
        Arc {
            x: 10,
            y: 100,
            width: 60,
            height: 40,
            angle1: 0,
            angle2: 90 << 6,
        },
        Arc {
            x: 90,
            y: 100,
            width: 55,
            height: 40,
            angle1: 0,
            angle2: 270 << 6,
        },
    ];

    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None)?;

    // Get the screen #screen_num
    let screen = &conn.setup().roots[screen_num];

    // Create black (foreground) graphic context
    let win = screen.root;

    let foreground = conn.generate_id()?;
    let values = CreateGCAux::default()
        .foreground(screen.black_pixel)
        .graphics_exposures(0);
    conn.create_gc(foreground, win, &values)?;

    // Ask for our window's Id
    let win = conn.generate_id()?;

    // Create the window
    let values = CreateWindowAux::default()
        .background_pixel(screen.white_pixel)
        .event_mask(EventMask::EXPOSURE);
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,    // depth
        win,                       // window Id
        screen.root,               // parent window
        0,                         // x
        0,                         // y
        150,                       // width
        150,                       // height
        10,                        // border_width
        WindowClass::INPUT_OUTPUT, // class
        screen.root_visual,        // visual
        &values,
    )?;

    // Map the window on the screen
    conn.map_window(win)?;

    // We flush the request
    conn.flush()?;

    loop {
        let event = conn.wait_for_event()?;
        if let Event::Expose(_) = event {
            // We draw the points
            conn.poly_point(CoordMode::ORIGIN, win, foreground, &points)?;

            // We draw the polygonal line
            conn.poly_line(CoordMode::PREVIOUS, win, foreground, &polyline)?;

            // We draw the segments
            conn.poly_segment(win, foreground, &segments)?;

            // We draw the rectangles
            conn.poly_rectangle(win, foreground, &rectangles)?;

            // We draw the arcs
            conn.poly_arc(win, foreground, &arcs)?;

            // We flush the request
            conn.flush()?;
        } else {
            // Unknown event type, ignore it
        }
    }
}

//     X Events
//     ========
//
// In an X program, everything is driven by events. Event painting on the screen is sometimes done
// as a response to an event (an `Expose` event). If part of a program's window that was hidden,
// gets exposed (e.g. the window was raised above other widows), the X server will send an "expose"
// event to let the program know it should repaint that part of the window. User input (key
// presses, mouse movement, etc) is also received as a set of events.
//
//
//     Registering for event types using event masks
//     ---------------------------------------------
//
// During the creation of a window, you should give it what kind of events it wishes to receive.
// Thus, you may register for various mouse (also called pointer) events, keyboard events, expose
// events, and so on. This is done for optimizing the server-to-client connection (i.e. why send a
// program (that might even be running at the other side of the globe) an event it is not
// interested in ?)
//
// In XCB, you use the "value_mask" and "value_list" data in the `xcb_create_window()` function to
// register for events. Here is how we register for `Expose` event when creating a window:
//

#[allow(unused)]
fn example_expose<C: Connection>(
    conn: &C,
    depth: u8,
    screen: &Screen,
) -> Result<(), Box<dyn Error>> {
    let values = CreateWindowAux::default().event_mask(EventMask::EXPOSURE);
    let win = conn.generate_id()?;
    conn.create_window(
        depth,
        win,
        screen.root,
        0,
        0,
        150,
        150,
        10,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &values,
    )?;
    Ok(())
}

// `XCB_EVENT_MASK_EXPOSURE` is a constant defined in the xcb_event_mask_t enumeration in the
// "xproto.h" header file.
//
// If we wanted to register for several event types, we can logically "or" them, as follows:

#[allow(unused)]
fn example_or<C: Connection>(conn: &C, depth: u8, screen: &Screen) -> Result<(), Box<dyn Error>> {
    let values =
        CreateWindowAux::default().event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS);
    let win = conn.generate_id()?;
    conn.create_window(
        depth,
        win,
        screen.root,
        0,
        0,
        150,
        150,
        10,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &values,
    )?;
    Ok(())
}

// This registers for `Expose` events as well as for mouse button presses inside the created
// window. You should note that a mask may represent several event sub-types.
//
// The values that a mask could take are given by the `xcb_cw_t` enumeration:
//
//    pub enum CW {
//        BackPixmap,
//        BackPixel,
//        BorderPixmap,
//        BorderPixel,
//        BitGravity,
//        WinGravity,
//        BackingStore,
//        BackingPlanes,
//        BackingPixel,
//        OverrideRedirect,
//        SaveUnder,
//        EventMask,
//        DontPropagate,
//        Colormap,
//        Cursor,
//    }
//
//
// [This note only applies to xcb, not x11rb]
//
// Note: we must be careful when setting the values of the valwin parameter, as they have to follow
// the order the `xcb_cw_t` enumeration. Here is an example:
//
// [example removed since x11rb does not have this problem]
//
// If the window has already been created, we can use the `xcb_change_window_attributes()` function
// to set the events that the window will receive. The subsection Configuring a window shows its
// prototype. As an example, here is a piece of code that configures the window to receive the
// `Expose` and `ButtonPress` events:

#[allow(unused)]
fn example_change_event_mask<C: Connection>(conn: &C, win: Window) -> Result<(), Box<dyn Error>> {
    let values = ChangeWindowAttributesAux::default()
        .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS);
    conn.change_window_attributes(win, &values)?;
    Ok(())
}

// Note: A common bug programmers have is adding code to handle new event types in their program,
// while forgetting to add the masks for these events in the creation of the window. Such a
// programmer would then sit there for hours debugging their program, wondering "Why doesn't my
// program notice that I released the button?", only to find that they registered for button press
// events but not for button release events.
//
//
//     Receiving events: writing the events loop
//     -----------------------------------------
//
// After we have registered for the event types we are interested in, we need to enter a loop of
// receiving events and handling them. There are two ways to receive events: a blocking way and a
// non-blocking way:
//
//  * `xcb_wait_for_event (xcb_connection_t *c)` is the blocking way. It waits (so blocks...)
//    until an event is queued in the X server. Then it retrieves it into a newly allocated
//    structure (it dequeues it from the queue) and returns it. This structure has to be freed. The
//    function returns `NULL` if an error occurs.
//
//  * `xcb_poll_for_event (xcb_connection_t *c, int *error)` is the non-blocking way. It looks at
//    the event queue and returns (and dequeues too) an existing event into a newly allocated
//    structure. This structure has to be freed. It returns `NULL` if there is no event. If an
//    error occurs, the parameter `error` will be filled with the error status.
//
// There are various ways to write such a loop. We present two ways to write such a loop, with the
// two functions above. The first one uses `xcb_wait_for_event_t`, which is similar to an event
// Xlib loop using only `XNextEvent`:

#[allow(unused)]
fn example_wait_for_event<C: Connection>(conn: &C) -> Result<(), Box<dyn Error>> {
    loop {
        let event = conn.wait_for_event()?;
        match event {
            Event::Expose(_event) => {
                // ....
            }
            Event::ButtonPress(_event) => {
                // ....
            }
            _ => {
                // Unknown event type, ignore it
            }
        }
    }

    Ok(())
}

// You will certainly want to use `xcb_poll_for_event(xcb_connection_t *c, int *error)` if, in
// Xlib, you use `XPending` or `XCheckMaskEvent`:
//
//    while (XPending (display)) {
//      XEvent ev;
//
//      XNextEvent(d, &ev);
//
//      /* Manage your event */
//    }
//
// Such a loop in XCB looks like:
//
//    xcb_generic_event_t *ev;
//
//    while ((ev = xcb_poll_for_event (conn, 0))) {
//      /* Manage your event */
//    }
//
// The events are managed in the same way as with `xcb_wait_for_event_t`. Obviously, we will need
// to give the user some way of terminating the program. This is usually done by handling a special
// "quit" event, as we will soon see.
//
//
//     Expose events
//     -------------
//
// The `Expose` event is one of the most basic (and most used) events an application may receive.
// It will be sent to us in one of several cases:
//
// * A window that covered part of our window has moved away, exposing part (or all) of our window.
// * Our window was raised above other windows.
// * Our window mapped for the first time.
// * Our window was de-iconified.
//
// You should note the implicit assumption hidden here: the contents of our window is lost when it
// is being obscured (covered) by either windows. One may wonder why the X server does not save
// this contents. The answer is: to save memory. After all, the number of windows on a display at a
// given time may be very large, and storing the contents of all of them might require a lot of
// memory. Actually, there is a way to tell the X server to store the contents of a window in
// special cases, as we will see later.
//
// When we get an `Expose` event, we should take the event's data from the members of the following
// structure:

#[allow(unused)]
pub struct RenamedExposeEvent {
    /// The Id of the window that receives the event (in case our application
    /// registered for events in several windows)
    pub window: u32,
    /// The x coordinate of the top-left part of the window that needs to be redrawn
    pub x: u16,
    /// The y coordinate of the top-left part of the window that needs to be redrawn
    pub y: u16,
    /// The width of the part of the window that needs to be redrawn
    pub width: u16,
    /// The height of the part of the window that needs to be redrawn
    pub height: u16,
    pub count: u16,
}

//     Getting user input
//     ==================
//
// User input traditionally comes from two sources: the mouse and the keyboard. Various event types
// exist to notify us of user input (a key being presses on the keyboard, a key being released on
// the keyboard, the mouse moving over our window, the mouse entering (or leaving) our window, and
// so on.
//
//
//     Mouse button press and release events
//     -------------------------------------
//
// The first event type we will deal with is a mouse button-press (or button-release) event in our
// window. In order to register to such an event type, we should add one (or more) of the following
// masks when we create our window:
//
// * `XCB_EVENT_MASK_BUTTON_PRESS`: notify us of any button that was pressed in one of our windows.
// * `XCB_EVENT_MASK_BUTTON_RELEASE`: notify us of any button that was released in one of our windows.
//
// The structure to be checked for in our events loop is the same for these two events, and is the
// following:

#[allow(unused)]
pub struct RenamedButtonPressEvent {
    pub detail: u8,
    /// Time, in milliseconds the event took place in
    pub time: u32,
    pub root: u32,
    pub event: u32,
    pub child: u32,
    pub root_x: i16,
    pub root_y: i16,
    /// The x coordinate where the mouse has been pressed in the window
    pub event_x: i16,
    /// The y coordinate where the mouse has been pressed in the window
    pub event_y: i16,
    /// A mask of the buttons (or keys) during the event
    pub state: u16,
    pub same_screen: u8,
}

// The `time` field may be used to calculate "double-click" situations by an application (e.g. if
// the mouse button was clicked two times in a duration shorter than a given amount of time, assume
// this was a double click).
//
// The `state` field is a mask of the buttons held down during the event. It is a bitwise OR of any
// of the following (from the xcb_button_mask_t and xcb_mod_mask_t enumerations):
//
// * `XCB_BUTTON_MASK_1`
// * `XCB_BUTTON_MASK_2`
// * `XCB_BUTTON_MASK_3`
// * `XCB_BUTTON_MASK_4`
// * `XCB_BUTTON_MASK_5`
// * `XCB_MOD_MASK_SHIFT`
// * `XCB_MOD_MASK_LOCK`
// * `XCB_MOD_MASK_CONTROL`
// * `XCB_MOD_MASK_1`
// * `XCB_MOD_MASK_2`
// * `XCB_MOD_MASK_3`
// * `XCB_MOD_MASK_4`
// * `XCB_MOD_MASK_5`
//
// Their names are self explanatory, where the first 5 refer to the mouse buttons that are being
// pressed, while the rest refer to various "special keys" that are being pressed (Mod1 is usually
// the 'Alt' key or the 'Meta' key).
//
// **TODO:** Problem: it seems that the state does not change when clicking with various buttons.
//
//
//     Mouse movement events
//     ---------------------
//
// Similar to mouse button press and release events, we also can be notified of various mouse
// movement events. These can be split into two families. One is of mouse pointer movement while no
// buttons are pressed, and the second is a mouse pointer motion while one (or more) of the buttons
// are pressed (this is sometimes called "a mouse drag operation", or just "dragging"). The
// following event masks may be added during the creation of our window:
//
// * `XCB_EVENT_MASK_POINTER_MOTION`: events of the pointer moving in one of the windows controlled
//                                    by our application, while no mouse button is held pressed.
// * `XCB_EVENT_MASK_BUTTON_MOTION`: Events of the pointer moving while one or more of the mouse
//                                   buttons is held pressed.
// * `XCB_EVENT_MASK_BUTTON_1_MOTION`: same as `XCB_EVENT_MASK_BUTTON_MOTION`, but only when the
//                                     1st mouse button is held pressed.
// * `XCB_EVENT_MASK_BUTTON_2_MOTION`, `XCB_EVENT_MASK_BUTTON_3_MOTION`,
// `XCB_EVENT_MASK_BUTTON_4_MOTION`, `XCB_EVENT_MASK_BUTTON_5_MOTION`: same as
// `XCB_EVENT_MASK_BUTTON_1_MOTION`, but respectively for 2nd, 3rd, 4th and 5th mouse button.
//
// The structure to be checked for in our events loop is the same for these events, and is the
// following:

#[allow(unused)]
pub struct RenamedMotionNotifyEvent {
    pub detail: u8,
    /// Time, in milliseconds the event took place in
    pub time: u32,
    pub root: u32,
    pub event: u32,
    pub child: u32,
    pub root_x: i16,
    pub root_y: i16,
    /// The x coordinate where the mouse has been pressed in the window
    pub event_x: i16,
    /// The y coordinate where the mouse has been pressed in the window
    pub event_y: i16,
    /// A mask of the buttons (or keys) during the event
    pub state: u16,
    pub same_screen: u8,
}

//     Mouse pointer enter and leave events
//     ------------------------------------
//
// Another type of event that applications might be interested in, is a mouse pointer entering a
// window the program controls, or leaving such a window. Some programs use these events to show
// the user that the application is now in focus. In order to register for such an event type, we
// should add one (or more) of the following masks when we create our window:
//
// * `xcb_event_enter_window_t`: notify us when the mouse pointer enters any of our controlled
//                               windows.
// * `xcb_event_leave_window_t`: notify us when the mouse pointer leaves any of our controlled
//                               windows.
//
// The structure to be checked for in our events loop is the same for these two events, and is the
// following:

#[allow(unused)]
pub struct RenamedEnterNotifyEvent {
    pub detail: u8,
    pub time: u32,
    pub root: u32,
    pub event: u32,
    pub child: u32,
    pub root_x: i16,
    pub root_y: i16,
    pub event_x: i16,
    pub event_y: i16,
    pub state: u16,
    pub mode: u8,
    pub same_screen_focus: u8,
}

//     The keyboard focus
//     ------------------
//
// There may be many windows on a screen, but only a single keyboard attached to them. How does the
// X server then know which window should be sent a given keyboard input ? This is done using the
// keyboard focus. Only a single window on the screen may have the keyboard focus at a given time.
// There is a XCB function that allows a program to set the keyboard focus to a given window. The
// user can usually set the keyboard focus using the window manager (often by clicking on the title
// bar of the desired window). Once our window has the keyboard focus, every key press or key
// release will cause an event to be sent to our program (if it registered for these event
// types...).
//
//     Keyboard press and release events
//     ---------------------------------
//
// If a window controlled by our program currently holds the keyboard focus, it can receive key
// press and key release events. So, we should add one (or more) of the following masks when we
// create our window:
//
// * `XCB_EVENT_MASK_KEY_PRESS`: notify us when a key was pressed while any of our controlled
//                               windows had the keyboard focus.
// * `XCB_EVENT_MASK_KEY_RELEASE`: notify us when a key was released while any of our controlled
//                                 windows had the keyboard focus.
//
// The structure to be checked for in our events loop is the same for these two events, and is the
// following:

#[allow(unused)]
pub struct RenamedKeyPressEvent {
    pub detail: u8,
    /// Time, in milliseconds the event took place in
    pub time: u32,
    pub root: u32,
    pub event: u32,
    pub child: u32,
    pub root_x: i16,
    pub root_y: i16,
    pub event_x: i16,
    pub event_y: i16,
    pub state: u16,
    pub same_screen: u8,
}

// The `detail` field refers to the physical key on the keyboard.
//
// **TODO:** Talk about getting the ASCII code from the key code.
//
//
//     X events: a complete example
//     ============================
//
// As an example for handling events, we show a program that creates a window, enters an events
// loop and checks for all the events described above, and writes on the terminal the relevant
// characteristics of the event. With this code, it should be easy to add drawing operations, like
// those which have been described above.

fn print_modifiers(mask: x11rb::protocol::xproto::KeyButMask) {
    println!("Modifier mask: {:#?}", mask);
}

fn example7() -> Result<(), Box<dyn Error>> {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None)?;

    // Get the screen #screen_num
    let screen = &conn.setup().roots[screen_num];

    // Ask for our window's Id
    let win = conn.generate_id()?;

    // Create the window
    let values = CreateWindowAux::default()
        .background_pixel(screen.white_pixel)
        .event_mask(
            EventMask::EXPOSURE
                | EventMask::BUTTON_PRESS
                | EventMask::BUTTON_RELEASE
                | EventMask::POINTER_MOTION
                | EventMask::ENTER_WINDOW
                | EventMask::LEAVE_WINDOW
                | EventMask::KEY_PRESS
                | EventMask::KEY_RELEASE,
        );
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,    // depth
        win,                       // window Id
        screen.root,               // parent window
        0,                         // x
        0,                         // y
        150,                       // width
        150,                       // height
        10,                        // border_width
        WindowClass::INPUT_OUTPUT, // class
        screen.root_visual,        // visual
        &values,
    )?;

    // Map the window on the screen
    conn.map_window(win)?;
    conn.flush()?;

    loop {
        let event = conn.wait_for_event()?;
        match event {
            Event::Expose(event) => {
                println!(
                    "Window {} exposed. Region to be redrawn at location ({},{}) with dimensions \
                     ({},{})",
                    event.window, event.x, event.y, event.width, event.height
                );
            }
            Event::ButtonPress(event) => {
                print_modifiers(event.state);
                match event.detail {
                    4 => println!(
                        "Wheel Button up in window {}, at coordinates ({},{})",
                        event.event, event.event_x, event.event_y
                    ),
                    5 => println!(
                        "Wheel Button down in window {}, at coordinates ({},{})",
                        event.event, event.event_x, event.event_y
                    ),
                    _ => println!(
                        "Button {} pressed in window {}, at coordinates ({},{})",
                        event.detail, event.event, event.event_x, event.event_y
                    ),
                }
            }
            Event::ButtonRelease(event) => {
                print_modifiers(event.state);
                println!(
                    "Button {} released in window {}, at coordinates ({},{})",
                    event.detail, event.event, event.event_x, event.event_y
                );
            }
            Event::MotionNotify(event) => {
                println!(
                    "Mouse moved in window {} at coordinates ({},{})",
                    event.event, event.event_x, event.event_y
                );
            }
            Event::EnterNotify(event) => {
                println!(
                    "Mouse entered window {} at coordinates ({},{})",
                    event.event, event.event_x, event.event_y
                );
            }
            Event::LeaveNotify(event) => {
                println!(
                    "Mouse left window {} at coordinates ({},{})",
                    event.event, event.event_x, event.event_y
                );
            }
            Event::KeyPress(event) => {
                print_modifiers(event.state);
                println!("Key pressed in window {}", event.event);
            }
            Event::KeyRelease(event) => {
                print_modifiers(event.state);
                println!("Key released in window {}", event.event);
            }
            _ => {
                // Unknown event type, ignore it
                println!("Unknown event: {:?}", event);
            }
        }
    }
}

//     Handling text and fonts
//
//     =======================
//
// Besides drawing graphics on a window, we often want to draw text. Text strings have two major
// properties: the characters to be drawn and the font with which they are drawn. In order to draw
// text, we need to first request the X server to load a font. We then assign a font to a Graphic
// Context, and finally, we draw the text in a window, using the Graphic Context.
//
//
//     The Font structure
//     ------------------
//
// In order to support flexible fonts, a font type is defined. You know what ? It's an Id:
//
//   pub type Font = u32;
//
// It is used to contain information about a font, and is passed to several functions that handle
// fonts selection and text drawing. We ask the X server to attribute an Id to our font with the
// function:
//
//    conn.generate_id();
//
//
//     Opening a Font
//     --------------
//
// To open a font, we use the following function:
//
//   pub fn open_font(&self, fid: u32, name: &[u8]) -> Result<SequenceNumber, ConnectionError>;
//
// The `fid` parameter is the font Id defined by `xcb_generate_id()` (see above). The `name`
// parameter is the name of the font you want to open. Use the command `xlsfonts` in a terminal to
// know which are the fonts available on your computer. The parameter `name_len` is the length of
// the name of the font (given by `strlen()`).
//
//
//     Assigning a Font to a Graphic Context
//     -------------------------------------
//
// Once a font is opened, you have to create a Graphic Context that will contain the informations
// about the color of the foreground and the background used when you draw a text in a Drawable.
// Here is an example of a Graphic Context that will allow us to draw an opened font with a black
// foreground and a white background:

#[allow(unused)]
fn example_assign_font<C: Connection>(
    conn: &C,
    screen: &Screen,
    window: Window,
    font: Font,
) -> Result<(), Box<dyn Error>> {
    let gc = conn.generate_id()?;
    let values = CreateGCAux::default()
        .foreground(screen.black_pixel)
        .background(screen.white_pixel)
        .font(font);
    conn.create_gc(gc, window, &values)?;

    // The font is not needed anymore, so we close it
    conn.close_font(font)?;

    Ok(())
}

//     Drawing text in a drawable
//     --------------------------
//
// To draw a text in a drawable, we use the following function:
//
//    pub fn image_text8(&self, drawable: u32, gc: u32, x: i16, y: i16, string: &[u8])
//    -> Result<SequenceNumber, ConnectionError>;
//
// The `string` parameter is the text to draw. The location of the drawing is given by the
// parameters `x` and `y`. The base line of the text is exactly the parameter `y`.
//
//
//     Complete example
//     ----------------
//
// This example draw a text at 10 pixels (for the base line) of the bottom of a window. Pressing
// the Esc key exits the program.
//
// (This whole example uses checked requests in the original, but that does not really seem useful
// to me, so I changed it.)

fn text_draw<C: Connection>(
    conn: &C,
    screen: &Screen,
    window: Window,
    x1: i16,
    y1: i16,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    let gc = gc_font_get(conn, screen, window, "7x13")?;

    conn.image_text8(window, gc, x1, y1, label.as_bytes())?;
    conn.free_gc(gc)?;

    Ok(())
}

fn gc_font_get<C: Connection>(
    conn: &C,
    screen: &Screen,
    window: Window,
    font_name: &str,
) -> Result<Gcontext, ReplyOrIdError> {
    let font = conn.generate_id()?;

    conn.open_font(font, font_name.as_bytes())?;

    let gc = conn.generate_id()?;
    let values = CreateGCAux::default()
        .foreground(screen.black_pixel)
        .background(screen.white_pixel)
        .font(font);
    conn.create_gc(gc, window, &values)?;

    conn.close_font(font)?;

    Ok(gc)
}

fn example8() -> Result<(), Box<dyn Error>> {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None)?;

    // Get the screen #screen_num
    let screen = &conn.setup().roots[screen_num];

    const WIDTH: u16 = 300;
    const HEIGHT: u16 = 100;

    // Creating the window
    let window = conn.generate_id()?;
    let values = CreateWindowAux::default()
        .background_pixel(screen.white_pixel)
        .event_mask(EventMask::KEY_RELEASE | EventMask::EXPOSURE);
    conn.create_window(
        screen.root_depth,
        window,
        screen.root,
        20,
        200,
        WIDTH,
        HEIGHT,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &values,
    )?;
    conn.map_window(window)?;

    conn.flush()?;

    loop {
        let event = conn.wait_for_event()?;
        match event {
            Event::Expose(_) => {
                let text = "Press ESC key to exit...";
                text_draw(&conn, screen, window, 10, HEIGHT as i16 - 10, text)?;
                conn.flush()?;
            }
            Event::KeyRelease(event) => {
                if event.detail == 9 {
                    // ESC
                    return Ok(());
                }
            }
            _ => {} // Unknown event type, ignore it
        }
    }
}

//     Interacting with the window manager
//     ===================================
//
// After we have seen how to create windows and draw on them, we take one step back, and look at
// how our windows are interacting with their environment (the full screen and the other windows).
// First of all, our application needs to interact with the window manager. The window manager is
// responsible to decorating drawn windows (i.e. adding a frame, an iconify button, a system menu,
// a title bar, etc), as well as handling icons shown when windows are being iconified. It also
// handles ordering of windows on the screen, and other administrative tasks. We need to give it
// various hints as to how we want it to treat our application's windows.
//
//
//     Window properties
//     -----------------
//
// Many of the parameters communicated to the window manager are passed using data called
// "properties". These properties are attached by the X server to different windows, and are stored
// in a format that makes it possible to read them from different machines that may use different
// architectures (remember that an X client program may run on a remote machine).
//
// The property and its type (a string, an integer, etc) are Id. Their type are `xcb_atom_t`:
//
//    pub type ATOM = u32;
//
// To change the property of a window, we use one of the following functions:
//
//    fn change_property8<A, B, C>(
//        &self,
//        mode: A,
//        window: Window,
//        property: B,
//        type_: C,
//        data: &[u8],
//    ) -> Result<VoidCookie<'_, Self>, ConnectionError>
//    where
//        A: Into<u8>,
//        B: Into<ATOM>,
//        C: Into<ATOM>
//
//    fn change_property16<A, B, C>(
//        &self,
//        mode: A,
//        window: Window,
//        property: B,
//        type_: C,
//        data: &[u16],
//    ) -> Result<VoidCookie<'_, Self>, ConnectionError>
//    where
//        A: Into<u8>,
//        B: Into<ATOM>,
//        C: Into<ATOM>
//
//    fn change_property32<A, B, C>(
//        &self,
//        mode: A,
//        window: Window,
//        property: B,
//        type_: C,
//        data: &[u32],
//    ) -> Result<VoidCookie<'_, Self>, ConnectionError>
//    where
//        A: Into<u8>,
//        B: Into<ATOM>,
//        C: Into<ATOM>
//
// The `mode` parameter could be one of the following values (defined in enumeration
// xcb_prop_mode_t in the xproto.h header file):
//
//    pub enum PropMode {
//        Replace,
//        Prepend,
//        Append,
//    }
//
//
//
//     Setting the window name and icon name
//     -------------------------------------
//
// The first thing we want to do would be to set the name for our window. This is done using the
// `xcb_change_property()` function. This name may be used by the window manager as the title of
// the window (in the title bar), in a task list, etc. The property atom to use to set the name of
// a window is `WM_NAME` (and `WM_ICON_NAME` for the iconified window) and its type is `STRING`.
// Here is an example of utilization:

fn example9() -> Result<(), Box<dyn Error>> {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None)?;

    // Get the screen #screen_num
    let screen = &conn.setup().roots[screen_num];

    // Ask for our window's Id
    let win = conn.generate_id()?;

    // Create the window
    conn.create_window(
        0,
        win,
        screen.root,
        0,
        0,
        250,
        150,
        10,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &Default::default(),
    )?;

    // Set the title of the window
    let title = "Hello World !";
    conn.change_property8(
        PropMode::REPLACE,
        win,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        title.as_bytes(),
    )?;

    // Set the title of the window icon
    let title_icon = "Hello World ! (iconified)";
    conn.change_property8(
        PropMode::REPLACE,
        win,
        AtomEnum::WM_ICON_NAME,
        AtomEnum::STRING,
        title_icon.as_bytes(),
    )?;

    // Map the window on the screen
    conn.map_window(win)?;

    conn.flush()?;

    loop {
        conn.wait_for_event()?;
    }
}

//     Simple window operations
//     ========================
//
// One more thing we can do to our window is manipulate them on the screen (resize them, move them,
// raise or lower them, iconify them, and so on). Some window operations functions are supplied by
// XCB for this purpose.
//
//
//     Mapping and un-mapping a window
//     -------------------------------
//
// The first pair of operations we can apply on a window is mapping it, or un-mapping it. Mapping a
// window causes the window to appear on the screen, as we have seen in our simple window program
// example. Un-mapping it causes it to be removed from the screen (although the window as a logical
// entity still exists). This gives the effect of making a window hidden (unmapped) and shown again
// (mapped). For example, if we have a dialog box window in our program, instead of creating it
// every time the user asks to open it, we can create the window once, in an un-mapped mode, and
// when the user asks to open it, we simply map the window on the screen. When the user clicked the
// 'OK' or 'Cancel' button, we simply un-map the window. This is much faster than creating and
// destroying the window, however, the cost is wasted resources, both on the client side, and on
// the X server side.
//
// To map a window, you use the following function:
//
//     fn map_window(&self, window: u32) -> Result<SequenceNumber, ConnectionError>;
//
// To have a simple example, see the examples above. The mapping operation will cause an `Expose`
// event to be sent to our application, unless the window is completely covered by other windows.
//
// Un-mapping a window is also simple. You use the function
//
//    fn unmap_window(&self, window: u32) -> Result<SequenceNumber, ConnectionError>;
//
// The utilization of this function is the same as `xcb_map_window()`.
//
//
//     Configuring a window
//     --------------------
//
// As we have seen when we have created our first window, in the X Events subsection, we can set
// some attributes for the window (that is, the position, the size, the events the window will
// receive, etc). If we want to modify them, but the window is already created, we can change them
// by using the following function:
//
//     fn configure_window(&self, window: u32, value_list: &ConfigureWindowAux) -> Result<SequenceNumber, ConnectionError>;
//
// We set the `value_mask` to one or several mask values that are in the xcb_config_window_t enumeration in the xproto.h header:
//
// * `XCB_CONFIG_WINDOW_X`: new x coordinate of the window's top left corner
// * `XCB_CONFIG_WINDOW_Y`: new y coordinate of the window's top left corner
// * `XCB_CONFIG_WINDOW_WIDTH`: new width of the window
// * `XCB_CONFIG_WINDOW_HEIGHT`: new height of the window
// * `XCB_CONFIG_WINDOW_BORDER_WIDTH`: new width of the border of the window
// * `XCB_CONFIG_WINDOW_SIBLING`
// * `XCB_CONFIG_WINDOW_STACK_MODE`: the new stacking order
//
// We then give to `value_mask` the new value. We now describe how to use `xcb_configure_window_t`
// in some useful situations.
//
//
//     Moving a window around the screen
//     ---------------------------------
//
// An operation we might want to do with windows is to move them to a different location. This can
// be done like this:

#[allow(unused)]
fn example_move<C: Connection>(conn: &C, win: Window) -> Result<(), ReplyError> {
    // Move the window to coordinates x = 10 and y = 20
    let values = ConfigureWindowAux::default().x(10).y(20);
    conn.configure_window(win, &values)?;
    Ok(())
}

// Note that when the window is moved, it might get partially exposed or partially hidden by other
// windows, and thus we might get `Expose` events due to this operation.
//
//
//     Resizing a window
//     -----------------
//
// Yet another operation we can do is to change the size of a window. This is done using the
// following code:

#[allow(unused)]
fn example_resize<C: Connection>(conn: &C, win: Window) -> Result<(), ReplyError> {
    // Move the window to coordinates width = 10 and height = 20
    let values = ConfigureWindowAux::default().width(10).height(20);
    conn.configure_window(win, &values)?;
    Ok(())
}

// We can also combine the move and resize operations using one single call to
// `xcb_configure_window_t`:

#[allow(unused)]
fn example_move_resize<C: Connection>(conn: &C, win: Window) -> Result<(), ReplyError> {
    // Move the window to coordinates x = 10 and y = 20
    // and resize the window to width = 200 and height = 300
    let values = ConfigureWindowAux::default()
        .x(10)
        .y(20)
        .width(200)
        .height(300);
    conn.configure_window(win, &values)?;
    Ok(())
}

//     Changing windows stacking order: raise and lower
//     ------------------------------------------------
//
// Until now, we changed properties of a single window. We'll see that there are properties that
// relate to the window and other windows. One of them is the stacking order. That is, the order in
// which the windows are layered on top of each other. The front-most window is said to be on the
// top of the stack, while the back-most window is at the bottom of the stack. Here is how to
// manipulate our windows stack order:

#[allow(unused)]
fn example_stack_above<C: Connection>(conn: &C, win: Window) -> Result<(), ReplyError> {
    // Move the window on the top of the stack
    let values = ConfigureWindowAux::default().stack_mode(StackMode::ABOVE);
    conn.configure_window(win, &values)?;
    Ok(())
}

#[allow(unused)]
fn example_stack_below<C: Connection>(conn: &C, win: Window) -> Result<(), ReplyError> {
    // Move the window to the bottom of the stack
    let values = ConfigureWindowAux::default().stack_mode(StackMode::BELOW);
    conn.configure_window(win, &values)?;
    Ok(())
}

//     Getting information about a window
//     ----------------------------------
//
// Just like we can set various attributes of our windows, we can also ask the X server supply the
// current values of these attributes. For example, we can check where a window is located on the
// screen, what is its current size, whether it is mapped or not, etc. The structure that contains
// some of this information is

pub struct RenamedGetGeometryReply {
    pub depth: u8,         // depth of the window
    pub root: u32,         // Id of the root window
    pub x: i16,            // x coordinate of the window's location
    pub y: i16,            // Y coordinate of the window's location
    pub width: u16,        // Width of the window
    pub height: u16,       // Height of the window
    pub border_width: u16, // Width of the window's border
}

// x11rb fills this structure with two functions:
//
//     fn get_geometry(&self, drawable: u32) -> Result<Cookie<A, GetGeometryReply>, ConnectionError>;
//
// and the .reply() function on the returned cookie
//
// You use them as follows:

#[allow(unused)]
fn example_get_geometry<C: Connection>(conn: &C, win: Window) -> Result<(), ReplyError> {
    let geom = conn.get_geometry(win)?.reply()?;

    // Do something with the fields of geom

    Ok(())
}

// Remark that you have to free the structure, as `xcb_get_geometry_reply_t` allocates a newly one.
//
// [Also remark that with x11rb and rust, you do not have to free the structure yourself]
//
// One problem is that the returned location of the window is relative to its parent window. This
// makes these coordinates rather useless for any window manipulation functions, like moving it on
// the screen. In order to overcome this problem, we need to take a two-step operation. First, we
// find out the Id of the parent window of our window. We then translate the above relative
// coordinates to the screen coordinates.
//
// To get the Id of the parent window, we need this structure:
//
//    pub struct QueryTreeReply {
//        pub root: u32,
//        pub parent: u32,
//        pub children: Vec<u32>,
//    }
//
// x11rb fills this structure with two functions:
//
//    fn query_tree(&self, window: u32) -> Result<Cookie<A, QueryTreeReply>, ConnectionError>;
//
// and the .reply() function on the returned cookie
//
// The translated coordinates will be found in this structure:
//
//    pub struct TranslateCoordinatesReply {
//        pub same_screen: u8,
//        pub child: u32,
//        pub dst_x: i16,
//        pub dst_y: i16,
//    }
//
// As usual, we need two functions to fill this structure:
//
//    fn translate_coordinates(&self, src_window: u32, dst_window: u32, src_x: i16, src_y: i16)
//    -> Result<Cookie<Self, TranslateCoordinatesReply>, ConnectionError>
//
// and the .reply() function on the returned cookie
//
// We use them as follows:

#[allow(unused)]
fn example_get_and_query<C: Connection>(conn: &C, win: Window) -> Result<(), ReplyError> {
    let geom = conn.get_geometry(win)?;
    let tree = conn.query_tree(win)?;

    let geom = geom.reply()?;
    let tree = tree.reply()?;

    let trans = conn
        .translate_coordinates(win, tree.parent, geom.x, geom.y)?
        .reply()?;

    // the translated coordinates are in trans.dst_x and trans.dst_y

    Ok(())
}

// Of course, as for `geom`, `tree` and `trans` have to be freed.
// [But not in rust / x11rb]
//
// The work is a bit hard, but XCB is a very low-level library.
//
// **TODO:** the utilization of these functions should be a prog, which displays the coordinates of the window.
//
// There is another structure that gives informations about our window:
//
//    pub struct GetWindowAttributesReply {
//        pub backing_store: u8,
//        pub visual: u32,
//        pub class: u16,
//        pub bit_gravity: u8,
//        pub win_gravity: u8,
//        pub backing_planes: u32,
//        pub backing_pixel: u32,
//        pub save_under: u8,
//        pub map_is_installed: u8,
//        pub map_state: u8,
//        pub override_redirect: u8,
//        pub colormap: u32,
//        pub all_event_masks: u32,
//        pub your_event_mask: u32,
//        pub do_not_propagate_mask: u16,
//    }
//
// XCB supplies these two functions to fill it:
//
//    fn get_window_attributes(&self, window: u32) -> Result<Cookie<A, GetWindowAttributesReply>, ConnectionError>;
//
// and the .reply() function on the returned cookie
//
// You use them as follows:

#[allow(unused)]
fn example_get_attributes<C: Connection>(conn: &C, win: Window) -> Result<(), ReplyError> {
    let geom = conn.get_window_attributes(win)?.reply()?;

    // Do something with the fields of attr

    Ok(())
}

//     Using colors to paint the rainbow
//     =================================
//
// Up until now, all our painting operation were done using black and white. We will (finally) see
// now how to draw using colors.
//
//
//     Color maps
//     ----------
//
// In the beginning, there were not enough colors. Screen controllers could only support a limited
// number of colors simultaneously (initially 2, then 4, 16 and 256). Because of this, an
// application could not just ask to draw in a "light purple-red" color, and expect that color to
// be available. Each application allocated the colors it needed, and when all the color entries
// (4, 16, 256 colors) were in use, the next color allocation would fail.
//
// Thus, the notion of "a color map" was introduced. A color map is a table whose size is the same
// as the number of simultaneous colors a given screen controller. Each entry contained the RGB
// (Red, Green and Blue) values of a different color (all colors can be drawn using some
// combination of red, green and blue). When an application wants to draw on the screen, it does
// not specify which color to use. Rather, it specifies which color entry of some color map to be
// used during this drawing. Change the value in this color map entry and the drawing will use a
// different color.
//
// In order to be able to draw using colors that got something to do with what the programmer
// intended, color map allocation functions are supplied. You could ask to allocate entry for a
// color with a set of RGB values. If one already existed, you would get its index in the table. If
// none existed, and the table was not full, a new cell would be allocated to contain the given RGB
// values, and its index returned. If the table was full, the procedure would fail. You could then
// ask to get a color map entry with a color that is closest to the one you were asking for. This
// would mean that the actual drawing on the screen would be done using colors similar to what you
// wanted, but not the same.
//
// On today's more modern screens where one runs an X server with support for 16 million colors,
// this limitation looks a little silly, but remember that there are still older computers with
// older graphics cards out there. Using color map, support for these screen becomes transparent to
// you. On a display supporting 16 million colors, any color entry allocation request would
// succeed. On a display supporting a limited number of colors, some color allocation requests
// would return similar colors. It won't look as good, but your application would still work.
//
//
//     Allocating and freeing Color Maps
//     ---------------------------------
//
// When you draw using XCB, you can choose to use the standard color map of the screen your window
// is displayed on, or you can allocate a new color map and apply it to a window. In the latter
// case, each time the mouse moves onto your window, the screen color map will be replaced by your
// window's color map, and you'll see all the other windows on screen change their colors into
// something quite bizarre. In fact, this is the effect you get with X applications that use the
// "-install" command line option.
//
// In XCB, a color map is (as often in X) an Id:
//
//     type COLORMAP = u32;
//
// In order to access the screen's default color map, you just have to retrieve the
// `default_colormap` field of the `xcb_screen_t` structure (see Section [Checking basic
// information about a connection](#screen)):

#[allow(unused)]
fn example_get_colormap<C: Connection>(conn: &C) {
    let screen = &conn.setup().roots[0];
    let _colormap = screen.default_colormap;
}

// This will return the color map used by default on the first screen (again, remember that an X
// server may support several different screens, each of which might have its own resources).
//
// The other option, that of allocating a new colormap, works as follows. We first ask the X server
// to give an Id to our color map, with this function:
//
//    conn.generate_id();
//
// Then, we create the color map with
//
//     fn create_colormap<A>(&self, alloc: A, mid: u32, window: u32, visual: u32)
//     -> Result<SequenceNumber, ConnectionError>
//     where A: Into<u8>
//
// Here is an example of creation of a new color map:

#[allow(unused)]
fn example_create_colormap<C: Connection>(
    conn: &C,
    win: Window,
    screen: &Screen,
) -> Result<(), ReplyOrIdError> {
    let cmap = conn.generate_id()?;
    conn.create_colormap(ColormapAlloc::NONE, cmap, win, screen.root_visual)?;

    Ok(())
}

// Note that the window parameter is only used to allow the X server to create the color map for
// the given screen. We can then use this color map for any window drawn on the same screen.
//
// To free a color map, it suffices to use this function:
//
//     fn free_colormap(&self, cmap: u32) -> Result<SequenceNumber, ConnectionError>;
//
//
//     Allocating and freeing a color entry
//     ------------------------------------
//
// Once we got access to some color map, we can start allocating colors. The informations related
// to a color are stored in the following structure:
//
//   pub struct AllocColorReply {
//       pub red: u16,
//       pub green: u16,
//       pub blue: u16,
//       pub pixel: u32,
//   }
//
// XCB supplies these two functions to fill it:
//
//     fn alloc_color(&self, cmap: u32, red: u16, green: u16, blue: u16) -> Result<Cookie<A, AllocColorReply>, ConnectionError>;
//
// and the .reply() function on the returned cookie
//
// The fuction `xcb_alloc_color()` takes the 3 RGB components as parameters (red, green and blue).
// Here is an example of using these functions:

#[allow(unused)]
fn example_fill_colormap<C: Connection>(
    conn: &C,
    win: Window,
    screen: &Screen,
) -> Result<(), ReplyOrIdError> {
    let cmap = conn.generate_id()?;
    conn.create_colormap(ColormapAlloc::NONE, cmap, win, screen.root_visual)?;
    let _rep = conn.alloc_color(cmap, 65535, 0, 0)?.reply()?;

    // Do something with r.pixel or the components

    Ok(())
}

// As `xcb_alloc_color_reply()` allocates memory, you have to free `rep`.
// [But not in rust / x11rb]
//
// **TODO**: Talk about freeing colors.
//
//
//     X Bitmaps and Pixmaps
//     =====================
//
// One thing many so-called "Multi-Media" applications need to do, is display images. In the X
// world, this is done using bitmaps and pixmaps. We have already seen some usage of them when
// setting an icon for our application. Lets study them further, and see how to draw these images
// inside a window, along side the simple graphics and text we have seen so far.
//
// One thing to note before delving further, is that XCB (nor Xlib) supplies no means of
// manipulating popular image formats, such as gif, png, jpeg or tiff. It is up to the programmer
// (or to higher level graphics libraries) to translate these image formats into formats that the X
// server is familiar with (x bitmaps and x pixmaps).
//
//
//     What is a X Bitmap? An X Pixmap?
//     --------------------------------
//
// An X bitmap is a two-color image stored in a format specific to the X window system. When stored
// in a file, the bitmap data looks like a C source file. It contains variables defining the width
// and the height of the bitmap, an array containing the bit values of the bitmap (the size of the
// array is (width+7)/8*height and the bit and byte order are LSB), and an optional hot-spot
// location (that will be explained later, when discussing mouse cursors).
//
// An X pixmap is a format used to stored images in the memory of an X server. This format can
// store both black and white images (such as x bitmaps) as well as color images. It is the only
// image format supported by the X protocol, and any image to be drawn on screen, should be first
// translated into this format.
//
// In actuality, an X pixmap can be thought of as a window that does not appear on the screen. Many
// graphics operations that work on windows, will also work on pixmaps. Indeed, the type of X
// pixmap in XCB is an Id like a window:
//
//     type PIXMAP = u32;
//
// Like Xlib, there is no difference between a Drawable, a Window or a Pixmap:
//
//     type DRAWABLE = u32;
//
// in order to avoid confusion between a window and a pixmap. The operations that will work the
// same on a window or a pixmap will require a `xcb_drawable_t`
//
// Remark: In Xlib, there is no specific difference between a `Drawable`, a `Pixmap` or a `Window`:
// all are 32 bit long integer. XCB wraps all these different IDs in structures to provide some
// measure of type-safety.
//
//
//     Creating a pixmap
//     -----------------
//
// Sometimes we want to create an un-initialized pixmap, so we can later draw into it. This is
// useful for image drawing programs (creating a new empty canvas will cause the creation of a new
// pixmap on which the drawing can be stored). It is also useful when reading various image
// formats: we load the image data into memory, create a pixmap on the server, and then draw the
// decoded image data onto that pixmap.
//
// To create a new pixmap, we first ask the X server to give an Id to our pixmap, with this
// function:
//
//    conn.generate_id();
//
// Then, XCB supplies the following function to create new pixmaps:
//
//     fn create_pixmap(&self, depth: u8, pid: u32, drawable: u32, width: u16, height: u16) -> Result<SequenceNumber, ConnectionError>
//
// **TODO**: Explain the drawable parameter, and give an example (like xpoints.c)
//
//
//     Drawing a pixmap in a window
//     ----------------------------
//
// Once we got a handle to a pixmap, we can draw it on some window, using the following function:
//
//     fn copy_area(&self, src_drawable: u32, dst_drawable: u32, gc: u32, src_x: i16,
//                  src_y: i16, dst_x: i16, dst_y: i16, width: u16, height: u16)
//     -> Result<SequenceNumber, ConnectionError>;
//
// As you can see, we could copy the whole pixmap, as well as only a given rectangle of the pixmap.
// This is useful to optimize the drawing speed: we could copy only what we have modified in the
// pixmap.
//
// **One important note should be made**: it is possible to create pixmaps with different depths on
// the same screen. When we perform copy operations (a pixmap onto a window, etc), we should make
// sure that both source and target have the same depth. If they have a different depth, the
// operation would fail. The exception to this is if we copy a specific bit plane of the source
// pixmap using the `xcb_copy_plane_t` function. In such an event, we can copy a specific plane to
// the target window (in actuality, setting a specific bit in the color of each pixel copied). This
// can be used to generate strange graphic effects in a window, but that is beyond the scope of
// this tutorial.
//
//
//     Freeing a pixmap
//     ----------------
//
// Finally, when we are done using a given pixmap, we should free it, in order to free resources of
// the X server. This is done using this function:
//
//     fn free_pixmap(&self, pixmap: u32) -> Result<SequenceNumber, ConnectionError>;
//
// Of course, after having freed it, we must not try accessing the pixmap again.
//
// **TODO**: Give an example, or a link to xpoints.c
//
//
//     Messing with the mouse cursor
//     =============================
//
// It it possible to modify the shape of the mouse pointer (also called the X pointer) when in
// certain states, as we often see in programs. For example, a busy application would often display
// the hourglass cursor over its main window, to give the user a visual hint that they should wait.
// Let's see how we can change the mouse cursor of our windows.
//
//
//     Creating and destroying a mouse cursor
//     --------------------------------------
//
// There are two methods for creating cursors. One of them is by using a set of predefined cursors,
// that are supplied by the X server, the other is by using a user-supplied bitmap.
//
// In the first method, we use a special font named "cursor", and the function
// `xcb_create_glyph_cursor`:
//
//     fn create_glyph_cursor(&self, cid: u32, source_font: u32, mask_font: u32,
//                           source_char: u16, mask_char: u16, fore_red: u16, fore_green: u16,
//                           fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16)
//     -> Result<SequenceNumber, ConnectionError>;
//
// **TODO**: Describe `source_char` and `mask_char`, for example by giving an example on how to get
// the values. There is a list there: [X Font Cursors](http://tronche.com/gui/x/xlib/appendix/b/)
//
// So we first open that font (see 'Loading a Font') and create the new cursor. As for
// every X resource, we have to ask for an X id with `xcb_generate_id` first:

#[allow(unused)]
fn example_create_glyph_cursor<C: Connection>(
    conn: &C,
    win: Window,
    screen: &Screen,
) -> Result<(), ReplyOrIdError> {
    let font = conn.generate_id()?;
    conn.open_font(font, b"cursor")?;

    let cursor = conn.generate_id()?;
    conn.create_glyph_cursor(cursor, font, font, 58, 58 + 1, 0, 0, 0, 0, 0, 0)?;

    Ok(())
}

// We have created the cursor "right hand" by specifying 58 to the `source_fon`t argument and 58 +
// 1 to the `mask_font`.
//
// The cursor is destroyed by using the function
//
//     fn free_cursor(&self, cursor: u32) -> Result<SequenceNumber, ConnectionError>;
//
// In the second method, we create a new cursor by using a pair of pixmaps, with depth of one (that
// is, two colors pixmaps). One pixmap defines the shape of the cursor, while the other works as a
// mask, specifying which pixels of the cursor will be actually drawn. The rest of the pixels will
// be transparent.
//
// **TODO**: give an example.
//
//
//     Setting a window's mouse cursor
//     -------------------------------
//
// Once the cursor is created, we can modify the cursor of our window by using
// `xcb_change_window_attributes` and using the `XCB_CWCURSOR` attribute:

#[allow(unused)]
fn example_change_window_cursor<C: Connection>(
    conn: &C,
    win: Window,
    cursor: Cursor,
) -> Result<(), ReplyError> {
    let values = ChangeWindowAttributesAux::default().cursor(cursor);
    conn.change_window_attributes(win, &values)?;

    Ok(())
}

// Of course, the cursor and the font must be freed.
//
//
//     Complete example
//     ----------------
//
// The following example displays a window with a button. When entering the window, the window
// cursor is changed to an arrow. When clicking once on the button, the cursor is changed to a
// hand. When clicking again on the button, the cursor window gets back to the arrow. The Esc key
// exits the application.

fn button_draw<C: Connection>(
    conn: &C,
    screen: &Screen,
    window: Window,
    x1: i16,
    y1: i16,
    label: &str,
) -> Result<(), ReplyOrIdError> {
    let inset = 2;
    let gc = gc_font_get(conn, screen, window, "7x13")?;
    let width = 7 * label.len() + 2 * (inset + 1);
    let height = 13 + 2 * (inset + 1);
    let (width, height) = (width as i16, height as i16);
    let inset = inset as i16;

    let points = [
        Point { x: x1, y: y1 },
        Point {
            x: x1 + width,
            y: y1,
        },
        Point {
            x: x1 + width,
            y: y1 - height,
        },
        Point {
            x: x1,
            y: y1 - height,
        },
        Point { x: x1, y: y1 },
    ];
    conn.poly_line(CoordMode::ORIGIN, window, gc, &points)?;
    conn.image_text8(window, gc, x1 + inset + 1, y1 - inset - 1, label.as_bytes())?;
    conn.free_gc(gc)?;
    Ok(())
}

// text_draw and gc_font_get were already defined above

fn cursor_set<C: Connection>(
    conn: &C,
    screen: &Screen,
    window: Window,
    cursor_id: u16,
) -> Result<(), ReplyOrIdError> {
    let font = conn.generate_id()?;
    conn.open_font(font, b"cursor")?;

    let cursor = conn.generate_id()?;
    conn.create_glyph_cursor(
        cursor,
        font,
        font,
        cursor_id,
        cursor_id + 1,
        0,
        0,
        0,
        0,
        0,
        0,
    )?;

    let gc = conn.generate_id()?;
    let values = CreateGCAux::default()
        .foreground(screen.black_pixel)
        .background(screen.black_pixel)
        .font(font);
    conn.create_gc(gc, window, &values)?;

    let values = ChangeWindowAttributesAux::default().cursor(cursor);
    conn.change_window_attributes(window, &values)?;

    conn.free_cursor(cursor)?;
    conn.close_font(font)?;
    Ok(())
}

fn example10() -> Result<(), Box<dyn Error>> {
    const WIDTH: i16 = 300;
    const HEIGHT: i16 = 300;

    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None)?;

    // Get the screen #screen_num
    let screen = &conn.setup().roots[screen_num];

    // Creating the window
    let window = conn.generate_id()?;
    let values = CreateWindowAux::default()
        .background_pixel(screen.white_pixel)
        .event_mask(EventMask::KEY_RELEASE | EventMask::BUTTON_PRESS | EventMask::EXPOSURE);
    conn.create_window(
        screen.root_depth,
        window,
        screen.root,
        20,
        200,
        WIDTH as u16,
        HEIGHT as u16,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &values,
    )?;
    conn.map_window(window)?;

    cursor_set(&conn, screen, window, 68)?;

    conn.flush()?;

    let mut is_hand = false;

    loop {
        let event = conn.wait_for_event()?;
        match event {
            Event::Expose(_) => {
                let text = "click here to change cursor";
                button_draw(
                    &conn,
                    screen,
                    window,
                    (WIDTH - 7 * text.len() as i16) / 2,
                    (HEIGHT - 16) / 2,
                    text,
                )?;

                let text = "Press ESC key to exit...";
                text_draw(&conn, screen, window, 10, HEIGHT - 10, text)?;
                conn.flush()?;
            }
            Event::ButtonPress(event) => {
                let length = "click here to change cursor".len() as i16;

                if (event.event_x >= (WIDTH - 7 * length) / 2)
                    && (event.event_x <= ((WIDTH - 7 * length) / 2 + 7 * length + 6))
                    && (event.event_y >= (HEIGHT - 16) / 2 - 19)
                    && (event.event_y <= ((HEIGHT - 16) / 2))
                {
                    is_hand = !is_hand;
                }

                if is_hand {
                    cursor_set(&conn, screen, window, 58)?;
                } else {
                    cursor_set(&conn, screen, window, 68)?;
                }
                conn.flush()?;
            }
            Event::KeyRelease(event) => {
                if event.detail == 9 {
                    // ESC
                    return Ok(());
                }
            }
            _ => {} // Unknown event type, ignore it
        }
    }
}

//     Translation of basic Xlib functions and macros
//
// The problem when you want to port an Xlib program to XCB is that you don't know if the Xlib
// function that you want to "translate" is a X Window one or an Xlib macro. In that section, we
// describe a way to translate the usual functions or macros that Xlib provides. It's usually just
// a member of a structure.
//
//
//     Members of the Display structure
//     --------------------------------
//
// In this section, we look at how to translate the macros that return some members of the
// `Display` structure. They are obtained by using a function that requires a `xcb_connection_t *`
// or a member of the `xcb_setup_t` structure (via the function `xcb_get_setup`), or a function
// that requires that structure.
//
//
//     ConnectionNumber
//
// This number is the file descriptor that connects the client to the server. You just have to use
// the trait `std::os::unix::io::AsRawFd`.
//
//
//     DefaultScreen
//
// That number is not stored by XCB. It is returned in the second parameter of the function
// `connect`. Hence, you have to store it yourself if you want to use it. Then, to get the
// `xcb_screen_t` structure, you have to iterate on the screens. The equivalent function of the
// Xlib's `ScreenOfDisplay` function can be found below at ScreenOfDisplay. This is also provided
// in the xcb_aux_t library as `xcb_aux_get_screen()`. OK, here is the small piece of code to get
// that number:

#[allow(unused)]
fn example_get_screen_number() {
    let (conn, screen_num) = x11rb::connect(None).unwrap();

    // screen_num now contains the number of the default screen
}

//     QLength
//
// Not documented yet.
//
// However, this points out a basic difference in philosophy between Xlib and XCB. Xlib has several
// functions for filtering and manipulating the incoming and outgoing X message queues. XCB wishes
// to hide this as much as possible from the user, which allows for more freedom in implementation
// strategies.
//
//
//     ScreenCount
//
// You get the count of screens with the functions `xcb_get_setup` and `xcb_setup_roots_iterator`
// (if you need to iterate):

#[allow(unused)]
fn example_get_screen_count() {
    let (conn, screen_num) = x11rb::connect(None).unwrap();

    let _screen_count = conn.setup().roots.len();

    // screen_num now contains the number of the default screen
}

//     ServerVendor
//
// You get the name of the vendor of the server hardware with the functions `xcb_get_setup` and
// `xcb_setup_vendor`. See the next example.
//
//
//     ProtocolVersion
//
// You get the major version of the protocol in the `xcb_setup_t` structure, with the function
// `xcb_get_setup`. See the next example.
//
//
//     ProtocolRevision
//
// You get the minor version of the protocol in the `xcb_setup_t` structure, with the function
// `xcb_get_setup`. See the next example.
//
//
//     VendorRelease
//
// You get the number of the release of the server hardware in the `xcb_setup_t` structure, with
// the function `xcb_get_setup`. See the next example
//
//
//     DisplayString
//
// The name of the display is not stored in XCB. You have to store it by yourself.
//
//
//     BitmapUnit
//
// You get the bitmap scanline unit in the `xcb_setup_t` structure, with the function
// `xcb_get_setup`. See the next example.
//
//
//     BitmapBitOrder
//
// You get the bitmap bit order in the `xcb_setup_t` structure, with the function `xcb_get_setup`.
// See the next example.
//
//
//     BitmapPad
//
// You get the bitmap scanline pad in the `xcb_setup_t` structure, with the function
// `xcb_get_setup`. See the next example.
//
//
//     ImageByteOrder
//
// You get the image byte order in the `xcb_setup_t` structure, with the function `xcb_get_setup`.
// See the next example.

fn example11() -> Result<(), Box<dyn Error>> {
    let (conn, _) = x11rb::connect(None)?;
    let setup = conn.setup();

    println!(
        "Name of server vendor is {}",
        String::from_utf8_lossy(&setup.vendor)
    );
    println!("Release number is {}", setup.release_number);
    println!(
        "Protocol version is {}.{}",
        setup.protocol_major_version, setup.protocol_minor_version
    );
    println!(
        "Bitmap format scanline unit is {}",
        setup.bitmap_format_scanline_unit
    );
    println!(
        "Bitmap format bit order is {:?}",
        setup.bitmap_format_bit_order
    );
    println!(
        "Bitmap format scanline pad is {}",
        setup.bitmap_format_scanline_pad
    );
    println!("Image byte order is {:?}", setup.image_byte_order);

    Ok(())
}

//     ScreenOfDisplay related functions
//     ---------------------------------
//
// In Xlib, `ScreenOfDisplay` returns a `Screen` structure that contains several characteristics of
// your screen. XCB has a similar structure (`xcb_screen_t`), but the way to obtain it is a bit
// different. With Xlib, you just provide the number of the screen and you grab it from an array.
// With XCB, you iterate over all the screens to obtain the one you want. The complexity of this
// operation is O(n). So the best is to store this structure if you use it often. See
// ScreenOfDisplay just below.
//
// Xlib provides generally two functions to obtain the characteristics related to the screen. One
// with the display and the number of the screen, which calls `ScreenOfDisplay`, and the other that
// uses the `Screen` structure. This might be a bit confusing. As mentioned above, with XCB, it is
// better to store the `xcb_screen_t` structure. Then, you have to read the members of this
// structure. That's why the Xlib functions are put by pairs (or more) as, with XCB, you will use
// the same code.
//
//
//     ScreenOfDisplay
//
// This function returns the Xlib `Screen` structure. With XCB, you iterate over all the screens
// and once you get the one you want, you return it:

#[allow(unused)]
fn example_get_screen<C: Connection>(conn: &C, index: usize) -> &Screen {
    &conn.setup().roots[index]
}

// As mentioned above, you might want to store the value returned by this function.
//
// All the functions below will use the result of that function, as they just grab a specific
// member of the `xcb_screen_t` structure.
//
//
//     DefaultScreenOfDisplay
//
// It is the default screen that you obtain when you connect to the X server. It suffices to call
// the example_get_screen() function above with the connection and the number of the default
// screen.

#[allow(unused)]
fn example_get_screen2<C: Connection>(conn: &C, index: usize) {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None).unwrap();
    let _default_screen = &conn.setup().roots[screen_num];
}

//     RootWindow / RootWindowOfScreen
//
// Just use the .root member of `Screen`.

#[allow(unused)]
fn example_get_root<C: Connection>(conn: &C, index: usize) -> Window {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None).unwrap();
    let default_screen = &conn.setup().roots[screen_num];
    default_screen.root
}

//     DefaultRootWindow
//
// It is the root window of the default screen. So, you call `ScreenOfDisplay` with the default
// screen number and you get the root window as above in example_get_root().
//
//
//     DefaultVisual / DefaultVisualOfScreen
//
// While a Visual is, in Xlib, a structure, in XCB, there are two types: `xcb_visualid_t`, which is
// the Id of the visual, and `xcb_visualtype_t`, which corresponds to the Xlib Visual. To get the
// Id of the visual of a screen, just get the `root_visual` member of a `xcb_screen_t`:

#[allow(unused)]
fn example_get_visual() {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None).unwrap();
    let screen = &conn.setup().roots[screen_num];
    let _root_visual = screen.root_visual;
}

// To get the `xcb_visualtype_t` structure, it's a bit less easy. You have to get the
// `xcb_screen_t` structure that you want, get its `root_visual` member, then iterate over the
// `xcb_depth_t`s and the `xcb_visualtype_t`s, and compare the `xcb_visualid_t` of these
// `xcb_visualtype_t`s: with `root_visual`:

#[allow(unused)]
fn example_get_visual2<C: Connection>(conn: &C, screen_num: usize) {
    // Open the connection to the X server. Use the DISPLAY environment variable.
    let (conn, screen_num) = x11rb::connect(None).unwrap();
    let screen = &conn.setup().roots[screen_num];

    for depth in &screen.allowed_depths {
        for visualtype in &depth.visuals {
            if visualtype.visual_id == screen.root_visual {
                println!("Found: {:?}", visualtype);
            }
        }
    }
}

//     DefaultGC / DefaultGCOfScreen
//
// This default Graphic Context is just a newly created Graphic Context, associated to the root
// window of a `xcb_screen_t`, using the black white pixels of that screen:

#[allow(unused)]
fn example_create_default_gc<C: Connection>(
    conn: &C,
    screen_num: usize,
) -> Result<Gcontext, ReplyOrIdError> {
    let screen = &conn.setup().roots[screen_num];
    let values = CreateGCAux::default()
        .foreground(screen.black_pixel)
        .background(screen.white_pixel);
    let gc = conn.generate_id()?;
    conn.create_gc(gc, screen.root, &values)?;
    Ok(gc)
}

//     BlackPixel / BlackPixelOfScreen
//
// It is the Id of the black pixel, which is in the structure of an `xcb_screen_t`.

#[allow(unused)]
fn example_black_pixel<C: Connection>(conn: &C, screen_num: usize) {
    let _black_pixel = conn.setup().roots[screen_num].black_pixel;
}

//     WhitePixel / WhitePixelOfScreen
//
// It is the Id of the white pixel, which is in the structure of an `xcb_screen_t`.

#[allow(unused)]
fn example_white_pixel<C: Connection>(conn: &C, screen_num: usize) {
    let _white_pixel = conn.setup().roots[screen_num].white_pixel;
}

//     DisplayWidth / WidthOfScreen
//
// It is the width in pixels of the screen that you want, and which is in the structure of the
// corresponding `xcb_screen_t`.

#[allow(unused)]
fn example_display_width<C: Connection>(conn: &C, screen_num: usize) {
    let _width = conn.setup().roots[screen_num].width_in_pixels;
}

//     DisplayHeight / HeightOfScreen
//
// It is the height in pixels of the screen that you want, and which is in the structure of the
// corresponding `xcb_screen_t`.

#[allow(unused)]
fn example_display_height<C: Connection>(conn: &C, screen_num: usize) {
    let _height = conn.setup().roots[screen_num].height_in_pixels;
}

//     DisplayWidthMM / WidthMMOfScreen
//
// It is the width in millimeters of the screen that you want, and which is in the structure of the
// corresponding `xcb_screen_t`.

#[allow(unused)]
fn example_display_width_mm<C: Connection>(conn: &C, screen_num: usize) {
    let screen = &conn.setup().roots[screen_num];
    let _width = screen.width_in_millimeters;
}

//     DisplayHeightMM / HeightMMOfScreen
//
// It is the height in millimeters of the screen that you want, and which is in the structure of
// the corresponding `xcb_screen_t`.

#[allow(unused)]
fn example_display_height_mm<C: Connection>(conn: &C, screen_num: usize) {
    let screen = &conn.setup().roots[screen_num];
    let _height = screen.height_in_millimeters;
}

//     DisplayPlanes / DefaultDepth / DefaultDepthOfScreen / PlanesOfScreen
//
// It is the depth (in bits) of the root window of the screen. You get it from the `xcb_screen_t`
// structure.

#[allow(unused)]
fn example_display_depth<C: Connection>(conn: &C, screen_num: usize) {
    let screen = &conn.setup().roots[screen_num];
    let _depth = screen.root_depth;
}

//     DefaultColormap / DefaultColormapOfScreen
//
// This is the default colormap of the screen (and not the (default) colormap of the default screen
// !). As usual, you get it from the `xcb_screen_t` structure:

#[allow(unused)]
fn example_display_colormap<C: Connection>(conn: &C, screen_num: usize) {
    let screen = &conn.setup().roots[screen_num];
    let _map = screen.default_colormap;
}

//     MinCmapsOfScreen
//
// You get the minimum installed colormaps in the `xcb_screen_t` structure:

#[allow(unused)]
fn example_display_min_installed_maps<C: Connection>(conn: &C, screen_num: usize) {
    let screen = &conn.setup().roots[screen_num];
    let _min_installed = screen.min_installed_maps;
}

//     MaxCmapsOfScreen
//
// You get the maximum installed colormaps in the `xcb_screen_t` structure:

#[allow(unused)]
fn example_display_max_installed_maps<C: Connection>(conn: &C, screen_num: usize) {
    let screen = &conn.setup().roots[screen_num];
    let _max_installed = screen.max_installed_maps;
}

//     DoesSaveUnders
//
// You know if `save_unders` is set, by looking in the `xcb_screen_t` structure:

#[allow(unused)]
fn example_save_unders<C: Connection>(conn: &C, screen_num: usize) {
    let screen = &conn.setup().roots[screen_num];
    let _save_unders = screen.save_unders;
}

//     DoesBackingStore
//
// You know the value of `backing_stores`, by looking in the `xcb_screen_t` structure:

#[allow(unused)]
fn example_backing_store<C: Connection>(conn: &C, screen_num: usize) {
    let screen = &conn.setup().roots[screen_num];
    let _backing_stores = screen.backing_stores;
}

//     EventMaskOfScreen
//
// To get the current input masks, you look in the `xcb_screen_t` structure:

#[allow(unused)]
fn example_input_masks<C: Connection>(conn: &C, screen_num: usize) {
    let screen = &conn.setup().roots[screen_num];
    let _input_masks = screen.current_input_masks;
}

//     Miscellaneous macros
//     --------------------
//
//
//     DisplayOfScreen
//
// in Xlib, the `Screen` structure stores its associated `Display` structure. This is not the case in the X Window protocol, hence, it's also not the case in XCB. So you have to store it by yourself.
//
//
// DisplayCells / CellsOfScreen
//
// To get the colormap entries, you look in the `xcb_visualtype_t` structure, that you grab as
// shown above:

#[allow(unused)]
fn example_visual_colormap_entries(visual: &Visualtype) -> u16 {
    visual.colormap_entries
}

fn main() {
    example1().unwrap();
    example2().unwrap();
    example3().unwrap();
    example4().unwrap();
    example5().unwrap();
    example6().unwrap();
    example7().unwrap();
    example8().unwrap();
    example9().unwrap();
    example10().unwrap();
    example11().unwrap();
}
