// Copyright 2024 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

/// Reduces boilerplate for defining error types where we want the compiler to
/// optimize for the non-error path by assuming constructing an error is
/// unlikely/cold code.
///
/// WARNING: Every struct/variant must contain some *non-constant* value so
/// that the "invariant code" pass of the compiler doesn't recognize the
/// constructor as being "invariant code" and optimizing it away;
/// although such optimization would be nice to take advantage of, it
/// seems to lose the `#[cold]` attribute.
///
/// Constructor functions ar marked `pub(super)` to ensure that instances can
/// only be constructed from within the enclosing module (and its submodules).
///
/// XXX: #[inline(never)] is required to avoid the (MIR?) optimizer inlining
/// away the function call and losing the `#[cold]` attribute in the process.
/// We'd otherwise maybe prefer all constructors to be inline.
///
/// The type is defined in its own submodule `#mod_name` to hide the
/// variant/struct constructor, ensuring instances are only constructed
/// through the generated `$constructor` functions. The constructor methods
/// work around the lack of the ability to mark an enum variant `#[cold]` and
/// `#[inline(never)]`.
macro_rules! cold_exhaustive_error {
    // struct
    {
        struct $mod_name:ident::$Error:ident with $vis:vis constructor {
            $field:ident: $ValueType:ty
        }
    } => {
        mod $mod_name {
            #[allow(unused_imports)]
            use super::*; // So `$ValueType` is in scope.

            pub struct $Error { #[allow(dead_code)] $field: $ValueType }

            impl $Error {
                #[cold]
                #[inline(never)]
                $vis fn new($field: $ValueType) -> Self {
                    Self { $field }
                }
            }
        }
    };
    // struct with default constructor visibility.
    {
        struct $mod_name:ident::$Error:ident {
            $field:ident: $ValueType:ty
        }
    } => {
        cold_exhaustive_error! {
            struct $mod_name::$Error with pub(super) constructor {
                $field: $ValueType
            }
        }
    };

    // enum
    {
        enum $mod_name:ident::$Error:ident {
            $(
                $constructor:ident => $Variant:ident($ValueType:ty),
            )+
        }
    } => {
        mod $mod_name {
            #[allow(unused_imports)]
            use super::*; // So `$ValueType` is in scope.

            pub enum $Error {
                $(
                    $Variant(#[allow(dead_code)] $ValueType)
                ),+
            }

            impl $Error {
                $(
                    #[cold]
                    #[inline(never)]
                    pub(super) fn $constructor(value: $ValueType) -> Self {
                        Self::$Variant(value)
                    }
                )+
            }
        }
    };
}
