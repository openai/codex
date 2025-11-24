#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::TomlWrite;
use crate::WriteTomlKey;

#[cfg(feature = "alloc")]
pub trait ToTomlValue {
    fn to_toml_value(&self) -> String;
}

#[cfg(feature = "alloc")]
impl<T> ToTomlValue for T
where
    T: WriteTomlValue + ?Sized,
{
    fn to_toml_value(&self) -> String {
        let mut result = String::new();
        let _ = self.write_toml_value(&mut result);
        result
    }
}

pub trait WriteTomlValue {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result;
}

impl WriteTomlValue for bool {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for u8 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for i8 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for u16 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for i16 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for u32 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for i32 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for u64 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for i64 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for u128 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for i128 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write!(writer, "{self}")
    }
}

impl WriteTomlValue for f32 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        match (self.is_sign_negative(), self.is_nan(), *self == 0.0) {
            (true, true, _) => write!(writer, "-nan"),
            (false, true, _) => write!(writer, "nan"),
            (true, false, true) => write!(writer, "-0.0"),
            (false, false, true) => write!(writer, "0.0"),
            (_, false, false) => {
                if self % 1.0 == 0.0 {
                    write!(writer, "{self}.0")
                } else {
                    write!(writer, "{self}")
                }
            }
        }
    }
}

impl WriteTomlValue for f64 {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        match (self.is_sign_negative(), self.is_nan(), *self == 0.0) {
            (true, true, _) => write!(writer, "-nan"),
            (false, true, _) => write!(writer, "nan"),
            (true, false, true) => write!(writer, "-0.0"),
            (false, false, true) => write!(writer, "0.0"),
            (_, false, false) => {
                if self % 1.0 == 0.0 {
                    write!(writer, "{self}.0")
                } else {
                    write!(writer, "{self}")
                }
            }
        }
    }
}

impl WriteTomlValue for char {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        let mut buf = [0; 4];
        let v = self.encode_utf8(&mut buf);
        v.write_toml_value(writer)
    }
}

impl WriteTomlValue for str {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        crate::TomlStringBuilder::new(self)
            .as_default()
            .write_toml_value(writer)
    }
}

#[cfg(feature = "alloc")]
impl WriteTomlValue for String {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        self.as_str().write_toml_value(writer)
    }
}

#[cfg(feature = "alloc")]
impl WriteTomlValue for Cow<'_, str> {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        self.as_ref().write_toml_value(writer)
    }
}

impl<V: WriteTomlValue> WriteTomlValue for [V] {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        writer.open_array()?;
        let mut iter = self.iter();
        if let Some(v) = iter.next() {
            writer.value(v)?;
        }
        for v in iter {
            writer.val_sep()?;
            writer.space()?;
            writer.value(v)?;
        }
        writer.close_array()?;
        Ok(())
    }
}

impl<V: WriteTomlValue, const N: usize> WriteTomlValue for [V; N] {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        self.as_slice().write_toml_value(writer)
    }
}

#[cfg(feature = "alloc")]
impl<V: WriteTomlValue> WriteTomlValue for Vec<V> {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        self.as_slice().write_toml_value(writer)
    }
}

#[cfg(feature = "alloc")]
impl<K: WriteTomlKey, V: WriteTomlValue> WriteTomlValue for alloc::collections::BTreeMap<K, V> {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write_toml_inline_table(self.iter(), writer)
    }
}

#[cfg(feature = "std")]
impl<K: WriteTomlKey, V: WriteTomlValue> WriteTomlValue for std::collections::HashMap<K, V> {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write_toml_inline_table(self.iter(), writer)
    }
}

fn write_toml_inline_table<
    'i,
    I: Iterator<Item = (&'i K, &'i V)>,
    K: WriteTomlKey + 'i,
    V: WriteTomlValue + 'i,
    W: TomlWrite + ?Sized,
>(
    mut iter: I,
    writer: &mut W,
) -> core::fmt::Result {
    writer.open_inline_table()?;
    let mut trailing_space = false;
    if let Some((key, value)) = iter.next() {
        writer.space()?;
        writer.key(key)?;
        writer.space()?;
        writer.keyval_sep()?;
        writer.space()?;
        writer.value(value)?;
        trailing_space = true;
    }
    for (key, value) in iter {
        writer.val_sep()?;
        writer.space()?;
        writer.key(key)?;
        writer.space()?;
        writer.keyval_sep()?;
        writer.space()?;
        writer.value(value)?;
    }
    if trailing_space {
        writer.space()?;
    }
    writer.close_inline_table()?;
    Ok(())
}

impl<V: WriteTomlValue + ?Sized> WriteTomlValue for &V {
    fn write_toml_value<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        (*self).write_toml_value(writer)
    }
}
