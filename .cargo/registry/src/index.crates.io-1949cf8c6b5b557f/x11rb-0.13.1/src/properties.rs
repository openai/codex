//! Utility functions for working with X11 properties

use crate::connection::RequestConnection;
use crate::cookie::{Cookie, VoidCookie};
use crate::errors::{ConnectionError, ParseError, ReplyError};
use crate::protocol::xproto::{self, Atom, AtomEnum, GetPropertyReply, Window};
use crate::x11_utils::{Serialize, TryParse};

macro_rules! property_cookie {
    {
        $(#[$meta:meta])*
        pub struct $cookie_name:ident: $struct_name:ident,
        $from_reply:expr,
    } => {
        $(#[$meta])*
        #[derive(Debug)]
        pub struct $cookie_name<'a, Conn: RequestConnection + ?Sized>(Cookie<'a, Conn, GetPropertyReply>);

        impl<'a, Conn> $cookie_name<'a, Conn>
        where
            Conn: RequestConnection + ?Sized,
        {
            /// Get the reply that the server sent.
            pub fn reply(self) -> Result<Option<$struct_name>, ReplyError> {
                #[allow(clippy::redundant_closure_call)]
                Ok($from_reply(self.0.reply()?)?)
            }

            /// Get the reply that the server sent, but have errors handled as events.
            pub fn reply_unchecked(self) -> Result<Option<$struct_name>, ConnectionError> {
                self.0
                    .reply_unchecked()?
                    .map($from_reply)
                    .transpose()
                    .map(|e| e.flatten())
                    .map_err(Into::into)
            }
        }
    }
}

// WM_CLASS

property_cookie! {
    /// A cookie for getting a window's `WM_CLASS` property.
    ///
    /// See `WmClass`.
    pub struct WmClassCookie: WmClass,
    WmClass::from_reply,
}

impl<'a, Conn> WmClassCookie<'a, Conn>
where
    Conn: RequestConnection + ?Sized,
{
    /// Send a `GetProperty` request for the `WM_CLASS` property of the given window
    pub fn new(conn: &'a Conn, window: Window) -> Result<Self, ConnectionError> {
        Ok(Self(xproto::get_property(
            conn,
            false,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            0,
            2048,
        )?))
    }
}

/// The value of a window's `WM_CLASS` property.
///
/// Usage example:
/// ```
/// use x11rb::connection::Connection;
/// use x11rb::errors::ConnectionError;
/// use x11rb::properties::WmClass;
/// use x11rb::protocol::xproto::Window;
///
/// fn print_class_instance(
///     conn: &impl Connection,
///     window: Window,
/// ) -> Result<bool, ConnectionError> {
///     let wm_class = match WmClass::get(conn, window)?.reply_unchecked()? {
///         Some(wm_class) => wm_class,
///         None => return Ok(false), // Getting the property failed
///     };
///     // Note that the WM_CLASS property is not actually encoded in utf8.
///     // ASCII values are most common and for these from_utf8() should be fine.
///     let class = std::str::from_utf8(wm_class.class());
///     let instance = std::str::from_utf8(wm_class.instance());
///     println!(
///         "For window {:x}, class is '{:?}' and instance is '{:?}'",
///         window, class, instance,
///     );
///     Ok(true)
/// }
/// ```
#[derive(Debug)]
pub struct WmClass(GetPropertyReply, usize);

impl WmClass {
    /// Send a `GetProperty` request for the `WM_CLASS` property of the given window
    pub fn get<C: RequestConnection>(
        conn: &C,
        window: Window,
    ) -> Result<WmClassCookie<'_, C>, ConnectionError> {
        WmClassCookie::new(conn, window)
    }

    /// Construct a new `WmClass` instance from a `GetPropertyReply`.
    ///
    /// The original `GetProperty` request must have been for a `WM_CLASS` property for this
    /// function to return sensible results.
    pub fn from_reply(reply: GetPropertyReply) -> Result<Option<Self>, ParseError> {
        if reply.type_ == AtomEnum::NONE.into() {
            return Ok(None);
        }
        if reply.type_ != AtomEnum::STRING.into() || reply.format != 8 {
            return Err(ParseError::InvalidValue);
        }
        // Find the first zero byte in the value
        let offset = reply
            .value
            .iter()
            .position(|&v| v == 0)
            .unwrap_or(reply.value.len());
        Ok(Some(WmClass(reply, offset)))
    }

    /// Get the instance contained in this `WM_CLASS` property
    pub fn instance(&self) -> &[u8] {
        &self.0.value[0..self.1]
    }

    /// Get the class contained in this `WM_CLASS` property
    pub fn class(&self) -> &[u8] {
        let start = self.1 + 1;
        if start >= self.0.value.len() {
            return &[];
        };
        let end = if self.0.value.last() == Some(&0) {
            self.0.value.len() - 1
        } else {
            self.0.value.len()
        };
        &self.0.value[start..end]
    }
}

