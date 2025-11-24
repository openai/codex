//! Utility functions for working with X11 cursors
//!
//! The code in this module is only available when the `cursor` feature of the library is enabled.

use crate::connection::Connection;
use crate::cookie::Cookie as X11Cookie;
use crate::errors::{ConnectionError, ReplyOrIdError};
use crate::protocol::render::{self, Pictformat};
use crate::protocol::xproto::{self, Font, Window};
use crate::resource_manager::Database;
use crate::NONE;

use std::fs::File;

mod find_cursor;
mod parse_cursor;

/// The level of cursor support of the X11 server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderSupport {
    /// Render extension not available
    None,

    /// Static cursor support (CreateCursor added in RENDER 0.5)
    StaticCursor,

    /// Animated cursor support (CreateAnimCursor added in RENDER 0.8)
    AnimatedCursor,
}

/// A cookie for creating a `Handle`
#[derive(Debug)]
pub struct Cookie<'a, 'b, C: Connection> {
    conn: &'a C,
    screen: &'a xproto::Screen,
    resource_database: &'b Database,
    render_info: Option<(
        X11Cookie<'a, C, render::QueryVersionReply>,
        X11Cookie<'a, C, render::QueryPictFormatsReply>,
    )>,
}

impl<C: Connection> Cookie<'_, '_, C> {
    /// Get the handle from the replies from the X11 server
    pub fn reply(self) -> Result<Handle, ReplyOrIdError> {
        let mut render_version = (0, 0);
        let mut picture_format = NONE;
        if let Some((version, formats)) = self.render_info {
            let version = version.reply()?;
            render_version = (version.major_version, version.minor_version);
            picture_format = find_format(&formats.reply()?);
        }
        Self::from_replies(
            self.conn,
            self.screen,
            self.resource_database,
            render_version,
            picture_format,
        )
    }

    /// Get the handle from the replies from the X11 server
    pub fn reply_unchecked(self) -> Result<Option<Handle>, ReplyOrIdError> {
        let mut render_version = (0, 0);
        let mut picture_format = NONE;
        if let Some((version, formats)) = self.render_info {
            match (version.reply_unchecked()?, formats.reply_unchecked()?) {
                (Some(version), Some(formats)) => {
                    render_version = (version.major_version, version.minor_version);
                    picture_format = find_format(&formats);
                }
                _ => return Ok(None),
            }
        }
        Ok(Some(Self::from_replies(
            self.conn,
            self.screen,
            self.resource_database,
            render_version,
            picture_format,
        )?))
    }

    fn from_replies(
        conn: &C,
        screen: &xproto::Screen,
        resource_database: &Database,
        render_version: (u32, u32),
        picture_format: Pictformat,
    ) -> Result<Handle, ReplyOrIdError> {
        let render_support = if render_version.0 >= 1 || render_version.1 >= 8 {
            RenderSupport::AnimatedCursor
        } else if render_version.0 >= 1 || render_version.1 >= 5 {
            RenderSupport::StaticCursor
        } else {
            RenderSupport::None
        };
        let theme = resource_database
            .get_string("Xcursor.theme", "")
            .map(|theme| theme.to_string());
        let cursor_size = match resource_database.get_value("Xcursor.size", "") {
            Ok(Some(value)) => value,
            _ => 0,
        };
        let xft_dpi = match resource_database.get_value("Xft.dpi", "") {
            Ok(Some(value)) => value,
            _ => 0,
        };
        let cursor_size = get_cursor_size(cursor_size, xft_dpi, screen);
        let cursor_font = conn.generate_id()?;
        let _ = xproto::open_font(conn, cursor_font, b"cursor")?;
        Ok(Handle {
            root: screen.root,
            cursor_font,
            picture_format,
            render_support,
            theme,
            cursor_size,
        })
    }
}

/// A handle necessary for loading cursors
#[derive(Debug)]
pub struct Handle {
    root: Window,
    cursor_font: Font,
    picture_format: Pictformat,
    render_support: RenderSupport,
    theme: Option<String>,
    cursor_size: u32,
}

impl Handle {
    /// Create a new cursor handle for creating cursors on the given screen.
    ///
    /// The `resource_database` is used to look up settings like the current cursor theme and the
    /// cursor size to use.
    ///
    /// This function returns a cookie that can be used to later get the actual handle.
    ///
    /// If you want this function not to block, you should prefetch the RENDER extension's data on
    /// the connection.
    #[allow(clippy::new_ret_no_self)]
    pub fn new<'a, 'b, C: Connection>(
        conn: &'a C,
        screen: usize,
        resource_database: &'b Database,
    ) -> Result<Cookie<'a, 'b, C>, ConnectionError> {
        let screen = &conn.setup().roots[screen];
        let render_info = if conn
            .extension_information(render::X11_EXTENSION_NAME)?
            .is_some()
        {
            let render_version = render::query_version(conn, 0, 8)?;
            let render_pict_format = render::query_pict_formats(conn)?;
            Some((render_version, render_pict_format))
        } else {
            None
        };
        Ok(Cookie {
            conn,
            screen,
            resource_database,
            render_info,
        })
    }

    /// Loads the specified cursor, either from the cursor theme or by falling back to the X11
    /// "cursor" font.
    pub fn load_cursor<C>(&self, conn: &C, name: &str) -> Result<xproto::Cursor, ReplyOrIdError>
    where
        C: Connection,
    {
        load_cursor(conn, self, name)
    }
}

fn open_cursor(theme: &Option<String>, name: &str) -> Option<find_cursor::Cursor<File>> {
    if let Some(theme) = theme {
        if let Ok(cursor) = find_cursor::find_cursor(theme, name) {
            return Some(cursor);
        }
    }
    if let Ok(cursor) = find_cursor::find_cursor("default", name) {
        Some(cursor)
    } else {
        None
    }
}

