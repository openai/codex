pub trait TomlWrite: core::fmt::Write {
    fn open_table_header(&mut self) -> core::fmt::Result {
        write!(self, "[")
    }
    fn close_table_header(&mut self) -> core::fmt::Result {
        write!(self, "]")
    }

    fn open_array_of_tables_header(&mut self) -> core::fmt::Result {
        write!(self, "[[")
    }
    fn close_array_of_tables_header(&mut self) -> core::fmt::Result {
        write!(self, "]]")
    }

    fn open_inline_table(&mut self) -> core::fmt::Result {
        write!(self, "{{")
    }
    fn close_inline_table(&mut self) -> core::fmt::Result {
        write!(self, "}}")
    }

    fn open_array(&mut self) -> core::fmt::Result {
        write!(self, "[")
    }
    fn close_array(&mut self) -> core::fmt::Result {
        write!(self, "]")
    }

    fn key_sep(&mut self) -> core::fmt::Result {
        write!(self, ".")
    }

    fn keyval_sep(&mut self) -> core::fmt::Result {
        write!(self, "=")
    }

    /// Write an encoded TOML key
    ///
    /// To customize the encoding, see [`TomlStringBuilder`][crate::TomlStringBuilder].
    fn key(&mut self, value: impl crate::WriteTomlKey) -> core::fmt::Result {
        value.write_toml_key(self)
    }

    /// Write an encoded TOML scalar value
    ///
    /// To customize the encoding, see
    /// - [`TomlStringBuilder`][crate::TomlStringBuilder]
    /// - [`TomlIntegerFormat`][crate::TomlIntegerFormat]
    ///
    /// <div class="warning">
    ///
    /// For floats, this preserves the sign bit for [`f32::NAN`] / [`f64::NAN`] for the sake of
    /// format-preserving editing.
    /// However, in most cases the sign bit is indeterminate and outputting signed NANs can be a
    /// cause of non-repeatable behavior.
    ///
    /// For general serialization, you should discard the sign bit.  For example:
    /// ```
    /// # let mut v = f64::NAN;
    /// if v.is_nan() {
    ///     v = v.copysign(1.0);
    /// }
    /// ```
    ///
    /// </div>
    fn value(&mut self, value: impl crate::WriteTomlValue) -> core::fmt::Result {
        value.write_toml_value(self)
    }

    fn val_sep(&mut self) -> core::fmt::Result {
        write!(self, ",")
    }

    fn space(&mut self) -> core::fmt::Result {
        write!(self, " ")
    }

    fn open_comment(&mut self) -> core::fmt::Result {
        write!(self, "#")
    }

    fn newline(&mut self) -> core::fmt::Result {
        writeln!(self)
    }
}

impl<W> TomlWrite for W where W: core::fmt::Write {}