// WM_SIZE_HINTS

/// Representation of whether some part of `WM_SIZE_HINTS` was user/program specified.
#[derive(Debug, Copy, Clone)]
pub enum WmSizeHintsSpecification {
    /// The user specified the values.
    UserSpecified,
    /// The program specified the values.
    ProgramSpecified,
}

property_cookie! {
    /// A cookie for getting a window's `WM_SIZE_HINTS` property.
    pub struct WmSizeHintsCookie: WmSizeHints,
    |reply| WmSizeHints::from_reply(&reply),
}

const NUM_WM_SIZE_HINTS_ELEMENTS: u16 = 18;

impl<'a, Conn> WmSizeHintsCookie<'a, Conn>
where
    Conn: RequestConnection + ?Sized,
{
    /// Send a `GetProperty` request for the given property of the given window
    pub fn new(
        conn: &'a Conn,
        window: Window,
        property: impl Into<Atom>,
    ) -> Result<Self, ConnectionError> {
        Ok(Self(xproto::get_property(
            conn,
            false,
            window,
            property,
            AtomEnum::WM_SIZE_HINTS,
            0,
            NUM_WM_SIZE_HINTS_ELEMENTS.into(),
        )?))
    }
}

// Possible flags for `WM_SIZE_HINTS`.
const U_S_POSITION: u32 = 1;
const U_S_SIZE: u32 = 1 << 1;
const P_S_POSITION: u32 = 1 << 2;
const P_S_SIZE: u32 = 1 << 3;
const P_MIN_SIZE: u32 = 1 << 4;
const P_MAX_SIZE: u32 = 1 << 5;
const P_RESIZE_INCREMENT: u32 = 1 << 6;
const P_ASPECT: u32 = 1 << 7;
const P_BASE_SIZE: u32 = 1 << 8;
const P_WIN_GRAVITY: u32 = 1 << 9;

/// An aspect ratio `numerator` / `denominator`.
#[derive(Debug, Copy, Clone)]
pub struct AspectRatio {
    /// The numerator of the aspect ratio.
    pub numerator: i32,
    /// The denominator of the aspect ratio.
    pub denominator: i32,
}

impl AspectRatio {
    /// Create a new aspect ratio with the given values.
    pub fn new(numerator: i32, denominator: i32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }
}

impl TryParse for AspectRatio {
    fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let ((numerator, denominator), remaining) = TryParse::try_parse(value)?;
        let result = AspectRatio::new(numerator, denominator);
        Ok((result, remaining))
    }
}

#[allow(clippy::many_single_char_names)]
impl Serialize for AspectRatio {
    type Bytes = [u8; 8];
    fn serialize(&self) -> Self::Bytes {
        let [a, b, c, d] = self.numerator.serialize();
        let [e, f, g, h] = self.denominator.serialize();
        [a, b, c, d, e, f, g, h]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        (self.numerator, self.denominator).serialize_into(bytes);
    }
}

