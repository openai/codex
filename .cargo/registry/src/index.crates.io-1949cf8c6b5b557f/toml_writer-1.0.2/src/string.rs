/// Describes how a TOML string (key or value) should be formatted.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "alloc")] {
/// # use toml_writer::ToTomlValue as _;
/// let string = "Hello
/// world!
/// ";
/// let string = toml_writer::TomlStringBuilder::new(string).as_default();
/// let string = string.to_toml_value();
/// assert_eq!(string, r#""""
/// Hello
/// world!
/// """"#);
/// # }
/// ```
#[derive(Copy, Clone, Debug)]
pub struct TomlStringBuilder<'s> {
    decoded: &'s str,
    metrics: ValueMetrics,
}

impl<'s> TomlStringBuilder<'s> {
    pub fn new(decoded: &'s str) -> Self {
        Self {
            decoded,
            metrics: ValueMetrics::calculate(decoded),
        }
    }

    pub fn as_default(&self) -> TomlString<'s> {
        self.as_basic_pretty()
            .or_else(|| self.as_literal())
            .or_else(|| self.as_ml_basic_pretty())
            .or_else(|| self.as_ml_literal())
            .unwrap_or_else(|| {
                if self.metrics.newline {
                    self.as_ml_basic()
                } else {
                    self.as_basic()
                }
            })
    }

    pub fn as_literal(&self) -> Option<TomlString<'s>> {
        if self.metrics.escape_codes
            || 0 < self.metrics.max_seq_single_quotes
            || self.metrics.newline
        {
            None
        } else {
            Some(TomlString {
                decoded: self.decoded,
                encoding: Encoding::LiteralString,
                newline: self.metrics.newline,
            })
        }
    }

    pub fn as_ml_literal(&self) -> Option<TomlString<'s>> {
        if self.metrics.escape_codes || 2 < self.metrics.max_seq_single_quotes {
            None
        } else {
            Some(TomlString {
                decoded: self.decoded,
                encoding: Encoding::MlLiteralString,
                newline: self.metrics.newline,
            })
        }
    }

    pub fn as_basic_pretty(&self) -> Option<TomlString<'s>> {
        if self.metrics.escape_codes
            || self.metrics.escape
            || 0 < self.metrics.max_seq_double_quotes
            || self.metrics.newline
        {
            None
        } else {
            Some(self.as_basic())
        }
    }

    pub fn as_ml_basic_pretty(&self) -> Option<TomlString<'s>> {
        if self.metrics.escape_codes
            || self.metrics.escape
            || 2 < self.metrics.max_seq_double_quotes
        {
            None
        } else {
            Some(self.as_ml_basic())
        }
    }

    pub fn as_basic(&self) -> TomlString<'s> {
        TomlString {
            decoded: self.decoded,
            encoding: Encoding::BasicString,
            newline: self.metrics.newline,
        }
    }

    pub fn as_ml_basic(&self) -> TomlString<'s> {
        TomlString {
            decoded: self.decoded,
            encoding: Encoding::MlBasicString,
            newline: self.metrics.newline,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TomlString<'s> {
    decoded: &'s str,
    encoding: Encoding,
    newline: bool,
}

impl crate::WriteTomlValue for TomlString<'_> {
    fn write_toml_value<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        write_toml_value(self.decoded, Some(self.encoding), self.newline, writer)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct TomlKeyBuilder<'s> {
    decoded: &'s str,
    metrics: KeyMetrics,
}

impl<'s> TomlKeyBuilder<'s> {
    pub fn new(decoded: &'s str) -> Self {
        Self {
            decoded,
            metrics: KeyMetrics::calculate(decoded),
        }
    }

