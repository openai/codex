// Copyright 2018 Trent Clarke.
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

// Generates an implementation of the Debug trait for a type that defers to the
// Debug implementation for a given field.
macro_rules! derive_debug_via_id {
    ($typename:ident) => {
        impl ::core::fmt::Debug for $typename {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> Result<(), ::core::fmt::Error> {
                ::core::fmt::Debug::fmt(&self.id, f)
            }
        }
    };
}

macro_rules! derive_debug_via_field {
    ($type:ty, $field:ident) => {
        derive_debug_via_field!($type, stringify!($type), $field);
    };

    ($type:ty, $typename:expr, $field:ident) => {
        impl ::core::fmt::Debug for $type {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> Result<(), ::core::fmt::Error> {
                f.debug_struct($typename)
                    .field(stringify!($field), &self.$field)
                    .finish()
            }
        }
    };
}

// Generates an implementation of the Debug trait for a type that outputs the
// hex encoding of the byte slice representation of the value.
macro_rules! derive_debug_self_as_ref_hex_bytes {
    ($typename:ident) => {
        impl ::core::fmt::Debug for $typename {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> Result<(), ::core::fmt::Error> {
                crate::debug::write_hex_tuple(f, stringify!($typename), self)
            }
        }
    };
}

pub(crate) fn write_hex_tuple(
    fmt: &mut core::fmt::Formatter,
    type_name: &str,
    value: &dyn AsRef<[u8]>,
) -> Result<(), ::core::fmt::Error> {
    fmt.debug_tuple(type_name)
        .field(&HexStr(value.as_ref()))
        .finish()
}

pub struct HexStr<'a>(pub &'a [u8]);

impl core::fmt::Debug for HexStr<'_> {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        fmt.write_str("\"")?;
        write_hex_bytes(fmt, self.0)?;
        fmt.write_str("\"")?;
        Ok(())
    }
}

pub(crate) fn write_hex_bytes(
    fmt: &mut core::fmt::Formatter,
    bytes: &[u8],
) -> Result<(), ::core::fmt::Error> {
    for byte in bytes {
        write!(fmt, "{:02x}", byte)?;
    }
    Ok(())
}