/// A structure representing a `WM_SIZE_HINTS` property.
#[derive(Debug, Default, Copy, Clone)]
pub struct WmSizeHints {
    /// The position that the window should be assigned.
    ///
    /// Note that current versions of ICCCM only make use of the `WmSizeHintsSpecification` field.
    /// The later two fields exist only for backwards compatibility.
    pub position: Option<(WmSizeHintsSpecification, i32, i32)>,
    /// The size that the window should be assigned.
    ///
    /// Note that current versions of ICCCM only make use of the `WmSizeHintsSpecification` field.
    /// The later two fields exist only for backwards compatibility.
    pub size: Option<(WmSizeHintsSpecification, i32, i32)>,
    /// The minimum size that the window may be assigned.
    pub min_size: Option<(i32, i32)>,
    /// The maximum size that the window may be assigned.
    pub max_size: Option<(i32, i32)>,
    /// The increment to be used for sizing the window together with `base_size`.
    pub size_increment: Option<(i32, i32)>,
    /// The minimum and maximum aspect ratio.
    pub aspect: Option<(AspectRatio, AspectRatio)>,
    /// The base size of the window.
    ///
    /// This is used together with `size_increment`.
    pub base_size: Option<(i32, i32)>,
    /// The gravity that is used to make room for window decorations.
    pub win_gravity: Option<xproto::Gravity>,
}

impl WmSizeHints {
    /// Get a new, empty `WmSizeHints` structure.
    pub fn new() -> Self {
        Default::default()
    }

    /// Send a `GetProperty` request for the given property of the given window
    pub fn get<C: RequestConnection>(
        conn: &C,
        window: Window,
        property: impl Into<Atom>,
    ) -> Result<WmSizeHintsCookie<'_, C>, ConnectionError> {
        WmSizeHintsCookie::new(conn, window, property)
    }

    /// Send a `GetProperty` request for the `WM_NORMAL_HINTS` property of the given window
    pub fn get_normal_hints<C: RequestConnection>(
        conn: &C,
        window: Window,
    ) -> Result<WmSizeHintsCookie<'_, C>, ConnectionError> {
        Self::get(conn, window, AtomEnum::WM_NORMAL_HINTS)
    }

    /// Construct a new `WmSizeHints` instance from a `GetPropertyReply`.
    ///
    /// The original `WmSizeHints` request must have been for a `WM_SIZE_HINTS` property for this
    /// function to return sensible results.
    pub fn from_reply(reply: &GetPropertyReply) -> Result<Option<Self>, ParseError> {
        if reply.type_ == AtomEnum::NONE.into() {
            return Ok(None);
        }
        if reply.type_ != AtomEnum::WM_SIZE_HINTS.into() || reply.format != 32 {
            return Err(ParseError::InvalidValue);
        }
        Ok(Some(Self::try_parse(&reply.value)?.0))
    }

    /// Set these `WM_SIZE_HINTS` on some window as the `WM_NORMAL_HINTS` property.
    pub fn set_normal_hints<'a, C: RequestConnection + ?Sized>(
        &self,
        conn: &'a C,
        window: Window,
    ) -> Result<VoidCookie<'a, C>, ConnectionError> {
        self.set(conn, window, AtomEnum::WM_NORMAL_HINTS)
    }

    /// Set these `WM_SIZE_HINTS` on some window as the given property.
    pub fn set<'a, C: RequestConnection + ?Sized>(
        &self,
        conn: &'a C,
        window: Window,
        property: impl Into<Atom>,
    ) -> Result<VoidCookie<'a, C>, ConnectionError> {
        let data = self.serialize();
        xproto::change_property(
            conn,
            xproto::PropMode::REPLACE,
            window,
            property.into(),
            AtomEnum::WM_SIZE_HINTS,
            32,
            NUM_WM_SIZE_HINTS_ELEMENTS.into(),
            &data,
        )
    }
}

impl TryParse for WmSizeHints {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        // Implemented based on what xcb_icccm does. At least a bit. This stuff makes no sense...

        let (flags, remaining) = u32::try_parse(remaining)?;
        let (x, remaining) = i32::try_parse(remaining)?;
        let (y, remaining) = i32::try_parse(remaining)?;
        let (width, remaining) = i32::try_parse(remaining)?;
        let (height, remaining) = i32::try_parse(remaining)?;
        let (min_size, remaining) = parse_with_flag::<(i32, i32)>(remaining, flags, P_MIN_SIZE)?;
        let (max_size, remaining) = parse_with_flag::<(i32, i32)>(remaining, flags, P_MAX_SIZE)?;
        let (size_increment, remaining) =
            parse_with_flag::<(i32, i32)>(remaining, flags, P_RESIZE_INCREMENT)?;
        let (aspect, remaining) =
            parse_with_flag::<(AspectRatio, AspectRatio)>(remaining, flags, P_ASPECT)?;
        // Apparently, some older version of ICCCM didn't have these...?
        let (base_size, win_gravity, remaining) = if remaining.is_empty() {
            (min_size, Some(xproto::Gravity::NORTH_WEST), remaining)
        } else {
            let (base_size, remaining) =
                parse_with_flag::<(i32, i32)>(remaining, flags, P_BASE_SIZE)?;
            let (win_gravity, remaining) = parse_with_flag::<u32>(remaining, flags, P_WIN_GRAVITY)?;
            (base_size, win_gravity.map(Into::into), remaining)
        };