    pub fn as_default(&self) -> TomlKey<'s> {
        self.as_unquoted()
            .or_else(|| self.as_basic_pretty())
            .or_else(|| self.as_literal())
            .unwrap_or_else(|| self.as_basic())
    }

    pub fn as_unquoted(&self) -> Option<TomlKey<'s>> {
        if self.metrics.unquoted {
            Some(TomlKey {
                decoded: self.decoded,
                encoding: None,
            })
        } else {
            None
        }
    }

    pub fn as_literal(&self) -> Option<TomlKey<'s>> {
        if self.metrics.escape_codes || self.metrics.single_quotes {
            None
        } else {
            Some(TomlKey {
                decoded: self.decoded,
                encoding: Some(Encoding::LiteralString),
            })
        }
    }

    pub fn as_basic_pretty(&self) -> Option<TomlKey<'s>> {
        if self.metrics.escape_codes || self.metrics.escape || self.metrics.double_quotes {
            None
        } else {
            Some(self.as_basic())
        }
    }

    pub fn as_basic(&self) -> TomlKey<'s> {
        TomlKey {
            decoded: self.decoded,
            encoding: Some(Encoding::BasicString),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TomlKey<'s> {
    decoded: &'s str,
    encoding: Option<Encoding>,
}

impl crate::WriteTomlKey for TomlKey<'_> {
    fn write_toml_key<W: crate::TomlWrite + ?Sized>(&self, writer: &mut W) -> core::fmt::Result {
        let newline = false;
        write_toml_value(self.decoded, self.encoding, newline, writer)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
#[allow(clippy::enum_variant_names)]
enum Encoding {
    LiteralString,
    BasicString,
    MlLiteralString,
    MlBasicString,
}

impl Encoding {}

fn write_toml_value<W: crate::TomlWrite + ?Sized>(
    decoded: &str,
    encoding: Option<Encoding>,
    newline: bool,
    writer: &mut W,
) -> core::fmt::Result {
    let delimiter = match encoding {
        Some(Encoding::LiteralString) => "'",
        Some(Encoding::BasicString) => "\"",
        Some(Encoding::MlLiteralString) => "'''",
        Some(Encoding::MlBasicString) => "\"\"\"",
        None => "",
    };
    let escaped = match encoding {
        Some(Encoding::LiteralString) | Some(Encoding::MlLiteralString) => false,
        Some(Encoding::BasicString) | Some(Encoding::MlBasicString) => true,
        None => false,
    };
    let is_ml = match encoding {
        Some(Encoding::LiteralString) | Some(Encoding::BasicString) => false,
        Some(Encoding::MlLiteralString) | Some(Encoding::MlBasicString) => true,
        None => false,
    };
    let newline_prefix = newline && is_ml;

    write!(writer, "{delimiter}")?;
    if newline_prefix {
        writer.newline()?;
    }
    if escaped {
        // ```bnf
        // basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
        // wschar =  %x20  ; Space
        // wschar =/ %x09  ; Horizontal tab
        // escape = %x5C                   ; \
        // ```
        let max_seq_double_quotes = if is_ml { 2 } else { 0 };
        let mut stream = decoded;
        while !stream.is_empty() {
            let mut unescaped_end = 0;
            let mut escaped = None;
            let mut seq_double_quotes = 0;
            for (i, b) in stream.as_bytes().iter().enumerate() {
                if *b == b'"' {
                    seq_double_quotes += 1;
                    if max_seq_double_quotes < seq_double_quotes {
                        escaped = Some(r#"\""#);
                        break;
                    }
                } else {
                    seq_double_quotes = 0;
                }

                match *b {
                    0x8 => {
                        escaped = Some(r#"\b"#);
                        break;
                    }
                    0x9 => {
                        escaped = Some(r#"\t"#);
                        break;
                    }
                    0xa => {
                        if !is_ml {
                            escaped = Some(r#"\n"#);
                            break;
                        }
                    }
                    0xc => {
                        escaped = Some(r#"\f"#);
                        break;
                    }
                    0xd => {
                        escaped = Some(r#"\r"#);
                        break;
                    }
                    0x22 => {} // double quote handled earlier
                    0x5c => {
                        escaped = Some(r#"\\"#);
                        break;
                    }
                    c if c <= 0x1f || c == 0x7f => {
                        break;
                    }
                    _ => {}
                }

                unescaped_end = i + 1;
            }
            let unescaped = &stream[0..unescaped_end];
            let escaped_str = escaped.unwrap_or("");
            let end = unescaped_end + if escaped.is_some() { 1 } else { 0 };
            stream = &stream[end..];
            write!(writer, "{unescaped}{escaped_str}")?;
            if escaped.is_none() && !stream.is_empty() {
                let b = stream.as_bytes().first().unwrap();
                write!(writer, "\\u{:04X}", *b as u32)?;
                stream = &stream[1..];
            }
        }
    } else {
        write!(writer, "{decoded}")?;
    }
    write!(writer, "{delimiter}")?;
    Ok(())
}

#[derive(Copy, Clone, Debug)]
struct ValueMetrics {
    max_seq_single_quotes: u8,
    max_seq_double_quotes: u8,
    escape_codes: bool,
    escape: bool,
    newline: bool,
}

impl ValueMetrics {
    fn new() -> Self {
        Self {
            max_seq_single_quotes: 0,
            max_seq_double_quotes: 0,
            escape_codes: false,
            escape: false,
            newline: false,
        }
    }

    fn calculate(s: &str) -> Self {
        let mut metrics = Self::new();

        let mut prev_single_quotes = 0;
        let mut prev_double_quotes = 0;
        for byte in s.as_bytes() {
            if *byte == b'\'' {
                prev_single_quotes += 1;
                metrics.max_seq_single_quotes =
                    metrics.max_seq_single_quotes.max(prev_single_quotes);
            } else {
                prev_single_quotes = 0;
            }
            if *byte == b'"' {
                prev_double_quotes += 1;
                metrics.max_seq_double_quotes =
                    metrics.max_seq_double_quotes.max(prev_double_quotes);
            } else {
                prev_double_quotes = 0;
            }

            // ```bnf
            // literal-char = %x09 / %x20-26 / %x28-7E / non-ascii
            //
            // basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
            // wschar =  %x20  ; Space
            // wschar =/ %x09  ; Horizontal tab
            // escape = %x5C                   ; \
            // ```
            match *byte {
                b'\\' => metrics.escape = true,
                // Escape codes are needed if any ascii control
                // characters are present, including \b \f \r.
                b'\t' => {} // always allowed; remaining neutral on this
                b'\n' => metrics.newline = true,
                c if c <= 0x1f || c == 0x7f => metrics.escape_codes = true,
                _ => {}
            }
        }

        metrics
    }
}

#[derive(Copy, Clone, Debug)]
struct KeyMetrics {
    unquoted: bool,
    single_quotes: bool,
    double_quotes: bool,
    escape_codes: bool,
    escape: bool,
}

impl KeyMetrics {
    fn new() -> Self {
        Self {
            unquoted: true,
            single_quotes: false,
            double_quotes: false,
            escape_codes: false,
            escape: false,
        }
    }

    fn calculate(s: &str) -> Self {
        let mut metrics = Self::new();

        metrics.unquoted = !s.is_empty();

        for byte in s.as_bytes() {
            if !matches!(*byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_') {
                metrics.unquoted = false;
            }

            // ```bnf
            // unquoted-key = 1*( ALPHA / DIGIT / %x2D / %x5F ) ; A-Z / a-z / 0-9 / - / _
            //
            // literal-char = %x09 / %x20-26 / %x28-7E / non-ascii
            //
            // basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
            // wschar =  %x20  ; Space
            // wschar =/ %x09  ; Horizontal tab
            // escape = %x5C                   ; \
            // ```
            match *byte {
                b'\'' => metrics.single_quotes = true,
                b'"' => metrics.double_quotes = true,
                b'\\' => metrics.escape = true,
                // Escape codes are needed if any ascii control
                // characters are present, including \b \f \r.
                b'\t' => {} // always allowed
                c if c <= 0x1f || c == 0x7f => metrics.escape_codes = true,
                _ => {}
            }
        }

        metrics
    }
}
