use std::{borrow::Cow, io::Write, slice::from_ref};

use crate::{bytecast, tags::Type, TiffError, TiffFormatError, TiffResult};

use super::writer::TiffWriter;

/// Trait for types that can be encoded in a tiff file
pub trait TiffValue {
    const BYTE_LEN: u8;
    const FIELD_TYPE: Type;
    fn count(&self) -> usize;
    fn bytes(&self) -> usize {
        self.count() * usize::from(Self::BYTE_LEN)
    }

    /// Access this value as an contiguous sequence of bytes.
    /// If their is no trivial representation, allocate it on the heap.
    fn data(&self) -> Cow<'_, [u8]>;

    /// Write this value to a TiffWriter.
    /// While the default implementation will work in all cases, it may require unnecessary allocations.
    /// The written bytes of any custom implementation MUST be the same as yielded by `self.data()`.
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_bytes(&self.data())?;
        Ok(())
    }
}

impl TiffValue for [u8] {
    const BYTE_LEN: u8 = 1;
    const FIELD_TYPE: Type = Type::BYTE;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self)
    }
}

impl TiffValue for [i8] {
    const BYTE_LEN: u8 = 1;
    const FIELD_TYPE: Type = Type::SBYTE;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::i8_as_ne_bytes(self))
    }
}

impl TiffValue for [u16] {
    const BYTE_LEN: u8 = 2;
    const FIELD_TYPE: Type = Type::SHORT;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::u16_as_ne_bytes(self))
    }
}

impl TiffValue for [i16] {
    const BYTE_LEN: u8 = 2;
    const FIELD_TYPE: Type = Type::SSHORT;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::i16_as_ne_bytes(self))
    }
}

impl TiffValue for [u32] {
    const BYTE_LEN: u8 = 4;
    const FIELD_TYPE: Type = Type::LONG;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::u32_as_ne_bytes(self))
    }
}

impl TiffValue for [i32] {
    const BYTE_LEN: u8 = 4;
    const FIELD_TYPE: Type = Type::SLONG;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::i32_as_ne_bytes(self))
    }
}

impl TiffValue for [u64] {
    const BYTE_LEN: u8 = 8;
    const FIELD_TYPE: Type = Type::LONG8;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::u64_as_ne_bytes(self))
    }
}

impl TiffValue for [i64] {
    const BYTE_LEN: u8 = 8;
    const FIELD_TYPE: Type = Type::SLONG8;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::i64_as_ne_bytes(self))
    }
}

impl TiffValue for [f32] {
    const BYTE_LEN: u8 = 4;
    const FIELD_TYPE: Type = Type::FLOAT;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        // We write using native endian so this should be safe
        Cow::Borrowed(bytecast::f32_as_ne_bytes(self))
    }
}

impl TiffValue for [f64] {
    const BYTE_LEN: u8 = 8;
    const FIELD_TYPE: Type = Type::DOUBLE;

    fn count(&self) -> usize {
        self.len()
    }

    fn data(&self) -> Cow<'_, [u8]> {
        // We write using native endian so this should be safe
        Cow::Borrowed(bytecast::f64_as_ne_bytes(self))
    }
}

impl TiffValue for u8 {
    const BYTE_LEN: u8 = 1;
    const FIELD_TYPE: Type = Type::BYTE;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u8(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(from_ref(self))
    }
}

impl TiffValue for i8 {
    const BYTE_LEN: u8 = 1;
    const FIELD_TYPE: Type = Type::SBYTE;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_i8(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::i8_as_ne_bytes(from_ref(self)))
    }
}

impl TiffValue for u16 {
    const BYTE_LEN: u8 = 2;
    const FIELD_TYPE: Type = Type::SHORT;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u16(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::u16_as_ne_bytes(from_ref(self)))
    }
}

impl TiffValue for i16 {
    const BYTE_LEN: u8 = 2;
    const FIELD_TYPE: Type = Type::SSHORT;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_i16(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::i16_as_ne_bytes(from_ref(self)))
    }
}

impl TiffValue for u32 {
    const BYTE_LEN: u8 = 4;
    const FIELD_TYPE: Type = Type::LONG;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u32(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::u32_as_ne_bytes(from_ref(self)))
    }
}

impl TiffValue for i32 {
    const BYTE_LEN: u8 = 4;
    const FIELD_TYPE: Type = Type::SLONG;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_i32(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::i32_as_ne_bytes(from_ref(self)))
    }
}

impl TiffValue for u64 {
    const BYTE_LEN: u8 = 8;
    const FIELD_TYPE: Type = Type::LONG8;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u64(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::u64_as_ne_bytes(from_ref(self)))
    }
}

impl TiffValue for i64 {
    const BYTE_LEN: u8 = 8;
    const FIELD_TYPE: Type = Type::SLONG8;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_i64(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::i64_as_ne_bytes(from_ref(self)))
    }
}

