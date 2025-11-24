// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Lower-level types for decimal formatting.

use core::fmt::Write;

use crate::grouper;
use crate::options::*;
use crate::parts;
use crate::provider::*;
use fixed_decimal::Decimal;
use fixed_decimal::Sign;
use writeable::Part;
use writeable::PartsWrite;
use writeable::Writeable;

/// An intermediate structure returned by [`DecimalFormatter`](crate::DecimalFormatter).
/// Use [`Writeable`][Writeable] to render the formatted decimal to a string or buffer.
#[derive(Debug, PartialEq, Clone)]
pub struct FormattedDecimal<'l> {
    pub(crate) value: &'l Decimal,
    pub(crate) options: &'l DecimalFormatterOptions,
    pub(crate) symbols: &'l DecimalSymbols<'l>,
    pub(crate) digits: &'l [char; 10],
}

impl FormattedDecimal<'_> {
    /// Returns the affixes needed for the current sign, as (prefix, suffix)
    fn get_affixes(&self) -> Option<(Part, (&str, &str))> {
        match self.value.sign() {
            Sign::None => None,
            Sign::Negative => Some((parts::MINUS_SIGN, self.symbols.minus_sign_affixes())),
            Sign::Positive => Some((parts::PLUS_SIGN, self.symbols.plus_sign_affixes())),
        }
    }
}

impl Writeable for FormattedDecimal<'_> {
    fn write_to_parts<W>(&self, w: &mut W) -> core::result::Result<(), core::fmt::Error>
    where
        W: writeable::PartsWrite + ?Sized,
    {
        let affixes = self.get_affixes();
        if let Some((part, affixes)) = affixes {
            w.with_part(part, |w| w.write_str(affixes.0))?;
        }
        let range = self.value.absolute.magnitude_range();
        let upper_magnitude = *range.end();
        let mut range = range.rev();
        let mut has_fraction = false;
        w.with_part(parts::INTEGER, |w| {
            loop {
                let m = match range.next() {
                    Some(m) if m < 0 => {
                        has_fraction = true;
                        break Ok(());
                    }
                    Some(m) => m,
                    None => {
                        break Ok(());
                    }
                };
                #[expect(clippy::indexing_slicing)] // digit_at in 0..=9
                w.write_char(self.digits[self.value.digit_at(m) as usize])?;
                if grouper::check(
                    upper_magnitude,
                    m,
                    self.options.grouping_strategy.unwrap_or_default(),
                    self.symbols.grouping_sizes,
                ) {
                    w.with_part(parts::GROUP, |w| {
                        w.write_str(self.symbols.grouping_separator())
                    })?;
                }
            }
        })?;
        if has_fraction {
            w.with_part(parts::DECIMAL, |w| {
                w.write_str(self.symbols.decimal_separator())
            })?;
            w.with_part(parts::FRACTION, |w| {
                let mut m = -1; // read in the previous loop
                loop {
                    #[expect(clippy::indexing_slicing)] // digit_at in 0..=9
                    w.write_char(self.digits[self.value.digit_at(m) as usize])?;
                    m = match range.next() {
                        Some(m) => m,
                        None => {
                            break Ok(());
                        }
                    };
                }
            })?;
        }
        if let Some((part, affixes)) = affixes {
            w.with_part(part, |w| w.write_str(affixes.1))?;
        }
        Ok(())
    }
}

writeable::impl_display_with_writeable!(FormattedDecimal<'_>);

#[cfg(test)]
mod tests {
    use icu_locale_core::locale;
    use writeable::assert_writeable_eq;

    use crate::DecimalFormatter;

    #[test]
    pub fn test_es_mx() {
        let locale = locale!("es-MX").into();
        let fmt = DecimalFormatter::try_new(locale, Default::default()).unwrap();
        let fd = "12345.67".parse().unwrap();
        assert_writeable_eq!(fmt.format(&fd), "12,345.67");
    }
}
