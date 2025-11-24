use std::io::{self, Write};

#[cfg(feature = "color")]
use super::WriteStyle;
use super::{Formatter, StyledValue};
#[cfg(feature = "color")]
use anstyle::Style;
use log::kv::{Error, Key, Source, Value, VisitSource};

/// Format function for serializing key/value pairs
///
/// This function determines how key/value pairs for structured logs are serialized within the default
/// format.
pub(crate) type KvFormatFn = dyn Fn(&mut Formatter, &dyn Source) -> io::Result<()> + Sync + Send;

/// Null Key Value Format
///
/// This function is intended to be passed to
/// [`Builder::format_key_values`](crate::Builder::format_key_values).
///
/// This key value format simply ignores any key/value fields and doesn't include them in the
/// output.
pub fn hidden_kv_format(_formatter: &mut Formatter, _fields: &dyn Source) -> io::Result<()> {
    Ok(())
}

/// Default Key Value Format
///
/// This function is intended to be passed to
/// [`Builder::format_key_values`](crate::Builder::format_key_values).
///
/// This is the default key/value format. Which uses an "=" as the separator between the key and
/// value and a " " between each pair.
///
/// For example: `ip=127.0.0.1 port=123456 path=/example`
pub fn default_kv_format(formatter: &mut Formatter, fields: &dyn Source) -> io::Result<()> {
    fields
        .visit(&mut DefaultVisitSource(formatter))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

struct DefaultVisitSource<'a>(&'a mut Formatter);

impl<'kvs> VisitSource<'kvs> for DefaultVisitSource<'_> {
    fn visit_pair(&mut self, key: Key<'_>, value: Value<'kvs>) -> Result<(), Error> {
        write!(self.0, " {}={}", self.style_key(key), value)?;
        Ok(())
    }
}

impl DefaultVisitSource<'_> {
    fn style_key<'k>(&self, text: Key<'k>) -> StyledValue<Key<'k>> {
        #[cfg(feature = "color")]
        {
            StyledValue {
                style: if self.0.write_style == WriteStyle::Never {
                    Style::new()
                } else {
                    Style::new().italic()
                },
                value: text,
            }
        }
        #[cfg(not(feature = "color"))]
        {
            text
        }
    }
}
