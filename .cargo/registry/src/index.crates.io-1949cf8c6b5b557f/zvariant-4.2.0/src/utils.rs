use std::slice::SliceIndex;

#[cfg(feature = "gvariant")]
use crate::signature_parser::SignatureParser;
use crate::{serialized::Format, Basic, Error, ObjectPath, Result, Signature};

#[cfg(unix)]
use crate::Fd;

/// The prefix of ARRAY type signature, as a character. Provided for manual signature creation.
pub const ARRAY_SIGNATURE_CHAR: char = 'a';
/// The prefix of ARRAY type signature, as a string. Provided for manual signature creation.
pub const ARRAY_SIGNATURE_STR: &str = "a";
pub(crate) const ARRAY_ALIGNMENT_DBUS: usize = 4;
/// The opening character of STRUCT type signature. Provided for manual signature creation.
pub const STRUCT_SIG_START_CHAR: char = '(';
/// The closing character of STRUCT type signature. Provided for manual signature creation.
pub const STRUCT_SIG_END_CHAR: char = ')';
/// The opening character of STRUCT type signature, as a string. Provided for manual signature
/// creation.
pub const STRUCT_SIG_START_STR: &str = "(";
/// The closing character of STRUCT type signature, as a string. Provided for manual signature
/// creation.
pub const STRUCT_SIG_END_STR: &str = ")";
pub(crate) const STRUCT_ALIGNMENT_DBUS: usize = 8;
/// The opening character of DICT_ENTRY type signature. Provided for manual signature creation.
pub const DICT_ENTRY_SIG_START_CHAR: char = '{';
/// The closing character of DICT_ENTRY type signature. Provided for manual signature creation.
pub const DICT_ENTRY_SIG_END_CHAR: char = '}';
/// The opening character of DICT_ENTRY type signature, as a string. Provided for manual signature
/// creation.
pub const DICT_ENTRY_SIG_START_STR: &str = "{";
/// The closing character of DICT_ENTRY type signature, as a string. Provided for manual signature
/// creation.
pub const DICT_ENTRY_SIG_END_STR: &str = "}";
pub(crate) const DICT_ENTRY_ALIGNMENT_DBUS: usize = 8;
/// The VARIANT type signature. Provided for manual signature creation.
pub const VARIANT_SIGNATURE_CHAR: char = 'v';
/// The VARIANT type signature, as a string. Provided for manual signature creation.
pub const VARIANT_SIGNATURE_STR: &str = "v";
pub(crate) const VARIANT_ALIGNMENT_DBUS: usize = 1;
#[cfg(feature = "gvariant")]
pub(crate) const VARIANT_ALIGNMENT_GVARIANT: usize = 8;
/// The prefix of MAYBE (GVariant-specific) type signature, as a character. Provided for manual
/// signature creation.
#[cfg(feature = "gvariant")]
pub const MAYBE_SIGNATURE_CHAR: char = 'm';
/// The prefix of MAYBE (GVariant-specific) type signature, as a string. Provided for manual
/// signature creation.
#[cfg(feature = "gvariant")]
pub const MAYBE_SIGNATURE_STR: &str = "m";

pub(crate) fn padding_for_n_bytes(value: usize, align: usize) -> usize {
    let len_rounded_up = value.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);

    len_rounded_up.wrapping_sub(value)
}

pub(crate) fn usize_to_u32(value: usize) -> u32 {
    assert!(
        value <= (u32::MAX as usize),
        "{} too large for `u32`",
        value,
    );

    value as u32
}

pub(crate) fn usize_to_u8(value: usize) -> u8 {
    assert!(value <= (u8::MAX as usize), "{} too large for `u8`", value,);

    value as u8
}

pub(crate) fn f64_to_f32(value: f64) -> f32 {
    assert!(value <= (f32::MAX as f64), "{} too large for `f32`", value,);

    value as f32
}

