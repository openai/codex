#![warn(missing_docs, rust_2018_idioms)]
//! A macro which makes errors easy to write
//!
//! Minimum type is like this:
//!
//! ```rust
//! #[macro_use] extern crate quick_error;
//! # fn main() {}
//!
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum SomeError {
//!         Variant1 {}
//!     }
//! }
//! ```
//! Both ``pub`` and non-public types may be declared, and all meta attributes
//! (such as ``#[derive(Debug)]``) are forwarded as is. The `Debug` must be
//! implemented (but you may do that yourself if you like). The documentation
//! comments ``/// something`` (as well as other meta attrbiutes) on variants
//! are allowed.
//!
//! # Allowed Syntax
//!
//! You may add arbitrary parameters to any struct variant:
//!
//! ```rust
//! # #[macro_use] extern crate quick_error;
//! # fn main() {}
//! #
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum SomeError {
//!         /// IO Error
//!         Io(err: std::io::Error) {}
//!         /// Utf8 Error
//!         Utf8(err: std::str::Utf8Error) {}
//!     }
//! }
//! ```
//!
//! Note unlike in normal Enum declarations you declare names of fields (which
//! are omitted from type). How they can be used is outlined below.
//!
//! Now you might have noticed trailing braces `{}`. They are used to define
//! implementations. By default:
//!
//! * `Error::source()` returns None (even if type wraps some value)
//! * `Display` outputs debug representation
//! * No `From` implementations are defined
//!
//! ```rust
//! # #[macro_use] extern crate quick_error;
//! # fn main() {}
//! #
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum SomeError {
//!         Io(err: std::io::Error) {
//!             display("{}", err)
//!         }
//!         Utf8(err: std::str::Utf8Error) {
//!             display("utf8 error")
//!         }
//!     }
//! }
//! ```
//!
//! To change `source` method to return some error, add `source(value)`, for
//! example:
//!
//! ```rust
//! # #[macro_use] extern crate quick_error;
//! # fn main() {}
//! #
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum SomeError {
//!         Io(err: std::io::Error) {
//!             source(err)
//!         }
//!         Utf8(err: std::str::Utf8Error) {
//!             display("utf8 error")
//!         }
//!         Other(err: Box<std::error::Error>) {
//!             source(&**err)
//!         }
//!     }
//! }
//! ```
//! Note you don't need to wrap value in `Some`, its implicit. In case you want
//! `None` returned just omit the `source`. You can't return `None`
//! conditionally.
//!
//! To change how each clause is `Display`ed add `display(pattern,..args)`,
//! for example:
//!
//! ```rust
//! # #[macro_use] extern crate quick_error;
//! # fn main() {}
//! #
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum SomeError {
//!         Io(err: std::io::Error) {
//!             display("I/O error: {}", err)
//!         }
//!         Utf8(err: std::str::Utf8Error) {
//!             display("Utf8 error, valid up to {}", err.valid_up_to())
//!         }
//!     }
//! }
//! ```
//!
//! If you need a reference to the error when `Display`ing, you can instead use
//! `display(x) -> (pattern, ..args)`, where `x` sets the name of the reference.
//!
//! ```rust
//! # #[macro_use] extern crate quick_error;
//! # fn main() {}
//! #
//! use std::error::Error; // put methods like `source()` of this trait into scope
//!
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum SomeError {
//!         Io(err: std::io::Error) {
//!             display(x) -> ("I/O: {}", err)
//!         }
//!         Utf8(err: std::str::Utf8Error) {
//!             display(self_) -> ("UTF-8 error. Valid up to {}", err.valid_up_to())
//!         }
//!     }
//! }
//! ```
//!
//! To convert to the type from any other, use one of the three forms of
//! `from` clause.
//!
//! For example, to convert simple wrapper use bare `from()`:
//!
//! ```rust
//! # #[macro_use] extern crate quick_error;
//! # fn main() {}
//! #
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum SomeError {
//!         Io(err: std::io::Error) {
//!             from()
//!         }
//!     }
//! }
//! ```
//!
//! This implements ``From<io::Error>``.
//!
//! To convert to singleton enumeration type (discarding the value), use
//! the `from(type)` form:
//!
//! ```rust
//! # #[macro_use] extern crate quick_error;
//! # fn main() {}
//! #
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum SomeError {
//!         FormatError {
//!             from(std::fmt::Error)
//!         }
//!     }
//! }
//! ```
//!
//! And the most powerful form is `from(var: type) -> (arguments...)`. It
//! might be used to convert to type with multiple arguments or for arbitrary
//! value conversions:
//!
//! ```rust
//! # #[macro_use] extern crate quick_error;
//! # fn main() {}
//! #
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum SomeError {
//!         FailedOperation(s: &'static str, errno: i32) {
//!             from(errno: i32) -> ("os error", errno)
//!             from(e: std::io::Error) -> ("io error", e.raw_os_error().unwrap())
//!         }
//!         /// Converts from both kinds of utf8 errors
//!         Utf8(err: std::str::Utf8Error) {
//!             from()
//!             from(err: std::string::FromUtf8Error) -> (err.utf8_error())
//!         }
//!     }
//! }
//! ```
//! # Context
//!
//! Since quick-error 1.1 we also have a `context` declaration, which is
//! similar to (the longest form of) `from`, but allows adding some context to
//! the error. We need a longer example to demonstrate this:
//!
//! ```rust
//! # #[macro_use] extern crate quick_error;
//! # use std::io;
//! # use std::fs::File;
//! # use std::path::{Path, PathBuf};
//! #
//! use quick_error::ResultExt;
//!
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum Error {
//!         File(filename: PathBuf, err: io::Error) {
//!             context(path: &'a Path, err: io::Error)
//!                 -> (path.to_path_buf(), err)
//!         }
//!     }
//! }
//!
//! fn openfile(path: &Path) -> Result<(), Error> {
//!     File::open(path).context(path)?;
//!
//!     // If we didn't have context, the line above would be written as;
//!     //
//!     // File::open(path)
//!     //     .map_err(|err| Error::File(path.to_path_buf(), err))?;
//!
//!     Ok(())
//! }
//!
//! # fn main() {
//! #     openfile(Path::new("/etc/somefile")).ok();
//! # }
//! ```
//!
//! Each `context(a: A, b: B)` clause implements
//! `From<Context<A, B>> for Error`. Which means multiple `context` clauses
//! are a subject to the normal coherence rules. Unfortunately, we can't
//! provide full support of generics for the context, but you may either use a
//! lifetime `'a` for references or `AsRef<Type>` (the latter means `A:
//! AsRef<Type>`, and `Type` must be concrete). It's also occasionally useful
//! to use a tuple as a type of the first argument.
//!
//! You also need to `use quick_error::ResultExt` extension trait to get
//! working `.context()` method.
//!
//! More info on context in [this article](http://bit.ly/1PsuxDt).
//!
//! All forms of `from`, `display`, `source`, and `context`
//! clauses can be combined and put in arbitrary order. Only `from` and
//! `context` can be used multiple times in single variant of enumeration.
//! Docstrings are also okay.  Empty braces can be omitted as of quick_error
//! 0.1.3.
//!
//! # Private Enums
//!
//! Since quick-error 1.2.0 we  have a way to make a private enum that is
//! wrapped by public structure:
//!
//! ```rust
//! #[macro_use] extern crate quick_error;
//! # fn main() {}
//!
//! quick_error! {
//!     #[derive(Debug)]
//!     pub enum PubError wraps ErrorEnum {
//!         Variant1 {}
//!     }
//! }
//! ```
//!
//! This generates data structures like this
//!
//! ```rust
//!
//! pub struct PubError(ErrorEnum);
//!
//! enum ErrorEnum {
//!     Variant1,
//! }
//!
//! ```
//!
//! Which in turn allows you to export just `PubError` in your crate and keep
//! actual enumeration private to the crate. This is useful to keep backwards
//! compatibility for error types. Currently there is no shorcuts to define
//! error constructors for the inner type, but we consider adding some in
//! future versions.
//!
//! It's possible to declare internal enum as public too.
//!
//!

/// Main macro that does all the work
#[macro_export]
macro_rules! quick_error {

    (   $(#[$meta:meta])*
        pub enum $name:ident { $($chunks:tt)* }
    ) => {
        quick_error!(SORT [pub enum $name $(#[$meta])* ]
            items [] buf []
            queue [ $($chunks)* ]);
    };
    (   $(#[$meta:meta])*
        enum $name:ident { $($chunks:tt)* }
    ) => {
        quick_error!(SORT [enum $name $(#[$meta])* ]
            items [] buf []
            queue [ $($chunks)* ]);
    };

    (   $(#[$meta:meta])*
        pub enum $name:ident wraps $enum_name:ident { $($chunks:tt)* }
    ) => {
        quick_error!(WRAPPER $enum_name [ pub struct ] $name $(#[$meta])*);
        quick_error!(SORT [enum $enum_name $(#[$meta])* ]
            items [] buf []
            queue [ $($chunks)* ]);
    };

    (   $(#[$meta:meta])*
        pub enum $name:ident wraps pub $enum_name:ident { $($chunks:tt)* }
    ) => {
        quick_error!(WRAPPER $enum_name [ pub struct ] $name $(#[$meta])*);
        quick_error!(SORT [pub enum $enum_name $(#[$meta])* ]
            items [] buf []
            queue [ $($chunks)* ]);
    };
    (   $(#[$meta:meta])*
        enum $name:ident wraps $enum_name:ident { $($chunks:tt)* }
    ) => {
        quick_error!(WRAPPER $enum_name [ struct ] $name $(#[$meta])*);
        quick_error!(SORT [enum $enum_name $(#[$meta])* ]
            items [] buf []
            queue [ $($chunks)* ]);
    };

    (   $(#[$meta:meta])*
        enum $name:ident wraps pub $enum_name:ident { $($chunks:tt)* }
    ) => {
        quick_error!(WRAPPER $enum_name [ struct ] $name $(#[$meta])*);
        quick_error!(SORT [pub enum $enum_name $(#[$meta])* ]
            items [] buf []
            queue [ $($chunks)* ]);
    };


    (
        WRAPPER $internal:ident [ $($strdef:tt)* ] $strname:ident
        $(#[$meta:meta])*
    ) => {
        $(#[$meta])*
        $($strdef)* $strname ( $internal );

        impl ::std::fmt::Display for $strname {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>)
                -> ::std::fmt::Result
            {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl From<$internal> for $strname {
            fn from(err: $internal) -> Self {
                $strname(err)
            }
        }

        impl ::std::error::Error for $strname {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.0.source()
            }
        }
    };

    // Queue is empty, can do the work
    (SORT [enum $name:ident $( #[$meta:meta] )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [ ]
        queue [ ]
    ) => {
        quick_error!(ENUM_DEFINITION [enum $name $( #[$meta] )*]
            body []
            queue [$($( #[$imeta] )*
                      => $iitem: $imode [$( $ivar: $ityp ),*] )*]
        );
        quick_error!(IMPLEMENTATIONS $name {$(
           $iitem: $imode [$(#[$imeta])*] [$( $ivar: $ityp ),*] {$( $ifuncs )*}
           )*});
        $(
            quick_error!(ERROR_CHECK $imode $($ifuncs)*);
        )*
    };
    (SORT [pub enum $name:ident $( #[$meta:meta] )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [ ]
        queue [ ]
    ) => {
        quick_error!(ENUM_DEFINITION [pub enum $name $( #[$meta] )*]
            body []
            queue [$($( #[$imeta] )*
                      => $iitem: $imode [$( $ivar: $ityp ),*] )*]
        );
        quick_error!(IMPLEMENTATIONS $name {$(
           $iitem: $imode [$(#[$imeta])*] [$( $ivar: $ityp ),*] {$( $ifuncs )*}
           )*});
        $(
            quick_error!(ERROR_CHECK $imode $($ifuncs)*);
        )*
    };
    // Add meta to buffer
    (SORT [$( $def:tt )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [$( #[$bmeta:meta] )*]
        queue [ #[$qmeta:meta] $( $tail:tt )*]
    ) => {
        quick_error!(SORT [$( $def )*]
            items [$( $(#[$imeta])* => $iitem: $imode [$( $ivar:$ityp ),*] {$( $ifuncs )*} )*]
            buf [$( #[$bmeta] )* #[$qmeta] ]
            queue [$( $tail )*]);
    };
    // Add ident to buffer
    (SORT [$( $def:tt )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [$( #[$bmeta:meta] )*]
        queue [ $qitem:ident $( $tail:tt )*]
    ) => {
        quick_error!(SORT [$( $def )*]
            items [$( $(#[$imeta])*
                      => $iitem: $imode [$( $ivar:$ityp ),*] {$( $ifuncs )*} )*]
            buf [$(#[$bmeta])* => $qitem : UNIT [ ] ]
            queue [$( $tail )*]);
    };
    // Flush buffer on meta after ident
    (SORT [$( $def:tt )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [$( #[$bmeta:meta] )*
            => $bitem:ident: $bmode:tt [$( $bvar:ident: $btyp:ty ),*] ]
        queue [ #[$qmeta:meta] $( $tail:tt )*]
    ) => {
        quick_error!(SORT [$( $def )*]
            items [$($( #[$imeta:meta] )*
                      => $iitem: $imode [$( $ivar:$ityp ),*] {$( $ifuncs )*} )*
                     $bitem: $bmode [$( $bvar:$btyp ),*] {} ]
            buf [ #[$qmeta] ]
            queue [$( $tail )*]);
    };
    // Add tuple enum-variant
    (SORT [$( $def:tt )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [$( #[$bmeta:meta] )* => $bitem:ident: UNIT [ ] ]
        queue [($( $qvar:ident: $qtyp:ty ),+) $( $tail:tt )*]
    ) => {
        quick_error!(SORT [$( $def )*]
            items [$( $(#[$imeta])* => $iitem: $imode [$( $ivar:$ityp ),*] {$( $ifuncs )*} )*]
            buf [$( #[$bmeta] )* => $bitem: TUPLE [$( $qvar:$qtyp ),+] ]
            queue [$( $tail )*]
        );
    };
    // Add struct enum-variant - e.g. { descr: &'static str }
    (SORT [$( $def:tt )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [$( #[$bmeta:meta] )* => $bitem:ident: UNIT [ ] ]
        queue [{ $( $qvar:ident: $qtyp:ty ),+} $( $tail:tt )*]
    ) => {
        quick_error!(SORT [$( $def )*]
            items [$( $(#[$imeta])* => $iitem: $imode [$( $ivar:$ityp ),*] {$( $ifuncs )*} )*]
            buf [$( #[$bmeta] )* => $bitem: STRUCT [$( $qvar:$qtyp ),+] ]
            queue [$( $tail )*]);
    };
    // Add struct enum-variant, with excess comma - e.g. { descr: &'static str, }
    (SORT [$( $def:tt )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [$( #[$bmeta:meta] )* => $bitem:ident: UNIT [ ] ]
        queue [{$( $qvar:ident: $qtyp:ty ),+ ,} $( $tail:tt )*]
    ) => {
        quick_error!(SORT [$( $def )*]
            items [$( $(#[$imeta])* => $iitem: $imode [$( $ivar:$ityp ),*] {$( $ifuncs )*} )*]
            buf [$( #[$bmeta] )* => $bitem: STRUCT [$( $qvar:$qtyp ),+] ]
            queue [$( $tail )*]);
    };
    // Add braces and flush always on braces
    (SORT [$( $def:tt )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [$( #[$bmeta:meta] )*
                 => $bitem:ident: $bmode:tt [$( $bvar:ident: $btyp:ty ),*] ]
        queue [ {$( $qfuncs:tt )*} $( $tail:tt )*]
    ) => {
        quick_error!(SORT [$( $def )*]
            items [$( $(#[$imeta])* => $iitem: $imode [$( $ivar:$ityp ),*] {$( $ifuncs )*} )*
                      $(#[$bmeta])* => $bitem: $bmode [$( $bvar:$btyp ),*] {$( $qfuncs )*} ]
            buf [ ]
            queue [$( $tail )*]);
    };
    // Flush buffer on double ident
    (SORT [$( $def:tt )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [$( #[$bmeta:meta] )*
                 => $bitem:ident: $bmode:tt [$( $bvar:ident: $btyp:ty ),*] ]
        queue [ $qitem:ident $( $tail:tt )*]
    ) => {
        quick_error!(SORT [$( $def )*]
            items [$( $(#[$imeta])* => $iitem: $imode [$( $ivar:$ityp ),*] {$( $ifuncs )*} )*
                     $(#[$bmeta])* => $bitem: $bmode [$( $bvar:$btyp ),*] {} ]
            buf [ => $qitem : UNIT [ ] ]
            queue [$( $tail )*]);
    };
    // Flush buffer on end
    (SORT [$( $def:tt )*]
        items [$($( #[$imeta:meta] )*
                  => $iitem:ident: $imode:tt [$( $ivar:ident: $ityp:ty ),*]
                                {$( $ifuncs:tt )*} )* ]
        buf [$( #[$bmeta:meta] )*
            => $bitem:ident: $bmode:tt [$( $bvar:ident: $btyp:ty ),*] ]
        queue [ ]
    ) => {
        quick_error!(SORT [$( $def )*]
            items [$( $(#[$imeta])* => $iitem: $imode [$( $ivar:$ityp ),*] {$( $ifuncs )*} )*
                     $(#[$bmeta])* => $bitem: $bmode [$( $bvar:$btyp ),*] {} ]
            buf [ ]
            queue [ ]);
    };
    // Public enum (Queue Empty)
    (ENUM_DEFINITION [pub enum $name:ident $( #[$meta:meta] )*]
        body [$($( #[$imeta:meta] )*
            => $iitem:ident ($(($( $ttyp:ty ),+))*) {$({$( $svar:ident: $styp:ty ),*})*} )* ]
        queue [ ]
    ) => {
        #[allow(unknown_lints)]  // no unused_doc_comments in older rust
        #[allow(renamed_and_removed_lints)]
        #[allow(unused_doc_comment)]
        #[allow(unused_doc_comments)]
        $(#[$meta])*
        pub enum $name {
            $(
                $(#[$imeta])*
                $iitem $(($( $ttyp ),+))* $({$( $svar: $styp ),*})*,
            )*
        }
    };
    // Private enum (Queue Empty)
    (ENUM_DEFINITION [enum $name:ident $( #[$meta:meta] )*]
        body [$($( #[$imeta:meta] )*
            => $iitem:ident ($(($( $ttyp:ty ),+))*) {$({$( $svar:ident: $styp:ty ),*})*} )* ]
        queue [ ]
    ) => {
        #[allow(unknown_lints)]  // no unused_doc_comments in older rust
        #[allow(renamed_and_removed_lints)]
        #[allow(unused_doc_comment)]
        #[allow(unused_doc_comments)]
        $(#[$meta])*
        enum $name {
            $(
                $(#[$imeta])*
                $iitem $(($( $ttyp ),+))* $({$( $svar: $styp ),*})*,
            )*
        }
    };
    // Unit variant
    (ENUM_DEFINITION [$( $def:tt )*]
        body [$($( #[$imeta:meta] )*
            => $iitem:ident ($(($( $ttyp:ty ),+))*) {$({$( $svar:ident: $styp:ty ),*})*} )* ]
        queue [$( #[$qmeta:meta] )*
            => $qitem:ident: UNIT [ ] $( $queue:tt )*]
    ) => {
        quick_error!(ENUM_DEFINITION [ $($def)* ]
            body [$($( #[$imeta] )* => $iitem ($(($( $ttyp ),+))*) {$({$( $svar: $styp ),*})*} )*
                    $( #[$qmeta] )* => $qitem () {} ]
            queue [ $($queue)* ]
        );
    };
    // Tuple variant
    (ENUM_DEFINITION [$( $def:tt )*]
        body [$($( #[$imeta:meta] )*
            => $iitem:ident ($(($( $ttyp:ty ),+))*) {$({$( $svar:ident: $styp:ty ),*})*} )* ]
        queue [$( #[$qmeta:meta] )*
            => $qitem:ident: TUPLE [$( $qvar:ident: $qtyp:ty ),+] $( $queue:tt )*]
    ) => {
        quick_error!(ENUM_DEFINITION [ $($def)* ]
            body [$($( #[$imeta] )* => $iitem ($(($( $ttyp ),+))*) {$({$( $svar: $styp ),*})*} )*
                    $( #[$qmeta] )* => $qitem (($( $qtyp ),+)) {} ]
            queue [ $($queue)* ]
        );
    };
    // Struct variant
    (ENUM_DEFINITION [$( $def:tt )*]
        body [$($( #[$imeta:meta] )*
            => $iitem:ident ($(($( $ttyp:ty ),+))*) {$({$( $svar:ident: $styp:ty ),*})*} )* ]
        queue [$( #[$qmeta:meta] )*
            => $qitem:ident: STRUCT [$( $qvar:ident: $qtyp:ty ),*] $( $queue:tt )*]
    ) => {
        quick_error!(ENUM_DEFINITION [ $($def)* ]
            body [$($( #[$imeta] )* => $iitem ($(($( $ttyp ),+))*) {$({$( $svar: $styp ),*})*} )*
                    $( #[$qmeta] )* => $qitem () {{$( $qvar: $qtyp ),*}} ]
            queue [ $($queue)* ]
        );
    };
    (IMPLEMENTATIONS
        $name:ident {$(
            $item:ident: $imode:tt [$(#[$imeta:meta])*] [$( $var:ident: $typ:ty ),*] {$( $funcs:tt )*}
        )*}
    ) => {
        #[allow(unused_variables)]
        #[allow(unknown_lints)]  // no unused_doc_comments in older rust
        #[allow(renamed_and_removed_lints)]
        #[allow(unused_doc_comment)]
        #[allow(unused_doc_comments)]
        impl ::std::fmt::Display for $name {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter<'_>)
                -> ::std::fmt::Result
            {
                match *self {
                    $(
                        $(#[$imeta])*
                        quick_error!(ITEM_PATTERN
                            $name $item: $imode [$( ref $var ),*]
                        ) => {
                            let display_fn = quick_error!(FIND_DISPLAY_IMPL
                                $name $item: $imode
                                {$( $funcs )*});

                            display_fn(self, fmt)
                        }
                    )*
                }
            }
        }
        #[allow(unused_variables)]
        #[allow(unknown_lints)]  // no unused_doc_comments in older rust
        #[allow(renamed_and_removed_lints)]
        #[allow(unused_doc_comment)]
        #[allow(unused_doc_comments)]
        impl ::std::error::Error for $name {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match *self {
                    $(
                        $(#[$imeta])*
                        quick_error!(ITEM_PATTERN
                            $name $item: $imode [$( ref $var ),*]
                        ) => {
                            quick_error!(FIND_SOURCE_IMPL
                                $item: $imode [$( $var ),*]
                                {$( $funcs )*})
                        }
                    )*
                }
            }
        }
        $(
            quick_error!(FIND_FROM_IMPL
                $name $item: $imode [$( $var:$typ ),*]
                {$( $funcs )*});
        )*
        $(
            quick_error!(FIND_CONTEXT_IMPL
                $name $item: $imode [$( $var:$typ ),*]
                {$( $funcs )*});
        )*
    };
    (FIND_DISPLAY_IMPL $name:ident $item:ident: $imode:tt
        { display($self_:tt) -> ($( $exprs:tt )*) $( $tail:tt )*}
    ) => {
        |quick_error!(IDENT $self_): &$name, f: &mut ::std::fmt::Formatter<'_>| { write!(f, $( $exprs )*) }
    };
    (FIND_DISPLAY_IMPL $name:ident $item:ident: $imode:tt
        { display($pattern:expr) $( $tail:tt )*}
    ) => {
        |_, f: &mut ::std::fmt::Formatter<'_>| { write!(f, $pattern) }
    };
    (FIND_DISPLAY_IMPL $name:ident $item:ident: $imode:tt
        { display($pattern:expr, $( $exprs:tt )*) $( $tail:tt )*}
    ) => {
        |_, f: &mut ::std::fmt::Formatter<'_>| { write!(f, $pattern, $( $exprs )*) }
    };
    (FIND_DISPLAY_IMPL $name:ident $item:ident: $imode:tt
        { $t:tt $( $tail:tt )*}
    ) => {
        quick_error!(FIND_DISPLAY_IMPL
            $name $item: $imode
            {$( $tail )*})
    };
    (FIND_DISPLAY_IMPL $name:ident $item:ident: $imode:tt
        { }
    ) => {
        |self_: &$name, f: &mut ::std::fmt::Formatter<'_>| {
            write!(f, "{:?}", self_)
        }
    };
    (FIND_SOURCE_IMPL $item:ident: $imode:tt
        [$( $var:ident ),*]
        { source($expr:expr) $( $tail:tt )*}
    ) => {
        Some($expr)
    };
    (FIND_SOURCE_IMPL $item:ident: $imode:tt
        [$( $var:ident ),*]
        { $t:tt $( $tail:tt )*}
    ) => {
        quick_error!(FIND_SOURCE_IMPL
            $item: $imode [$( $var ),*]
            { $($tail)* })
    };
    (FIND_SOURCE_IMPL $item:ident: $imode:tt
        [$( $var:ident ),*]
        { }
    ) => {
        None
    };
    // ----------------------------- FROM IMPL --------------------------
    (FIND_FROM_IMPL $name:ident $item:ident: $imode:tt
        [$( $var:ident: $typ:ty ),*]
        { from() $( $tail:tt )*}
    ) => {
        $(
            impl From<$typ> for $name {
                fn from($var: $typ) -> $name {
                    $name::$item($var)
                }
            }
        )*
        quick_error!(FIND_FROM_IMPL
            $name $item: $imode [$( $var:$typ ),*]
            {$( $tail )*});
    };
    (FIND_FROM_IMPL $name:ident $item:ident: UNIT
        [ ]
        { from($ftyp:ty) $( $tail:tt )*}
    ) => {
        impl From<$ftyp> for $name {
            fn from(_discarded_error: $ftyp) -> $name {
                $name::$item
            }
        }
        quick_error!(FIND_FROM_IMPL
            $name $item: UNIT [  ]
            {$( $tail )*});
    };
    (FIND_FROM_IMPL $name:ident $item:ident: TUPLE
        [$( $var:ident: $typ:ty ),*]
        { from($fvar:ident: $ftyp:ty) -> ($( $texpr:expr ),*) $( $tail:tt )*}
    ) => {
        impl From<$ftyp> for $name {
            fn from($fvar: $ftyp) -> $name {
                $name::$item($( $texpr ),*)
            }
        }
        quick_error!(FIND_FROM_IMPL
            $name $item: TUPLE [$( $var:$typ ),*]
            { $($tail)* });
    };
    (FIND_FROM_IMPL $name:ident $item:ident: STRUCT
        [$( $var:ident: $typ:ty ),*]
        { from($fvar:ident: $ftyp:ty) -> {$( $tvar:ident: $texpr:expr ),*} $( $tail:tt )*}
    ) => {
        impl From<$ftyp> for $name {
            fn from($fvar: $ftyp) -> $name {
                $name::$item {
                    $( $tvar: $texpr ),*
                }
            }
        }
        quick_error!(FIND_FROM_IMPL
            $name $item: STRUCT [$( $var:$typ ),*]
            { $($tail)* });
    };
    (FIND_FROM_IMPL $name:ident $item:ident: $imode:tt
        [$( $var:ident: $typ:ty ),*]
        { $t:tt $( $tail:tt )*}
    ) => {
        quick_error!(FIND_FROM_IMPL
            $name $item: $imode [$( $var:$typ ),*]
            {$( $tail )*}
        );
    };
    (FIND_FROM_IMPL $name:ident $item:ident: $imode:tt
        [$( $var:ident: $typ:ty ),*]
        { }
    ) => {
    };
    // ----------------------------- CONTEXT IMPL --------------------------
    (FIND_CONTEXT_IMPL $name:ident $item:ident: TUPLE
        [$( $var:ident: $typ:ty ),*]
        { context($cvar:ident: AsRef<$ctyp:ty>, $fvar:ident: $ftyp:ty)
            -> ($( $texpr:expr ),*) $( $tail:tt )* }
    ) => {
        impl<T: AsRef<$ctyp>> From<$crate::Context<T, $ftyp>> for $name {
            fn from(
                $crate::Context($cvar, $fvar): $crate::Context<T, $ftyp>)
                -> $name
            {
                $name::$item($( $texpr ),*)
            }
        }
        quick_error!(FIND_CONTEXT_IMPL
            $name $item: TUPLE [$( $var:$typ ),*]
            { $($tail)* });
    };
    (FIND_CONTEXT_IMPL $name:ident $item:ident: TUPLE
        [$( $var:ident: $typ:ty ),*]
        { context($cvar:ident: $ctyp:ty, $fvar:ident: $ftyp:ty)
            -> ($( $texpr:expr ),*) $( $tail:tt )* }
    ) => {
        impl<'a> From<$crate::Context<$ctyp, $ftyp>> for $name {
            fn from(
                $crate::Context($cvar, $fvar): $crate::Context<$ctyp, $ftyp>)
                -> $name
            {
                $name::$item($( $texpr ),*)
            }
        }
        quick_error!(FIND_CONTEXT_IMPL
            $name $item: TUPLE [$( $var:$typ ),*]
            { $($tail)* });
    };
    (FIND_CONTEXT_IMPL $name:ident $item:ident: STRUCT
        [$( $var:ident: $typ:ty ),*]
        { context($cvar:ident: AsRef<$ctyp:ty>, $fvar:ident: $ftyp:ty)
            -> {$( $tvar:ident: $texpr:expr ),*} $( $tail:tt )* }
    ) => {
        impl<T: AsRef<$ctyp>> From<$crate::Context<T, $ftyp>> for $name {
            fn from(
                $crate::Context($cvar, $fvar): $crate::Context<$ctyp, $ftyp>)
                -> $name
            {
                $name::$item {
                    $( $tvar: $texpr ),*
                }
            }
        }
        quick_error!(FIND_CONTEXT_IMPL
            $name $item: STRUCT [$( $var:$typ ),*]
            { $($tail)* });
    };
    (FIND_CONTEXT_IMPL $name:ident $item:ident: STRUCT
        [$( $var:ident: $typ:ty ),*]
        { context($cvar:ident: $ctyp:ty, $fvar:ident: $ftyp:ty)
            -> {$( $tvar:ident: $texpr:expr ),*} $( $tail:tt )* }
    ) => {
        impl<'a> From<$crate::Context<$ctyp, $ftyp>> for $name {
            fn from(
                $crate::Context($cvar, $fvar): $crate::Context<$ctyp, $ftyp>)
                -> $name
            {
                $name::$item {
                    $( $tvar: $texpr ),*
                }
            }
        }
        quick_error!(FIND_CONTEXT_IMPL
            $name $item: STRUCT [$( $var:$typ ),*]
            { $($tail)* });
    };
    (FIND_CONTEXT_IMPL $name:ident $item:ident: $imode:tt
        [$( $var:ident: $typ:ty ),*]
        { $t:tt $( $tail:tt )*}
    ) => {
        quick_error!(FIND_CONTEXT_IMPL
            $name $item: $imode [$( $var:$typ ),*]
            {$( $tail )*}
        );
    };
    (FIND_CONTEXT_IMPL $name:ident $item:ident: $imode:tt
        [$( $var:ident: $typ:ty ),*]
        { }
    ) => {
    };
    // ----------------------------- ITEM IMPL --------------------------
    (ITEM_BODY $(#[$imeta:meta])* $item:ident: UNIT
    ) => { };
    (ITEM_BODY $(#[$imeta:meta])* $item:ident: TUPLE
        [$( $typ:ty ),*]
    ) => {
        ($( $typ ),*)
    };
    (ITEM_BODY $(#[$imeta:meta])* $item:ident: STRUCT
        [$( $var:ident: $typ:ty ),*]
    ) => {
        {$( $var:$typ ),*}
    };
    (ITEM_PATTERN $name:ident $item:ident: UNIT []
    ) => {
        $name::$item
    };
    (ITEM_PATTERN $name:ident $item:ident: TUPLE
        [$( ref $var:ident ),*]
    ) => {
        $name::$item ($( ref $var ),*)
    };
    (ITEM_PATTERN $name:ident $item:ident: STRUCT
        [$( ref $var:ident ),*]
    ) => {
        $name::$item {$( ref $var ),*}
    };
    // This one should match all allowed sequences in "funcs" but not match
    // anything else.
    // This is to contrast FIND_* clauses which just find stuff they need and
    // skip everything else completely
    (ERROR_CHECK $imode:tt display($self_:tt) -> ($( $exprs:tt )*) $( $tail:tt )*)
    => { quick_error!(ERROR_CHECK $imode $($tail)*); };
    (ERROR_CHECK $imode:tt display($pattern: expr) $( $tail:tt )*)
    => { quick_error!(ERROR_CHECK $imode $($tail)*); };
    (ERROR_CHECK $imode:tt display($pattern: expr, $( $exprs:tt )*) $( $tail:tt )*)
    => { quick_error!(ERROR_CHECK $imode $($tail)*); };
    (ERROR_CHECK $imode:tt source($expr:expr) $($tail:tt)*)
    => { quick_error!(ERROR_CHECK $imode $($tail)*); };
    (ERROR_CHECK $imode:tt from() $($tail:tt)*)
    => { quick_error!(ERROR_CHECK $imode $($tail)*); };
    (ERROR_CHECK $imode:tt from($ftyp:ty) $($tail:tt)*)
    => { quick_error!(ERROR_CHECK $imode $($tail)*); };
    (ERROR_CHECK TUPLE from($fvar:ident: $ftyp:ty) -> ($( $e:expr ),*) $( $tail:tt )*)
    => { quick_error!(ERROR_CHECK TUPLE $($tail)*); };
    (ERROR_CHECK STRUCT from($fvar:ident: $ftyp:ty) -> {$( $v:ident: $e:expr ),*} $( $tail:tt )*)
    => { quick_error!(ERROR_CHECK STRUCT $($tail)*); };

    (ERROR_CHECK TUPLE context($cvar:ident: $ctyp:ty, $fvar:ident: $ftyp:ty)
        -> ($( $e:expr ),*) $( $tail:tt )*)
    => { quick_error!(ERROR_CHECK TUPLE $($tail)*); };
    (ERROR_CHECK STRUCT context($cvar:ident: $ctyp:ty, $fvar:ident: $ftyp:ty)
        -> {$( $v:ident: $e:expr ),*} $( $tail:tt )*)
    => { quick_error!(ERROR_CHECK STRUCT $($tail)*); };

    (ERROR_CHECK $imode:tt ) => {};
    // Utility functions
    (IDENT $ident:ident) => { $ident }
}

/// Generic context type
///
/// Used mostly as a transport for `ResultExt::context` method
#[derive(Debug)]
pub struct Context<X, E>(pub X, pub E);

/// Result extension trait adding a `context` method
pub trait ResultExt<T, E> {
    /// The method is use to add context information to current operation
    ///
    /// The context data is then used in error constructor to store additional
    /// information within error. For example, you may add a filename as a
    /// context for file operation. See crate documentation for the actual
    /// example.
    fn context<X>(self, x: X) -> Result<T, Context<X, E>>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn context<X>(self, x: X) -> Result<T, Context<X, E>> {
        self.map_err(|e| Context(x, e))
    }
}

#[cfg(test)]
mod test {
    use std::error::Error;
    use std::num::{ParseFloatError, ParseIntError};
    use std::path::{Path, PathBuf};
    use std::str::Utf8Error;
    use std::string::FromUtf8Error;

    use super::ResultExt;

    quick_error! {
        #[derive(Debug)]
        pub enum Bare {
            One
            Two
        }
    }

    #[test]
    fn bare_item_direct() {
        assert_eq!(format!("{}", Bare::One), "One".to_string());
        assert_eq!(format!("{:?}", Bare::One), "One".to_string());
        assert!(Bare::One.source().is_none());
    }

    #[test]
    fn bare_item_trait() {
        let err: &dyn Error = &Bare::Two;
        assert_eq!(format!("{}", err), "Two".to_string());
        assert_eq!(format!("{:?}", err), "Two".to_string());
        assert!(err.source().is_none());
    }

    quick_error! {
        #[derive(Debug)]
        pub enum Wrapper wraps Wrapped {
            One
            Two(s: String) {
                display("two: {}", s)
                from()
            }
        }
    }

    #[test]
    fn wrapper() {
        assert_eq!(
            format!("{}", Wrapper::from(Wrapped::One)),
            "One".to_string()
        );
        assert_eq!(
            format!("{}", Wrapper::from(Wrapped::from(String::from("hello")))),
            "two: hello".to_string()
        );
        assert_eq!(
            format!("{:?}", Wrapper::from(Wrapped::One)),
            "Wrapper(One)".to_string()
        );
    }

    quick_error! {
        #[derive(Debug, PartialEq)]
        pub enum TupleWrapper {
            /// ParseFloat Error
            ParseFloatError(err: ParseFloatError) {
                from()
                display("parse float error: {err}", err=err)
                source(err)
            }
            Other(descr: &'static str) {
                display("Error: {}", descr)
            }
            /// FromUtf8 Error
            FromUtf8Error(err: Utf8Error, source: Vec<u8>) {
                source(err)
                display(me) -> ("{desc} at index {pos}: {err}", desc="utf8 error", pos=err.valid_up_to(), err=err)
                from(err: FromUtf8Error) -> (err.utf8_error().clone(), err.into_bytes())
            }
            Discard {
                from(&'static str)
            }
            Singleton {
                display("Just a string")
            }
        }
    }

    #[test]
    fn tuple_wrapper_err() {
        let source = "one and a half times pi".parse::<f32>().unwrap_err();
        let err = TupleWrapper::ParseFloatError(source.clone());
        assert_eq!(format!("{}", err), format!("parse float error: {}", source));
        assert_eq!(
            format!("{:?}", err),
            format!("ParseFloatError({:?})", source)
        );
        assert_eq!(
            format!("{:?}", err.source().unwrap()),
            format!("{:?}", source)
        );
    }

    #[test]
    fn tuple_wrapper_trait_str() {
        let desc = "hello";
        let err: &dyn Error = &TupleWrapper::Other(desc);
        assert_eq!(format!("{}", err), format!("Error: {}", desc));
        assert_eq!(format!("{:?}", err), format!("Other({:?})", desc));
        assert!(err.source().is_none());
    }

    #[test]
    fn tuple_wrapper_trait_two_fields() {
        let invalid_utf8: Vec<u8> = vec![0, 159, 146, 150];
        let source = String::from_utf8(invalid_utf8.clone())
            .unwrap_err()
            .utf8_error();
        let err: &dyn Error = &TupleWrapper::FromUtf8Error(source.clone(), invalid_utf8.clone());
        assert_eq!(
            format!("{}", err),
            format!(
                "{desc} at index {pos}: {source}",
                desc = "utf8 error",
                pos = source.valid_up_to(),
                source = source
            )
        );
        assert_eq!(
            format!("{:?}", err),
            format!("FromUtf8Error({:?}, {:?})", source, invalid_utf8)
        );
        assert_eq!(
            format!("{:?}", err.source().unwrap()),
            format!("{:?}", source)
        );
    }

    #[test]
    fn tuple_wrapper_from() {
        let source = "one and a half times pi".parse::<f32>().unwrap_err();
        let err = TupleWrapper::ParseFloatError(source.clone());
        let err_from: TupleWrapper = From::from(source);
        assert_eq!(err_from, err);
    }

    #[test]
    fn tuple_wrapper_custom_from() {
        let invalid_utf8: Vec<u8> = vec![0, 159, 146, 150];
        let source = String::from_utf8(invalid_utf8.clone()).unwrap_err();
        let err = TupleWrapper::FromUtf8Error(source.utf8_error().clone(), invalid_utf8);
        let err_from: TupleWrapper = From::from(source);
        assert_eq!(err_from, err);
    }

    #[test]
    fn tuple_wrapper_discard() {
        let err: TupleWrapper = From::from("hello");
        assert_eq!(format!("{}", err), format!("Discard"));
        assert_eq!(format!("{:?}", err), format!("Discard"));
        assert!(err.source().is_none());
    }

    #[test]
    fn tuple_wrapper_singleton() {
        let err: TupleWrapper = TupleWrapper::Singleton;
        assert_eq!(format!("{}", err), format!("Just a string"));
        assert_eq!(format!("{:?}", err), format!("Singleton"));
        assert!(err.source().is_none());
    }

    quick_error! {
        #[derive(Debug, PartialEq)]
        pub enum StructWrapper {
            // Utf8 Error
            Utf8Error{ err: Utf8Error, hint: Option<&'static str> } {
                source(err)
                display(me) -> ("{desc} at index {pos}: {err}", desc="utf8 error", pos=err.valid_up_to(), err=err)
                from(err: Utf8Error) -> { err: err, hint: None }
            }
            // Utf8 Error
            ExcessComma { descr: &'static str, } {
                display("Error: {}", descr)
            }
        }
    }

    #[test]
    fn struct_wrapper_err() {
        let invalid_utf8: Vec<u8> = vec![0, 159, 146, 150];
        let source = String::from_utf8(invalid_utf8.clone())
            .unwrap_err()
            .utf8_error();
        let err: &dyn Error = &StructWrapper::Utf8Error {
            err: source.clone(),
            hint: Some("nonsense"),
        };
        assert_eq!(
            format!("{}", err),
            format!(
                "{desc} at index {pos}: {source}",
                desc = "utf8 error",
                pos = source.valid_up_to(),
                source = source
            )
        );
        assert_eq!(
            format!("{:?}", err),
            format!(
                "Utf8Error {{ err: {:?}, hint: {:?} }}",
                source,
                Some("nonsense")
            )
        );
        assert_eq!(
            format!("{:?}", err.source().unwrap()),
            format!("{:?}", source)
        );
    }

    #[test]
    fn struct_wrapper_struct_from() {
        let invalid_utf8: Vec<u8> = vec![0, 159, 146, 150];
        let source = String::from_utf8(invalid_utf8.clone())
            .unwrap_err()
            .utf8_error();
        let err = StructWrapper::Utf8Error {
            err: source.clone(),
            hint: None,
        };
        let err_from: StructWrapper = From::from(source);
        assert_eq!(err_from, err);
    }

    #[test]
    fn struct_wrapper_excess_comma() {
        let descr = "hello";
        let err = StructWrapper::ExcessComma { descr: descr };
        assert_eq!(format!("{}", err), format!("Error: {}", descr));
        assert_eq!(
            format!("{:?}", err),
            format!("ExcessComma {{ descr: {:?} }}", descr)
        );
        assert!(err.source().is_none());
    }

    quick_error! {
        #[derive(Debug)]
        pub enum ContextErr {
            Float(src: String, err: ParseFloatError) {
                context(s: &'a str, e: ParseFloatError) -> (s.to_string(), e)
                display("Float error {:?}: {}", src, err)
            }
            Int { src: String, err: ParseIntError } {
                context(s: &'a str, e: ParseIntError)
                    -> {src: s.to_string(), err: e}
                display("Int error {:?}: {}", src, err)
            }
            Utf8(path: PathBuf, err: Utf8Error) {
                context(p: AsRef<Path>, e: Utf8Error)
                    -> (p.as_ref().to_path_buf(), e)
                display("Path error at {:?}: {}", path, err)
            }
            Utf8Str(s: String, err: ::std::io::Error) {
                context(s: AsRef<str>, e: ::std::io::Error)
                    -> (s.as_ref().to_string(), e)
                display("Str error {:?}: {}", s, err)
            }
        }
    }

    #[test]
    fn parse_float_error() {
        fn parse_float(s: &str) -> Result<f32, ContextErr> {
            Ok(s.parse().context(s)?)
        }
        assert_eq!(
            format!("{}", parse_float("12ab").unwrap_err()),
            r#"Float error "12ab": invalid float literal"#
        );
    }

    #[test]
    fn parse_int_error() {
        fn parse_int(s: &str) -> Result<i32, ContextErr> {
            Ok(s.parse().context(s)?)
        }
        assert_eq!(
            format!("{}", parse_int("12.5").unwrap_err()),
            r#"Int error "12.5": invalid digit found in string"#
        );
    }

    #[test]
    fn debug_context() {
        fn parse_int(s: &str) -> i32 {
            s.parse().context(s).unwrap()
        }
        assert_eq!(parse_int("12"), 12);
        assert_eq!(
            format!("{:?}", "x".parse::<i32>().context("x")),
            r#"Err(Context("x", ParseIntError { kind: InvalidDigit }))"#
        );
    }

    #[test]
    fn path_context() {
        fn parse_utf<P: AsRef<Path>>(s: &[u8], p: P) -> Result<(), ContextErr> {
            ::std::str::from_utf8(s).context(p)?;
            Ok(())
        }
        let etext = parse_utf(b"a\x80\x80", "/etc").unwrap_err().to_string();
        assert!(etext.starts_with("Path error at \"/etc\": invalid utf-8"));
        let etext = parse_utf(b"\x80\x80", PathBuf::from("/tmp"))
            .unwrap_err()
            .to_string();
        assert!(etext.starts_with("Path error at \"/tmp\": invalid utf-8"));
    }

    #[test]
    fn conditional_compilation() {
        quick_error! {
            #[allow(dead_code)]
            #[derive(Debug)]
            pub enum Test {
                #[cfg(feature = "foo")]
                Variant
            }
        }
    }

    #[test]
    #[allow(deprecated)]
    fn cause_struct_wrapper_err() {
        let invalid_utf8: Vec<u8> = vec![0, 159, 146, 150];
        let cause = String::from_utf8(invalid_utf8.clone())
            .unwrap_err()
            .utf8_error();
        let err: &dyn Error = &StructWrapper::Utf8Error {
            err: cause.clone(),
            hint: Some("nonsense"),
        };
        assert_eq!(
            format!("{}", err),
            format!(
                "{desc} at index {pos}: {cause}",
                desc = "utf8 error",
                pos = cause.valid_up_to(),
                cause = cause
            )
        );
        assert_eq!(
            format!("{:?}", err),
            format!(
                "Utf8Error {{ err: {:?}, hint: {:?} }}",
                cause,
                Some("nonsense")
            )
        );
        assert_eq!(
            format!("{:?}", err.cause().unwrap()),
            format!("{:?}", cause)
        );
    }
}