impl TiffValue for f32 {
    const BYTE_LEN: u8 = 4;
    const FIELD_TYPE: Type = Type::FLOAT;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_f32(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::f32_as_ne_bytes(from_ref(self)))
    }
}

impl TiffValue for f64 {
    const BYTE_LEN: u8 = 8;
    const FIELD_TYPE: Type = Type::DOUBLE;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_f64(*self)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::f64_as_ne_bytes(from_ref(self)))
    }
}

impl TiffValue for Ifd {
    const BYTE_LEN: u8 = 4;
    const FIELD_TYPE: Type = Type::IFD;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u32(self.0)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::u32_as_ne_bytes(from_ref(&self.0)))
    }
}

impl TiffValue for Ifd8 {
    const BYTE_LEN: u8 = 8;
    const FIELD_TYPE: Type = Type::IFD8;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u64(self.0)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(bytecast::u64_as_ne_bytes(from_ref(&self.0)))
    }
}

impl TiffValue for Rational {
    const BYTE_LEN: u8 = 8;
    const FIELD_TYPE: Type = Type::RATIONAL;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u32(self.n)?;
        writer.write_u32(self.d)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Owned({
            let first_dword = bytecast::u32_as_ne_bytes(from_ref(&self.n));
            let second_dword = bytecast::u32_as_ne_bytes(from_ref(&self.d));
            [first_dword, second_dword].concat()
        })
    }
}

impl TiffValue for SRational {
    const BYTE_LEN: u8 = 8;
    const FIELD_TYPE: Type = Type::SRATIONAL;

    fn count(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_i32(self.n)?;
        writer.write_i32(self.d)?;
        Ok(())
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Owned({
            let first_dword = bytecast::i32_as_ne_bytes(from_ref(&self.n));
            let second_dword = bytecast::i32_as_ne_bytes(from_ref(&self.d));
            [first_dword, second_dword].concat()
        })
    }
}

impl TiffValue for str {
    const BYTE_LEN: u8 = 1;
    const FIELD_TYPE: Type = Type::ASCII;

    fn count(&self) -> usize {
        self.len() + 1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        if self.is_ascii() && !self.bytes().any(|b| b == 0) {
            writer.write_bytes(self.as_bytes())?;
            writer.write_u8(0)?;
            Ok(())
        } else {
            Err(TiffError::FormatError(TiffFormatError::InvalidTag))
        }
    }

    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Owned({
            if self.is_ascii() && !self.bytes().any(|b| b == 0) {
                let bytes: &[u8] = self.as_bytes();
                [bytes, &[0]].concat()
            } else {
                vec![]
            }
        })
    }
}

impl<T: TiffValue + ?Sized> TiffValue for &'_ T {
    const BYTE_LEN: u8 = T::BYTE_LEN;
    const FIELD_TYPE: Type = T::FIELD_TYPE;

    fn count(&self) -> usize {
        (*self).count()
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        (*self).write(writer)
    }

    fn data(&self) -> Cow<'_, [u8]> {
        T::data(self)
    }
}

macro_rules! impl_tiff_value_for_contiguous_sequence {
    ($inner_type:ty; $bytes:expr; $field_type:expr) => {
        impl $crate::encoder::TiffValue for [$inner_type] {
            const BYTE_LEN: u8 = $bytes;
            const FIELD_TYPE: Type = $field_type;

            fn count(&self) -> usize {
                self.len()
            }

            fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
                for x in self {
                    x.write(writer)?;
                }
                Ok(())
            }

            fn data(&self) -> Cow<'_, [u8]> {
                let mut buf: Vec<u8> = Vec::with_capacity(Self::BYTE_LEN as usize * self.len());
                for x in self {
                    buf.extend_from_slice(&x.data());
                }
                Cow::Owned(buf)
            }
        }
    };
}

impl_tiff_value_for_contiguous_sequence!(Ifd; 4; Type::IFD);
impl_tiff_value_for_contiguous_sequence!(Ifd8; 8; Type::IFD8);
impl_tiff_value_for_contiguous_sequence!(Rational; 8; Type::RATIONAL);
impl_tiff_value_for_contiguous_sequence!(SRational; 8; Type::SRATIONAL);

/// Type to represent tiff values of type `IFD`
#[derive(Clone)]
pub struct Ifd(pub u32);

/// Type to represent tiff values of type `IFD8`
#[derive(Clone)]
pub struct Ifd8(pub u64);

/// Type to represent tiff values of type `RATIONAL`
#[derive(Clone)]
pub struct Rational {
    pub n: u32,
    pub d: u32,
}

/// Type to represent tiff values of type `SRATIONAL`
#[derive(Clone)]
pub struct SRational {
    pub n: i32,
    pub d: i32,
}
