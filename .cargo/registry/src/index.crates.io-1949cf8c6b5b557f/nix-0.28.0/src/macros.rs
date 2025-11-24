// Thanks to Tokio for this macro
macro_rules! feature {
    (
        #![$meta:meta]
        $($item:item)*
    ) => {
        $(
            #[cfg($meta)]
            #[cfg_attr(docsrs, doc(cfg($meta)))]
            $item
        )*
    }
}

/// The `libc_bitflags!` macro helps with a common use case of defining a public bitflags type
/// with values from the libc crate. It is used the same way as the `bitflags!` macro, except
/// that only the name of the flag value has to be given.
///
/// The `libc` crate must be in scope with the name `libc`.
///
/// # Example
/// ```ignore
/// libc_bitflags!{
///     pub struct ProtFlags: libc::c_int {
///         PROT_NONE;
///         PROT_READ;
///         /// PROT_WRITE enables write protect
///         PROT_WRITE;
///         PROT_EXEC;
///         #[cfg(linux_android)]
///         PROT_GROWSDOWN;
///         #[cfg(linux_android)]
///         PROT_GROWSUP;
///     }
/// }
/// ```
///
/// Example with casting, due to a mistake in libc. In this example, the
/// various flags have different types, so we cast the broken ones to the right
/// type.
///
/// ```ignore
/// libc_bitflags!{
///     pub struct SaFlags: libc::c_ulong {
///         SA_NOCLDSTOP as libc::c_ulong;
///         SA_NOCLDWAIT;
///         SA_NODEFER as libc::c_ulong;
///         SA_ONSTACK;
///         SA_RESETHAND as libc::c_ulong;
///         SA_RESTART as libc::c_ulong;
///         SA_SIGINFO;
///     }
/// }
/// ```
macro_rules! libc_bitflags {
    (
        $(#[$outer:meta])*
        pub struct $BitFlags:ident: $T:ty {
            $(
                $(#[$inner:ident $($args:tt)*])*
                $Flag:ident $(as $cast:ty)*;
            )+
        }
    ) => {
        ::bitflags::bitflags! {
            #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            #[repr(transparent)]
            $(#[$outer])*
            pub struct $BitFlags: $T {
                $(
                    $(#[$inner $($args)*])*
                    const $Flag = libc::$Flag $(as $cast)*;
                )+
            }
        }
    };
}

/// The `libc_enum!` macro helps with a common use case of defining an enum exclusively using
/// values from the `libc` crate. This macro supports both `pub` and private `enum`s.
///
/// The `libc` crate must be in scope with the name `libc`.
///
/// # Example
/// ```ignore
/// libc_enum!{
///     pub enum ProtFlags {
///         PROT_NONE,
///         PROT_READ,
///         PROT_WRITE,
///         PROT_EXEC,
///         #[cfg(linux_android)]
///         PROT_GROWSDOWN,
///         #[cfg(linux_android)]
///         PROT_GROWSUP,
///     }
/// }
/// ```
// Some targets don't use all rules.
#[allow(unused_macro_rules)]
macro_rules! libc_enum {
    // Exit rule.
    (@make_enum
        name: $BitFlags:ident,
        {
            $v:vis
            attrs: [$($attrs:tt)*],
            entries: [$($entries:tt)*],
        }
    ) => {
        $($attrs)*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        $v enum $BitFlags {
            $($entries)*
        }
    };

    // Exit rule including TryFrom
    (@make_enum
        name: $BitFlags:ident,
        {
            $v:vis
            attrs: [$($attrs:tt)*],
            entries: [$($entries:tt)*],
            from_type: $repr:path,
            try_froms: [$($try_froms:tt)*]
        }
    ) => {
        $($attrs)*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        $v enum $BitFlags {
            $($entries)*
        }
        impl ::std::convert::TryFrom<$repr> for $BitFlags {
            type Error = $crate::Error;
            #[allow(unused_doc_comments)]
            #[allow(deprecated)]
            #[allow(unused_attributes)]
            fn try_from(x: $repr) -> $crate::Result<Self> {
                match x {
                    $($try_froms)*
                    _ => Err($crate::Error::EINVAL)
                }
            }
        }
    };

    // Done accumulating.
    (@accumulate_entries
        name: $BitFlags:ident,
        {
            $v:vis
            attrs: $attrs:tt,
        },
        $entries:tt,
        $try_froms:tt;
    ) => {
        libc_enum! {
            @make_enum
            name: $BitFlags,
            {
                $v
                attrs: $attrs,
                entries: $entries,
            }
        }
    };

    // Done accumulating and want TryFrom
    (@accumulate_entries
        name: $BitFlags:ident,
        {
            $v:vis
            attrs: $attrs:tt,
            from_type: $repr:path,
        },
        $entries:tt,
        $try_froms:tt;
    ) => {
        libc_enum! {
            @make_enum
            name: $BitFlags,
            {
                $v
                attrs: $attrs,
                entries: $entries,
                from_type: $repr,
                try_froms: $try_froms
            }
        }
    };

    // Munch an attr.
    (@accumulate_entries
        name: $BitFlags:ident,
        $prefix:tt,
        [$($entries:tt)*],
        [$($try_froms:tt)*];
        #[$attr:meta] $($tail:tt)*
    ) => {
        libc_enum! {
            @accumulate_entries
            name: $BitFlags,
            $prefix,
            [
                $($entries)*
                #[$attr]
            ],
            [
                $($try_froms)*
                #[$attr]
            ];
            $($tail)*
        }
    };

    // Munch last ident if not followed by a comma.
    (@accumulate_entries
        name: $BitFlags:ident,
        $prefix:tt,
        [$($entries:tt)*],
        [$($try_froms:tt)*];
        $entry:ident
    ) => {
        libc_enum! {
            @accumulate_entries
            name: $BitFlags,
            $prefix,
            [
                $($entries)*
                $entry = libc::$entry,
            ],
            [
                $($try_froms)*
                libc::$entry => Ok($BitFlags::$entry),
            ];
        }
    };

    // Munch an ident; covers terminating comma case.
    (@accumulate_entries
        name: $BitFlags:ident,
        $prefix:tt,
        [$($entries:tt)*],
        [$($try_froms:tt)*];
        $entry:ident,
        $($tail:tt)*
    ) => {
        libc_enum! {
            @accumulate_entries
            name: $BitFlags,
            $prefix,
            [
                $($entries)*
                $entry = libc::$entry,
            ],
            [
                $($try_froms)*
                libc::$entry => Ok($BitFlags::$entry),
            ];
            $($tail)*
        }
    };

    // Munch an ident and cast it to the given type; covers terminating comma.
    (@accumulate_entries
        name: $BitFlags:ident,
        $prefix:tt,
        [$($entries:tt)*],
        [$($try_froms:tt)*];
        $entry:ident as $ty:ty,
        $($tail:tt)*
    ) => {
        libc_enum! {
            @accumulate_entries
            name: $BitFlags,
            $prefix,
            [
                $($entries)*
                $entry = libc::$entry as $ty,
            ],
            [
                $($try_froms)*
                libc::$entry as $ty => Ok($BitFlags::$entry),
            ];
            $($tail)*
        }
    };

    // Entry rule.
    (
        $(#[$attr:meta])*
        $v:vis enum $BitFlags:ident {
            $($vals:tt)*
        }
    ) => {
        libc_enum! {
            @accumulate_entries
            name: $BitFlags,
            {
                $v
                attrs: [$(#[$attr])*],
            },
            [],
            [];
            $($vals)*
        }
    };

    // Entry rule including TryFrom
    (
        $(#[$attr:meta])*
        $v:vis enum $BitFlags:ident {
            $($vals:tt)*
        }
        impl TryFrom<$repr:path>
    ) => {
        libc_enum! {
            @accumulate_entries
            name: $BitFlags,
            {
                $v
                attrs: [$(#[$attr])*],
                from_type: $repr,
            },
            [],
            [];
            $($vals)*
        }
    };
}
