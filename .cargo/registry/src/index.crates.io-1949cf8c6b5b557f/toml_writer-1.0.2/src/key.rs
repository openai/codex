#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
#[cfg(feature = "alloc")]
use alloc::string::String;

use crate::TomlWrite;

#[cfg(feature = "alloc")]
pub trait ToTomlKey {
    fn to_toml_key(&self) -> String;
}

#[cfg(feature = "alloc")]
impl<T> ToTomlKey for T
where
    T: WriteTomlKey + ?Sized,
{
    fn to_toml_key(&self) -> String {
        let mut result = String::new();
        let _ = self.write_toml_key(&mut result);
        result
    }
}

pub trait WriteTomlKey {
    fn write_toml_key<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result;
}

impl WriteTomlKey for str {
    fn write_toml_key<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        crate::TomlKeyBuilder::new(self)
            .as_default()
            .write_toml_key(writer)
    }
}

#[cfg(feature = "alloc")]
impl WriteTomlKey for String {
    fn write_toml_key<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        self.as_str().write_toml_key(writer)
    }
}

#[cfg(feature = "alloc")]
impl WriteTomlKey for Cow<'_, str> {
    fn write_toml_key<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        self.as_ref().write_toml_key(writer)
    }
}

impl<V: WriteTomlKey + ?Sized> WriteTomlKey for &V {
    fn write_toml_key<W: TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        (*self).write_toml_key(writer)
    }
}
