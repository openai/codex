/// Format a u64 with thousands separators (commas), e.g. 1234567 -> "1,234,567".
pub fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);

    let mut chunks = s.as_bytes().rchunks(3).rev();
    if let Some(first) = chunks.next() {
        #[allow(clippy::unwrap_used)]
        out.push_str(std::str::from_utf8(first).unwrap());
        for chunk in chunks {
            out.push(',');
            #[allow(clippy::unwrap_used)]
            out.push_str(std::str::from_utf8(chunk).unwrap());
        }
    }

    out
}

/// Format token counts to 3 significant figures, using base-10 SI suffixes.
///
/// Examples:
///   - 999 -> "999"
///   - 1200 -> "1.20K"
///   - 123456789 -> "123M"
pub fn format_si_suffix(n: u64) -> String {
    if n < 1000 {
        return n.to_string();
    }

    const UNITS: &[(f64, &str)] = &[(1_000.0, "K"), (1_000_000.0, "M"), (1_000_000_000.0, "G")];

    let f = n as f64;
    for (scale, suffix) in UNITS {
        if (100.0 * f / *scale).round() < 1000.0 {
            return format!("{:.02}{}", f / *scale, suffix);
        } else if (10.0 * f / *scale).round() < 1000.0 {
            return format!("{:.01}{}", f / *scale, suffix);
        } else if (f / *scale).round() < 1000.0 {
            return format!("{:.00}{}", f / *scale, suffix);
        }
    }

    format!(
        "{}G",
        format_with_commas((n as f64 / 1_000_000_000.0).round() as u64)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commas() {
        assert_eq!(format_with_commas(0), "0");
        assert_eq!(format_with_commas(5), "5");
        assert_eq!(format_with_commas(999), "999");
        assert_eq!(format_with_commas(1_000), "1,000");
        assert_eq!(format_with_commas(12_345), "12,345");
        assert_eq!(format_with_commas(123_456), "123,456");
        assert_eq!(format_with_commas(1_234_567), "1,234,567");
    }

    #[test]
    fn kmg() {
        assert_eq!(format_si_suffix(0), "0");
        assert_eq!(format_si_suffix(999), "999");
        assert_eq!(format_si_suffix(1_000), "1.00K");
        assert_eq!(format_si_suffix(1_200), "1.20K");
        assert_eq!(format_si_suffix(10_000), "10.0K");
        assert_eq!(format_si_suffix(100_000), "100K");
        assert_eq!(format_si_suffix(999_500), "1.00M");
        assert_eq!(format_si_suffix(1_000_000), "1.00M");
        assert_eq!(format_si_suffix(1_234_000), "1.23M");
        assert_eq!(format_si_suffix(12_345_678), "12.3M");
        assert_eq!(format_si_suffix(999_950_000), "1.00G");
        assert_eq!(format_si_suffix(1_000_000_000), "1.00G");
        assert_eq!(format_si_suffix(1_234_000_000), "1.23G");
        // Above 1000G we keep whole‑G precision (no higher unit supported here).
        assert_eq!(format_si_suffix(1_234_000_000_000), "1,234G");
    }
}