// `signature` must be **one** complete and correct signature. Expect panics otherwise!
pub(crate) fn alignment_for_signature(signature: &Signature<'_>, format: Format) -> Result<usize> {
    let alignment = match signature
        .as_bytes()
        .first()
        .map(|b| *b as char)
        .ok_or_else(|| -> Error { serde::de::Error::invalid_length(0, &">= 1 character") })?
    {
        u8::SIGNATURE_CHAR => u8::alignment(format),
        bool::SIGNATURE_CHAR => bool::alignment(format),
        i16::SIGNATURE_CHAR => i16::alignment(format),
        u16::SIGNATURE_CHAR => u16::alignment(format),
        i32::SIGNATURE_CHAR => i32::alignment(format),
        u32::SIGNATURE_CHAR => u32::alignment(format),
        #[cfg(unix)]
        Fd::SIGNATURE_CHAR => u32::alignment(format),
        i64::SIGNATURE_CHAR => i64::alignment(format),
        u64::SIGNATURE_CHAR => u64::alignment(format),
        f64::SIGNATURE_CHAR => f64::alignment(format),
        <&str>::SIGNATURE_CHAR => <&str>::alignment(format),
        ObjectPath::SIGNATURE_CHAR => ObjectPath::alignment(format),
        Signature::SIGNATURE_CHAR => Signature::alignment(format),
        VARIANT_SIGNATURE_CHAR => match format {
            Format::DBus => VARIANT_ALIGNMENT_DBUS,
            #[cfg(feature = "gvariant")]
            Format::GVariant => VARIANT_ALIGNMENT_GVARIANT,
        },
        ARRAY_SIGNATURE_CHAR => alignment_for_array_signature(signature, format)?,
        STRUCT_SIG_START_CHAR => alignment_for_struct_signature(signature, format)?,
        DICT_ENTRY_SIG_START_CHAR => alignment_for_dict_entry_signature(signature, format)?,
        #[cfg(feature = "gvariant")]
        MAYBE_SIGNATURE_CHAR => alignment_for_maybe_signature(signature, format)?,
        _ => {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(signature),
                &"a valid signature",
            ))
        }
    };

    Ok(alignment)
}

#[cfg(feature = "gvariant")]
pub(crate) fn is_fixed_sized_signature<'a>(signature: &'a Signature<'a>) -> Result<bool> {
    match signature
        .as_bytes()
        .first()
        .map(|b| *b as char)
        .ok_or_else(|| -> Error { serde::de::Error::invalid_length(0, &">= 1 character") })?
    {
        u8::SIGNATURE_CHAR
        | bool::SIGNATURE_CHAR
        | i16::SIGNATURE_CHAR
        | u16::SIGNATURE_CHAR
        | i32::SIGNATURE_CHAR
        | u32::SIGNATURE_CHAR
        | i64::SIGNATURE_CHAR
        | u64::SIGNATURE_CHAR
        | f64::SIGNATURE_CHAR => Ok(true),
        #[cfg(unix)]
        Fd::SIGNATURE_CHAR => Ok(true),
        STRUCT_SIG_START_CHAR => is_fixed_sized_struct_signature(signature),
        DICT_ENTRY_SIG_START_CHAR => is_fixed_sized_dict_entry_signature(signature),
        _ => Ok(false),
    }
}

// Given an &str, create an owned (String-based) Signature w/ appropriate capacity
macro_rules! signature_string {
    ($signature:expr) => {{
        let mut s = String::with_capacity(255);
        s.push_str($signature);

        Signature::from_string_unchecked(s)
    }};
}

macro_rules! check_child_value_signature {
    ($expected_signature:expr, $child_signature:expr, $child_name:literal) => {{
        if $child_signature != $expected_signature {
            let unexpected = format!("{} with signature `{}`", $child_name, $child_signature,);
            let expected = format!("{} with signature `{}`", $child_name, $expected_signature);

            return Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Str(&unexpected),
                &expected.as_str(),
            ));
        }
    }};
}

fn alignment_for_single_child_type_signature(
    #[allow(unused)] signature: &Signature<'_>,
    format: Format,
    dbus_align: usize,
) -> Result<usize> {
    match format {
        Format::DBus => Ok(dbus_align),
        #[cfg(feature = "gvariant")]
        Format::GVariant => {
            let child_signature = signature.slice(1..);

            alignment_for_signature(&child_signature, format)
        }
    }
}