fn create_core_cursor<C: Connection>(
    conn: &C,
    cursor_font: Font,
    cursor: u16,
) -> Result<xproto::Cursor, ReplyOrIdError> {
    let result = conn.generate_id()?;
    let _ = xproto::create_glyph_cursor(
        conn,
        result,
        cursor_font,
        cursor_font,
        cursor,
        cursor + 1,
        // foreground color
        0,
        0,
        0,
        // background color
        u16::max_value(),
        u16::max_value(),
        u16::max_value(),
    )?;
    Ok(result)
}

fn create_render_cursor<C: Connection>(
    conn: &C,
    handle: &Handle,
    image: &parse_cursor::Image,
    storage: &mut Option<(xproto::Pixmap, xproto::Gcontext, u16, u16)>,
) -> Result<render::Animcursorelt, ReplyOrIdError> {
    let (cursor, picture) = (conn.generate_id()?, conn.generate_id()?);

    // Get a pixmap of the right size and a gc for it
    let (pixmap, gc) = if storage.map(|(_, _, w, h)| (w, h)) == Some((image.width, image.height)) {
        storage.map(|(pixmap, gc, _, _)| (pixmap, gc)).unwrap()
    } else {
        let (pixmap, gc) = if let Some((pixmap, gc, _, _)) = storage {
            let _ = xproto::free_gc(conn, *gc)?;
            let _ = xproto::free_pixmap(conn, *pixmap)?;
            (*pixmap, *gc)
        } else {
            (conn.generate_id()?, conn.generate_id()?)
        };
        let _ = xproto::create_pixmap(conn, 32, pixmap, handle.root, image.width, image.height)?;
        let _ = xproto::create_gc(conn, gc, pixmap, &Default::default())?;

        *storage = Some((pixmap, gc, image.width, image.height));
        (pixmap, gc)
    };

    // Sigh. We need the pixel data as a bunch of bytes.
    let pixels = crate::x11_utils::Serialize::serialize(&image.pixels[..]);
    let _ = xproto::put_image(
        conn,
        xproto::ImageFormat::Z_PIXMAP,
        pixmap,
        gc,
        image.width,
        image.height,
        0,
        0,
        0,
        32,
        &pixels,
    )?;

    let _ = render::create_picture(
        conn,
        picture,
        pixmap,
        handle.picture_format,
        &Default::default(),
    )?;
    let _ = render::create_cursor(conn, cursor, picture, image.x_hot, image.y_hot)?;
    let _ = render::free_picture(conn, picture)?;

    Ok(render::Animcursorelt {
        cursor,
        delay: image.delay,
    })
}

fn load_cursor<C: Connection>(
    conn: &C,
    handle: &Handle,
    name: &str,
) -> Result<xproto::Cursor, ReplyOrIdError> {
    // Find the right cursor, load it directly if it is a core cursor
    let cursor_file = match open_cursor(&handle.theme, name) {
        None => return Ok(NONE),
        Some(find_cursor::Cursor::CoreChar(c)) => {
            return create_core_cursor(conn, handle.cursor_font, c)
        }
        Some(find_cursor::Cursor::File(f)) => f,
    };

    // We have to load a file and use RENDER to create a cursor
    if handle.render_support == RenderSupport::None {
        return Ok(NONE);
    }

    // Load the cursor from the file
    use std::io::BufReader;
    let images = parse_cursor::parse_cursor(&mut BufReader::new(cursor_file), handle.cursor_size)
        .or(Err(crate::errors::ParseError::InvalidValue))?;
    let mut images = &images[..];

    // No animated cursor support? Only use the first image
    if handle.render_support == RenderSupport::StaticCursor {
        images = &images[0..1];
    }

    // Now transfer the cursors to the X11 server
    let mut storage = None;
    let cursors = images
        .iter()
        .map(|image| create_render_cursor(conn, handle, image, &mut storage))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some((pixmap, gc, _, _)) = storage {
        let _ = xproto::free_gc(conn, gc)?;
        let _ = xproto::free_pixmap(conn, pixmap)?;
    }

    if cursors.len() == 1 {
        Ok(cursors[0].cursor)
    } else {
        let result = conn.generate_id()?;
        let _ = render::create_anim_cursor(conn, result, &cursors)?;
        for elem in cursors {
            let _ = xproto::free_cursor(conn, elem.cursor)?;
        }
        Ok(result)
    }
}

fn find_format(reply: &render::QueryPictFormatsReply) -> Pictformat {
    reply
        .formats
        .iter()
        .filter(|format| {
            format.type_ == render::PictType::DIRECT
                && format.depth == 32
                && format.direct.red_shift == 16
                && format.direct.red_mask == 0xff
                && format.direct.green_shift == 8
                && format.direct.green_mask == 0xff
                && format.direct.blue_shift == 0
                && format.direct.blue_mask == 0xff
                && format.direct.alpha_shift == 24
                && format.direct.alpha_mask == 0xff
        })
        .map(|format| format.id)
        .next()
        .expect("The X11 server is missing the RENDER ARGB_32 standard format!")
}

fn get_cursor_size(rm_cursor_size: u32, rm_xft_dpi: u32, screen: &xproto::Screen) -> u32 {
    if let Some(size) = std::env::var("XCURSOR_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        return size;
    }
    if rm_cursor_size > 0 {
        return rm_cursor_size;
    }
    if rm_xft_dpi > 0 {
        return rm_xft_dpi * 16 / 72;
    }
    u32::from(screen.height_in_pixels.min(screen.width_in_pixels) / 48)
}
