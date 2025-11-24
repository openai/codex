use core::fmt::{self, Display};

/// Describes how a TOML integer should be formatted.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "alloc")] {
/// # use toml_writer::ToTomlValue as _;
/// let format = toml_writer::TomlIntegerFormat::new().as_hex_lower();
/// let number = 10;
/// let number = format.format(number).unwrap_or(toml_writer::TomlInteger::new(number));
/// let number = number.to_toml_value();
/// assert_eq!(number, "0xa");
/// # }
/// ```
#[derive(Copy, Clone, Debug)]
pub struct TomlIntegerFormat {
    radix: Radix,
}

impl TomlIntegerFormat {
    /// Creates a new integer format (decimal).
    pub fn new() -> Self {
        Self {
            radix: Radix::Decimal,
        }
    }

    /// Sets the format to decimal.
    pub fn as_decimal(mut self) -> Self {
        self.radix = Radix::Decimal;
        self
    }

    /// Sets the format to hexadecimal with all characters in uppercase.
    pub fn as_hex_upper(mut self) -> Self {
        self.radix = Radix::Hexadecimal {
            case: HexCase::Upper,
        };
        self
    }

    /// Sets the format to hexadecimal with all characters in lowercase.
    pub fn as_hex_lower(mut self) -> Self {
        self.radix = Radix::Hexadecimal {
            case: HexCase::Lower,
        };
        self
    }

    /// Sets the format to octal.
    pub fn as_octal(mut self) -> Self {
        self.radix = Radix::Octal;
        self
    }

    /// Sets the format to binary.
    pub fn as_binary(mut self) -> Self {
        self.radix = Radix::Binary;
        self
    }

    /// Formats `value` as a TOML integer.
    ///
    /// Returns `None` if the value cannot be formatted
    /// (e.g. value is negative and the radix is not decimal).
    pub fn format<N: PartialOrd<i32>>(self, value: N) -> Option<TomlInteger<N>>
    where
        TomlInteger<N>: crate::WriteTomlValue,
    {
        match self.radix {
            Radix::Decimal => (),
            Radix::Hexadecimal { .. } | Radix::Octal | Radix::Binary => {
                if value < 0 {
                    return None;
                }
            }
        }

        Some(TomlInteger {
            value,
            format: self,
        })
    }
}

impl Default for TomlIntegerFormat {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper struct for formatting TOML integers.
///
/// This may be constructed by calling [`TomlIntegerFormat::format()`].
#[derive(Copy, Clone, Debug)]
pub struct TomlInteger<N> {
    value: N,
    format: TomlIntegerFormat,
}

impl<N> TomlInteger<N>
where
    Self: crate::WriteTomlValue,
{
    /// Apply default formatting
    pub fn new(value: N) -> Self {
        Self {
            value,
            format: TomlIntegerFormat::new(),
        }
    }
}

impl crate::WriteTomlValue for TomlInteger<u8> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<i8> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<u16> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<i16> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<u32> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<i32> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<u64> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<i64> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<u128> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<i128> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<usize> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

impl crate::WriteTomlValue for TomlInteger<isize> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_toml_value(self.value, &self.format, writer)
    }
}

#[derive(Copy, Clone, Debug)]
enum Radix {
    Decimal,
    Hexadecimal { case: HexCase },
    Octal,
    Binary,
}

#[derive(Copy, Clone, Debug)]
enum HexCase {
    Upper,
    Lower,
}

fn write_toml_value<
    N: Display + fmt::UpperHex + fmt::LowerHex + fmt::Octal + fmt::Binary,
    W: crate::TomlWrite + ?Sized,
>(
    value: N,
    format: &TomlIntegerFormat,
    writer: &mut W,
) -> fmt::Result {
    match format.radix {
        Radix::Decimal => write!(writer, "{value}")?,
        Radix::Hexadecimal { case } => match case {
            HexCase::Upper => write!(writer, "0x{value:X}")?,
            HexCase::Lower => write!(writer, "0x{value:x}")?,
        },
        Radix::Octal => write!(writer, "0o{value:o}")?,
        Radix::Binary => write!(writer, "0b{value:b}")?,
    }
    Ok(())
}