        let position = if flags & U_S_POSITION != 0 {
            Some((WmSizeHintsSpecification::UserSpecified, x, y))
        } else if flags & P_S_POSITION != 0 {
            Some((WmSizeHintsSpecification::ProgramSpecified, x, y))
        } else {
            None
        };
        let size = if flags & U_S_SIZE != 0 {
            Some((WmSizeHintsSpecification::UserSpecified, width, height))
        } else if flags & P_S_SIZE != 0 {
            Some((WmSizeHintsSpecification::ProgramSpecified, width, height))
        } else {
            None
        };

        Ok((
            WmSizeHints {
                position,
                size,
                min_size,
                max_size,
                size_increment,
                aspect,
                base_size,
                win_gravity,
            },
            remaining,
        ))
    }
}

impl Serialize for WmSizeHints {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Self::Bytes {
        let mut result = Vec::with_capacity((NUM_WM_SIZE_HINTS_ELEMENTS * 4).into());
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        let mut flags = 0;
        match self.position {
            Some((WmSizeHintsSpecification::UserSpecified, _, _)) => flags |= U_S_POSITION,
            Some((WmSizeHintsSpecification::ProgramSpecified, _, _)) => flags |= P_S_POSITION,
            None => {}
        }
        match self.size {
            Some((WmSizeHintsSpecification::UserSpecified, _, _)) => flags |= U_S_SIZE,
            Some((WmSizeHintsSpecification::ProgramSpecified, _, _)) => flags |= P_S_SIZE,
            None => {}
        }
        flags |= self.min_size.map_or(0, |_| P_MIN_SIZE);
        flags |= self.max_size.map_or(0, |_| P_MAX_SIZE);
        flags |= self.size_increment.map_or(0, |_| P_RESIZE_INCREMENT);
        flags |= self.aspect.map_or(0, |_| P_ASPECT);
        flags |= self.base_size.map_or(0, |_| P_BASE_SIZE);
        flags |= self.win_gravity.map_or(0, |_| P_WIN_GRAVITY);
        flags.serialize_into(bytes);

        match self.position {
            Some((_, x, y)) => (x, y),
            None => (0, 0),
        }
        .serialize_into(bytes);

        match self.size {
            Some((_, width, height)) => (width, height),
            None => (0, 0),
        }
        .serialize_into(bytes);

        self.min_size.unwrap_or((0, 0)).serialize_into(bytes);
        self.max_size.unwrap_or((0, 0)).serialize_into(bytes);
        self.size_increment.unwrap_or((0, 0)).serialize_into(bytes);
        self.aspect
            .unwrap_or((AspectRatio::new(0, 0), AspectRatio::new(0, 0)))
            .serialize_into(bytes);
        self.base_size.unwrap_or((0, 0)).serialize_into(bytes);
        self.win_gravity.map_or(0, u32::from).serialize_into(bytes);
    }
}

// WM_HINTS
//
property_cookie! {
    /// A cookie for getting a window's `WM_HINTS` property.
    ///
    /// See `WmHints`.
    pub struct WmHintsCookie: WmHints,
    |reply| WmHints::from_reply(&reply),
}

const NUM_WM_HINTS_ELEMENTS: u32 = 9;

impl<'a, Conn> WmHintsCookie<'a, Conn>
where
    Conn: RequestConnection + ?Sized,
{
    /// Send a `GetProperty` request for the `WM_CLASS` property of the given window
    pub fn new(conn: &'a Conn, window: Window) -> Result<Self, ConnectionError> {
        Ok(Self(xproto::get_property(
            conn,
            false,
            window,
            AtomEnum::WM_HINTS,
            AtomEnum::WM_HINTS,
            0,
            NUM_WM_HINTS_ELEMENTS,
        )?))
    }
}

