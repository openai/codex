use crate::{serialized::Format, Signature, Type};

/// Trait for basic types.
///
/// All basic types are also [`Type`] implementers.
///
/// [`Type`]: trait.Type.html
/// [`Value`]: enum.Value.html
pub trait Basic: Type {
    /// The type signature, as a character.
    const SIGNATURE_CHAR: char;
    /// The type signature, as a string.
    const SIGNATURE_STR: &'static str;

    /// The required padding alignment for the given format.
    fn alignment(format: Format) -> usize;
}

impl<B: ?Sized> Basic for &B
where
    B: Basic,
{
    const SIGNATURE_CHAR: char = B::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = B::SIGNATURE_STR;

    fn alignment(format: Format) -> usize {
        B::alignment(format)
    }
}

macro_rules! impl_type {
    ($for:ty) => {
        impl Type for $for {
            fn signature() -> Signature<'static> {
                Signature::from_static_str_unchecked(<$for>::SIGNATURE_STR)
            }
        }
    };
}

macro_rules! alignment_method {
    ($alignment:expr) => {
        alignment_method!($alignment, $alignment);
    };
    ($dbus_alignment:expr, $gvariant_alignment:expr) => {
        fn alignment(format: Format) -> usize {
            match format {
                Format::DBus => $dbus_alignment,
                #[cfg(feature = "gvariant")]
                Format::GVariant => $gvariant_alignment,
            }
        }
    };
}

impl Basic for u8 {
    const SIGNATURE_CHAR: char = 'y';
    const SIGNATURE_STR: &'static str = "y";

    alignment_method!(1);
}
impl_type!(u8);

impl Basic for std::num::NonZeroU8 {
    const SIGNATURE_CHAR: char = u8::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = u8::SIGNATURE_STR;

    alignment_method!(1);
}
impl_type!(std::num::NonZeroU8);

// No i8 type in D-Bus/GVariant, let's pretend it's i16
impl Basic for i8 {
    const SIGNATURE_CHAR: char = i16::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = i16::SIGNATURE_STR;

    alignment_method!(
        i16::alignment(Format::DBus),
        i16::alignment(Format::GVariant)
    );
}
impl_type!(i8);

impl Basic for std::num::NonZeroI8 {
    const SIGNATURE_CHAR: char = i8::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = i8::SIGNATURE_STR;

    alignment_method!(
        i16::alignment(Format::DBus),
        i16::alignment(Format::GVariant)
    );
}
impl_type!(std::num::NonZeroI8);

impl Basic for bool {
    const SIGNATURE_CHAR: char = 'b';
    const SIGNATURE_STR: &'static str = "b";

    alignment_method!(4);
}
impl_type!(bool);

impl Basic for i16 {
    const SIGNATURE_CHAR: char = 'n';
    const SIGNATURE_STR: &'static str = "n";

    alignment_method!(2);
}
impl_type!(i16);

impl Basic for std::num::NonZeroI16 {
    const SIGNATURE_CHAR: char = i16::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = i16::SIGNATURE_STR;

    alignment_method!(2);
}
impl_type!(std::num::NonZeroI16);

impl Basic for u16 {
    const SIGNATURE_CHAR: char = 'q';
    const SIGNATURE_STR: &'static str = "q";

    alignment_method!(2);
}
impl_type!(u16);

impl Basic for std::num::NonZeroU16 {
    const SIGNATURE_CHAR: char = u16::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = u16::SIGNATURE_STR;

    alignment_method!(2);
}
impl_type!(std::num::NonZeroU16);

impl Basic for i32 {
    const SIGNATURE_CHAR: char = 'i';
    const SIGNATURE_STR: &'static str = "i";

    alignment_method!(4);
}
impl_type!(i32);

impl Basic for std::num::NonZeroI32 {
    const SIGNATURE_CHAR: char = i32::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = i32::SIGNATURE_STR;

    alignment_method!(4);
}
impl_type!(std::num::NonZeroI32);

impl Basic for u32 {
    const SIGNATURE_CHAR: char = 'u';
    const SIGNATURE_STR: &'static str = "u";

    alignment_method!(4);
}
impl_type!(u32);

impl Basic for std::num::NonZeroU32 {
    const SIGNATURE_CHAR: char = u32::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = u32::SIGNATURE_STR;

    alignment_method!(4);
}
impl_type!(std::num::NonZeroU32);

impl Basic for i64 {
    const SIGNATURE_CHAR: char = 'x';
    const SIGNATURE_STR: &'static str = "x";

    alignment_method!(8);
}
impl_type!(i64);

impl Basic for std::num::NonZeroI64 {
    const SIGNATURE_CHAR: char = i64::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = i64::SIGNATURE_STR;

    alignment_method!(8);
}
impl_type!(std::num::NonZeroI64);

impl Basic for u64 {
    const SIGNATURE_CHAR: char = 't';
    const SIGNATURE_STR: &'static str = "t";

    alignment_method!(8);
}
impl_type!(u64);

impl Basic for std::num::NonZeroU64 {
    const SIGNATURE_CHAR: char = u64::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = u64::SIGNATURE_STR;

    alignment_method!(8);
}
impl_type!(std::num::NonZeroU64);

// No f32 type in D-Bus/GVariant, let's pretend it's f64
impl Basic for f32 {
    const SIGNATURE_CHAR: char = f64::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = f64::SIGNATURE_STR;

    alignment_method!(
        f64::alignment(Format::DBus),
        f64::alignment(Format::GVariant)
    );
}
impl_type!(f32);

impl Basic for f64 {
    const SIGNATURE_CHAR: char = 'd';
    const SIGNATURE_STR: &'static str = "d";

    alignment_method!(8);
}
impl_type!(f64);

impl Basic for str {
    const SIGNATURE_CHAR: char = 's';
    const SIGNATURE_STR: &'static str = "s";

    alignment_method!(4, 1);
}
impl_type!(str);

impl Basic for String {
    const SIGNATURE_CHAR: char = 's';
    const SIGNATURE_STR: &'static str = "s";

    alignment_method!(4, 1);
}
impl_type!(String);

impl Basic for char {
    const SIGNATURE_CHAR: char = <&str>::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = <&str>::SIGNATURE_STR;

    alignment_method!(4, 1);
}
impl_type!(char);
