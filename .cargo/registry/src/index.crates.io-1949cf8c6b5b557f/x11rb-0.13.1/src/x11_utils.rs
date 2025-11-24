//! Some utilities for working with X11.

pub use x11rb_protocol::x11_utils::{
    parse_request_header, BigRequests, ExtInfoProvider, ExtensionInformation, ReplyParsingFunction,
    Request, RequestHeader, Serialize, TryParse, TryParseFd, X11Error,
};

/// A helper macro for managing atoms
///
/// In X11, one often has to work with many different atoms that are already known at compile time.
/// This macro can simplify managing such a list of atoms.
///
/// The following macro invocation:
/// ```
/// # use x11rb::atom_manager;
/// atom_manager! {
///     /// A collection of Atoms.
///     pub AtomCollection:
///     /// A handle to a response from the X11 server.
///     AtomCollectionCookie {
///         _NET_WM_NAME,
///         _NET_WM_ICON,
///         ATOM_WITH_SPACES: b"ATOM WITH SPACES",
///         WHATEVER,
///     }
/// }
/// ```
/// ...expands to this:
/// ```
/// # use x11rb::protocol::xproto::{Atom, ConnectionExt, InternAtomReply};
/// # use x11rb::errors::{ConnectionError, ReplyError};
/// # use x11rb::cookie::Cookie;
/// #[allow(non_snake_case)]
/// #[derive(Debug, Clone, Copy)]
/// /// A collection of Atoms.
/// pub struct AtomCollection {
///     pub _NET_WM_NAME: Atom,
///     pub _NET_WM_ICON: Atom,
///     pub ATOM_WITH_SPACES: Atom,
///     pub WHATEVER: Atom,
/// }
///
/// #[allow(non_snake_case)]
/// #[derive(Debug)]
/// /// A handle to a response from the X11 server.
/// struct AtomCollectionCookie<'c, C: ConnectionExt> {
///     // please treat the actual members as private
///     # phantom: std::marker::PhantomData<&'c C>,
///     # _NET_WM_NAME: Cookie<'c, C, InternAtomReply>,
///     # _NET_WM_ICON: Cookie<'c, C, InternAtomReply>,
///     # ATOM_WITH_SPACES: Cookie<'c, C, InternAtomReply>,
///     # WHATEVER: Cookie<'c, C, InternAtomReply>,
/// }
///
/// impl AtomCollection {
///     pub fn new<C: ConnectionExt>(
///         conn: &C,
///     ) -> Result<AtomCollectionCookie<'_, C>, ConnectionError> {
///         // This is just an example for readability; the actual code is more efficient.
///         Ok(AtomCollectionCookie {
///             phantom: std::marker::PhantomData,
///             _NET_WM_NAME: conn.intern_atom(false, b"_NET_WM_NAME")?,
///             _NET_WM_ICON: conn.intern_atom(false, b"_NET_WM_ICON")?,
///             ATOM_WITH_SPACES: conn.intern_atom(false, b"ATOM WITH SPACES")?,
///             WHATEVER: conn.intern_atom(false, b"WHATEVER")?,
///         })
///     }
/// }
///
/// impl<'c, C> AtomCollectionCookie<'c, C>
/// where
///     C: ConnectionExt,
/// {
///     pub fn reply(self) -> Result<AtomCollection, ReplyError> {
///         // This is just an example for readability; the actual code is different.
///         Ok(AtomCollection {
///             _NET_WM_NAME: self._NET_WM_NAME.reply()?.atom,
///             _NET_WM_ICON: self._NET_WM_ICON.reply()?.atom,
///             ATOM_WITH_SPACES: self.ATOM_WITH_SPACES.reply()?.atom,
///             WHATEVER: self.WHATEVER.reply()?.atom,
///         })
///     }
/// }
/// ```
#[macro_export]
macro_rules! atom_manager {
    {
        $(#[$struct_meta:meta])*
        $vis:vis $struct_name:ident:
        $(#[$cookie_meta:meta])*
        $cookie_name:ident {
            $($field_name:ident$(: $atom_value:expr)?,)*
        }
    } => {
        // Cookie version
        #[allow(non_snake_case)]
        #[derive(Debug)]
        $(#[$cookie_meta])*
        $vis struct $cookie_name<'a, C: $crate::protocol::xproto::ConnectionExt> {
            __private_phantom: ::std::marker::PhantomData<&'a C>,
            __private_cookies: ::std::vec::Vec<$crate::cookie::Cookie<'a, C, $crate::protocol::xproto::InternAtomReply>>,
        }

        // Replies
        #[allow(non_snake_case)]
        #[derive(Debug, Clone, Copy)]
        $(#[$struct_meta])*
        $vis struct $struct_name {
            $(
                $vis $field_name: $crate::protocol::xproto::Atom,
            )*
        }

        impl $struct_name {
            $vis fn new<C: $crate::protocol::xproto::ConnectionExt>(
                _conn: &C,
            ) -> ::std::result::Result<$cookie_name<'_, C>, $crate::errors::ConnectionError> {
                let names = [
                    $($crate::__atom_manager_atom_value!($field_name$(: $atom_value)?),)*
                ];
                let cookies: ::std::result::Result<::std::vec::Vec<_>, _>
                    = names.into_iter().map(|name| _conn.intern_atom(false, name)).collect();
                Ok($cookie_name {
                    __private_phantom: ::std::marker::PhantomData,
                    __private_cookies: cookies?,
                })
            }
        }

        impl<'a, C: $crate::protocol::xproto::ConnectionExt> $cookie_name<'a, C> {
            $vis fn reply(self) -> ::std::result::Result<$struct_name, $crate::errors::ReplyError> {
                let mut replies = self.__private_cookies.into_iter();
                Ok($struct_name {
                    $(
                        $field_name: replies.next().expect("new() should have constructed a Vec of the correct size").reply()?.atom,
                    )*
                })
            }
        }
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __atom_manager_atom_value {
    ($field_name:ident) => {
        stringify!($field_name).as_bytes()
    };
    ($field_name:ident: $atom_value:expr) => {
        $atom_value
    };
}