/// The possible values for a `WM_STATE`'s state field.
#[derive(Debug, Copy, Clone)]
pub enum WmHintsState {
    /// The window should be in Normal state.
    Normal,
    /// The window should be in Iconic state.
    Iconic,
}

// Possible flags for `WM_HINTS`.
const HINT_INPUT: u32 = 1;
const HINT_STATE: u32 = 1 << 1;
const HINT_ICON_PIXMAP: u32 = 1 << 2;
const HINT_ICON_WINDOW: u32 = 1 << 3;
const HINT_ICON_POSITION: u32 = 1 << 4;
const HINT_ICON_MASK: u32 = 1 << 5;
const HINT_WINDOW_GROUP: u32 = 1 << 6;
// This bit is obsolete, according to ICCCM
//const HINT_MESSAGE: u32 = 1 << 7;
const HINT_URGENCY: u32 = 1 << 8;

/// A structure representing a `WM_HINTS` property.
#[derive(Debug, Default, Copy, Clone)]
pub struct WmHints {
    /// Whether the window manager may set the input focus to this window.
    ///
    /// See ICCCM §4.1.7 for details.
    pub input: Option<bool>,

    /// The state that the window should be when it leaves the Withdrawn state.
    pub initial_state: Option<WmHintsState>,

    /// A pixmap that represents the icon of this window.
    pub icon_pixmap: Option<xproto::Pixmap>,

    /// A window that should be used as icon.
    pub icon_window: Option<Window>,

    /// The position where the icon should be shown.
    pub icon_position: Option<(i32, i32)>,

    /// A mask for `icon_pixmap`.
    ///
    /// This allows nonrectangular icons.
    pub icon_mask: Option<xproto::Pixmap>,

    /// A window that represents a group of windows.
    ///
    /// The specified window is called the "group leader". All windows with the same group leader
    /// are part of the same group.
    pub window_group: Option<Window>,

    /// Indication that the window contents are urgent.
    ///
    /// Urgency means that a timely response of the user is required. The window manager must make
    /// some effort to draw the user's attention to this window while this flag is set.
    pub urgent: bool,
}

impl WmHints {
    /// Get a new, empty `WmSizeHints` structure.
    pub fn new() -> Self {
        Default::default()
    }

    /// Send a `GetProperty` request for the `WM_HINTS` property of the given window
    pub fn get<C: RequestConnection>(
        conn: &C,
        window: Window,
    ) -> Result<WmHintsCookie<'_, C>, ConnectionError> {
        WmHintsCookie::new(conn, window)
    }

    /// Construct a new `WmHints` instance from a `GetPropertyReply`.
    ///
    /// The original `WmHints` request must have been for a `WM_HINTS` property for this
    /// function to return sensible results.
    pub fn from_reply(reply: &GetPropertyReply) -> Result<Option<Self>, ParseError> {
        if reply.type_ == AtomEnum::NONE.into() {
            return Ok(None);
        }
        if reply.type_ != AtomEnum::WM_HINTS.into() || reply.format != 32 {
            return Err(ParseError::InvalidValue);
        }
        Ok(Some(Self::try_parse(&reply.value)?.0))
    }

    /// Set these `WM_HINTS` on some window.
    pub fn set<'a, C: RequestConnection + ?Sized>(
        &self,
        conn: &'a C,
        window: Window,
    ) -> Result<VoidCookie<'a, C>, ConnectionError> {
        let data = self.serialize();
        xproto::change_property(
            conn,
            xproto::PropMode::REPLACE,
            window,
            AtomEnum::WM_HINTS,
            AtomEnum::WM_HINTS,
            32,
            NUM_WM_HINTS_ELEMENTS,
            &data,
        )
    }
}