fn alignment_for_array_signature(signature: &Signature<'_>, format: Format) -> Result<usize> {
    alignment_for_single_child_type_signature(signature, format, ARRAY_ALIGNMENT_DBUS)
}

#[cfg(feature = "gvariant")]
fn alignment_for_maybe_signature(signature: &Signature<'_>, format: Format) -> Result<usize> {
    alignment_for_single_child_type_signature(signature, format, 1)
}

fn alignment_for_struct_signature(
    #[allow(unused)] signature: &Signature<'_>,
    format: Format,
) -> Result<usize> {
    match format {
        Format::DBus => Ok(STRUCT_ALIGNMENT_DBUS),
        #[cfg(feature = "gvariant")]
        Format::GVariant => {
            if signature.len() < 3 {
                return Err(serde::de::Error::invalid_length(
                    signature.len(),
                    &">= 3 characters in struct signature",
                ));
            }
            let inner_signature = Signature::from_str_unchecked(&signature[1..signature.len() - 1]);
            let mut sig_parser = SignatureParser::new(inner_signature);
            let mut alignment = 0;

            while !sig_parser.done() {
                let child_signature = sig_parser.parse_next_signature()?;

                let child_alignment = alignment_for_signature(&child_signature, format)?;
                if child_alignment > alignment {
                    alignment = child_alignment;

                    if alignment == 8 {
                        // 8 bytes is max alignment so we can short-circuit here
                        break;
                    }
                }
            }

            Ok(alignment)
        }
    }
}

fn alignment_for_dict_entry_signature(
    #[allow(unused)] signature: &Signature<'_>,
    format: Format,
) -> Result<usize> {
    match format {
        Format::DBus => Ok(DICT_ENTRY_ALIGNMENT_DBUS),
        #[cfg(feature = "gvariant")]
        Format::GVariant => {
            if signature.len() < 4 {
                return Err(serde::de::Error::invalid_length(
                    signature.len(),
                    &">= 4 characters in dict entry signature",
                ));
            }
            let key_signature = Signature::from_str_unchecked(&signature[1..2]);
            let key_alignment = alignment_for_signature(&key_signature, format)?;
            if key_alignment == 8 {
                // 8 bytes is max alignment so we can short-circuit here
                return Ok(8);
            }

            let value_signature = Signature::from_str_unchecked(&signature[2..signature.len() - 1]);
            let value_alignment = alignment_for_signature(&value_signature, format)?;
            if value_alignment > key_alignment {
                Ok(value_alignment)
            } else {
                Ok(key_alignment)
            }
        }
    }
}

#[cfg(feature = "gvariant")]
fn is_fixed_sized_struct_signature<'a>(signature: &'a Signature<'a>) -> Result<bool> {
    let inner_signature = Signature::from_str_unchecked(&signature[1..signature.len() - 1]);
    let mut sig_parser = SignatureParser::new(inner_signature);
    let mut fixed_sized = true;

    while !sig_parser.done() {
        let child_signature = sig_parser.parse_next_signature()?;

        if !is_fixed_sized_signature(&child_signature)? {
            // STRUCT is fixed-sized only if all its children are
            fixed_sized = false;

            break;
        }
    }

    Ok(fixed_sized)
}

#[cfg(feature = "gvariant")]
fn is_fixed_sized_dict_entry_signature<'a>(signature: &'a Signature<'a>) -> Result<bool> {
    let key_signature = Signature::from_str_unchecked(&signature[1..2]);
    if !is_fixed_sized_signature(&key_signature)? {
        return Ok(false);
    }

    let value_signature = Signature::from_str_unchecked(&signature[2..signature.len() - 1]);

    is_fixed_sized_signature(&value_signature)
}

/// Slice the given slice of bytes safely and return an error if the slice is too small.
pub(crate) fn subslice<I, T>(input: &[T], index: I) -> Result<&I::Output>
where
    I: SliceIndex<[T]>,
{
    input.get(index).ok_or(Error::OutOfBounds)
}