impl TryParse for WmHints {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (flags, remaining) = u32::try_parse(remaining)?;
        let (input, remaining) = parse_with_flag::<u32>(remaining, flags, HINT_INPUT)?;
        let (initial_state, remaining) = parse_with_flag::<u32>(remaining, flags, HINT_STATE)?;
        let (icon_pixmap, remaining) = parse_with_flag::<u32>(remaining, flags, HINT_ICON_PIXMAP)?;
        let (icon_window, remaining) = parse_with_flag::<u32>(remaining, flags, HINT_ICON_WINDOW)?;
        let (icon_position, remaining) =
            parse_with_flag::<(i32, i32)>(remaining, flags, HINT_ICON_POSITION)?;
        let (icon_mask, remaining) = parse_with_flag::<u32>(remaining, flags, HINT_ICON_MASK)?;
        // Apparently, some older version of ICCCM didn't have this...?
        let (window_group, remaining) = if remaining.is_empty() {
            (None, remaining)
        } else {
            let (window_group, remaining) =
                parse_with_flag::<u32>(remaining, flags, HINT_WINDOW_GROUP)?;
            (window_group, remaining)
        };

        let input = input.map(|input| input != 0);

        let initial_state = match initial_state {
            None => None,
            Some(1) => Some(WmHintsState::Normal),
            Some(3) => Some(WmHintsState::Iconic),
            _ => return Err(ParseError::InvalidValue),
        };

        let urgent = flags & HINT_URGENCY != 0;

        Ok((
            WmHints {
                input,
                initial_state,
                icon_pixmap,
                icon_window,
                icon_position,
                icon_mask,
                window_group,
                urgent,
            },
            remaining,
        ))
    }
}

impl Serialize for WmHints {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Self::Bytes {
        // 9*4 surely fits into an usize, so this unwrap() cannot trigger
        let mut result = Vec::with_capacity((NUM_WM_HINTS_ELEMENTS * 4).try_into().unwrap());
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        let mut flags = 0;
        flags |= self.input.map_or(0, |_| HINT_INPUT);
        flags |= self.initial_state.map_or(0, |_| HINT_STATE);
        flags |= self.icon_pixmap.map_or(0, |_| HINT_ICON_PIXMAP);
        flags |= self.icon_window.map_or(0, |_| HINT_ICON_WINDOW);
        flags |= self.icon_position.map_or(0, |_| HINT_ICON_POSITION);
        flags |= self.icon_mask.map_or(0, |_| HINT_ICON_MASK);
        flags |= self.window_group.map_or(0, |_| HINT_WINDOW_GROUP);
        if self.urgent {
            flags |= HINT_URGENCY;
        }

        flags.serialize_into(bytes);
        u32::from(self.input.unwrap_or(false)).serialize_into(bytes);
        match self.initial_state {
            Some(WmHintsState::Normal) => 1,
            Some(WmHintsState::Iconic) => 3,
            None => 0,
        }
        .serialize_into(bytes);
        self.icon_pixmap.unwrap_or(0).serialize_into(bytes);
        self.icon_window.unwrap_or(0).serialize_into(bytes);
        self.icon_position.unwrap_or((0, 0)).serialize_into(bytes);
        self.icon_mask.unwrap_or(0).serialize_into(bytes);
        self.window_group.unwrap_or(0).serialize_into(bytes);
    }
}

/// Parse an element of type `T` and turn it into an `Option` by checking if the given `bit` is set
/// in `flags`.
fn parse_with_flag<T: TryParse>(
    remaining: &[u8],
    flags: u32,
    bit: u32,
) -> Result<(Option<T>, &[u8]), ParseError> {
    let (value, remaining) = T::try_parse(remaining)?;
    if flags & bit != 0 {
        Ok((Some(value), remaining))
    } else {
        Ok((None, remaining))
    }
}

#[cfg(test)]
mod test {
    use super::{WmClass, WmHints, WmHintsState, WmSizeHints};
    use crate::protocol::xproto::{Atom, AtomEnum, GetPropertyReply, Gravity};
    use crate::x11_utils::Serialize;

    fn get_property_reply(value: &[u8], format: u8, type_: impl Into<Atom>) -> GetPropertyReply {
        GetPropertyReply {
            format,
            sequence: 0,
            length: 0,
            type_: type_.into(),
            bytes_after: 0,
            value_len: value.len().try_into().unwrap(),
            value: value.to_vec(),
        }
    }

    #[test]
    fn test_wm_class() {
        for (input, instance, class) in &[
            (&b""[..], &b""[..], &b""[..]),
            (b"\0", b"", b""),
            (b"\0\0", b"", b""),
            (b"\0\0\0", b"", b"\0"),
            (b"Hello World", b"Hello World", b""),
            (b"Hello World\0", b"Hello World", b""),
            (b"Hello\0World\0", b"Hello", b"World"),
            (b"Hello\0World", b"Hello", b"World"),
            (b"Hello\0World\0Good\0Day", b"Hello", b"World\0Good\0Day"),
        ] {
            let wm_class = WmClass::from_reply(get_property_reply(input, 8, AtomEnum::STRING))
                .unwrap()
                .unwrap();
            assert_eq!((wm_class.instance(), wm_class.class()), (*instance, *class));
        }
    }

    #[test]
    fn test_wm_class_missing() {
        let wm_class = WmClass::from_reply(get_property_reply(&[], 0, AtomEnum::NONE)).unwrap();
        assert!(wm_class.is_none());
    }

    #[test]
    fn test_wm_normal_hints() {
        // This is the value of some random xterm window.
        // It was acquired via 'xtrace xprop WM_NORMAL_HINTS'.
        let input = [
            0x0000_0350,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0015,
            0x0000_0017,
            0x0000_0000,
            0x0000_0000,
            0x0000_000a,
            0x0000_0013,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_000b,
            0x0000_0004,
            0x0000_0001,
        ];
        let input = input
            .iter()
            .flat_map(|v| u32::serialize(v).to_vec())
            .collect::<Vec<u8>>();
        let wm_size_hints =
            WmSizeHints::from_reply(&get_property_reply(&input, 32, AtomEnum::WM_SIZE_HINTS))
                .unwrap()
                .unwrap();

        assert!(
            wm_size_hints.position.is_none(),
            "{:?}",
            wm_size_hints.position,
        );
        assert!(wm_size_hints.size.is_none(), "{:?}", wm_size_hints.size);
        assert_eq!(wm_size_hints.min_size, Some((21, 23)));
        assert_eq!(wm_size_hints.max_size, None);
        assert_eq!(wm_size_hints.size_increment, Some((10, 19)));
        assert!(wm_size_hints.aspect.is_none(), "{:?}", wm_size_hints.aspect);
        assert_eq!(wm_size_hints.base_size, Some((11, 4)));
        assert_eq!(wm_size_hints.win_gravity, Some(Gravity::NORTH_WEST));

        assert_eq!(input, wm_size_hints.serialize());
    }

    #[test]
    fn test_wm_normal_hints_missing() {
        let wm_size_hints =
            WmSizeHints::from_reply(&get_property_reply(&[], 0, AtomEnum::NONE)).unwrap();
        assert!(wm_size_hints.is_none());
    }

    #[test]
    fn test_wm_hints() {
        // This is the value of some random xterm window.
        // It was acquired via 'xtrace xprop WM_HINTS'.
        let input = [
            0x0000_0043,
            0x0000_0001,
            0x0000_0001,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0060_0009,
        ];
        let input = input
            .iter()
            .flat_map(|v| u32::serialize(v).to_vec())
            .collect::<Vec<u8>>();
        let wm_hints = WmHints::from_reply(&get_property_reply(&input, 32, AtomEnum::WM_HINTS))
            .unwrap()
            .unwrap();

        assert_eq!(wm_hints.input, Some(true));
        match wm_hints.initial_state {
            Some(WmHintsState::Normal) => {}
            value => panic!("Expected Some(Normal), but got {:?}", value),
        }
        assert_eq!(wm_hints.icon_pixmap, None);
        assert_eq!(wm_hints.icon_window, None);
        assert_eq!(wm_hints.icon_position, None);
        assert_eq!(wm_hints.icon_mask, None);
        assert_eq!(wm_hints.window_group, Some(0x0060_0009));
        assert!(!wm_hints.urgent);

        assert_eq!(input, wm_hints.serialize());
    }

    #[test]
    fn test_wm_hints_missing() {
        let wm_hints = WmHints::from_reply(&get_property_reply(&[], 0, AtomEnum::NONE)).unwrap();
        assert!(wm_hints.is_none());
    }
}
