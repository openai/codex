//! Precise signed credit amounts and dates for analytics presentation.

/// Keep tiny refunds visible while avoiding noise on ordinary credit amounts.
pub(super) fn amount(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.abs() < 0.000001 {
        return format!("{value:.2e}");
    }
    let rounded = if value.abs() < 0.01 {
        format!("{value:.6}")
    } else {
        format!("{value:.2}")
    };
    let (whole, fraction) = rounded.split_once('.').unwrap_or((&rounded, ""));
    let mut grouped = String::new();
    for (index, ch) in whole.chars().enumerate() {
        if index > 0
            && ch.is_ascii_digit()
            && (whole.len() - index).is_multiple_of(/*rhs*/ 3)
            && !grouped.ends_with('-')
        {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let fraction = fraction.trim_end_matches('0');
    if !fraction.is_empty() {
        grouped.push('.');
        grouped.push_str(fraction);
    }
    grouped
}

/// Align ordinary credit amounts to two decimals without hiding tiny adjustments.
pub(super) fn credit_amount(value: f64) -> String {
    let formatted = amount(value);
    if value != 0.0 && value.abs() < 0.01 {
        return formatted;
    }
    match formatted.split_once('.') {
        None => format!("{formatted}.00"),
        Some((_, fraction)) if fraction.len() == 1 => format!("{formatted}0"),
        Some(_) => formatted,
    }
}

/// Format integer millionths without floating-point rounding or hiding tiny adjustments.
pub(super) fn credits(micros: i64) -> String {
    let magnitude = micros.unsigned_abs();
    if magnitude > 0 && magnitude < 10_000 {
        let fractional = format!("{magnitude:06}");
        return format!(
            "{}0.{}",
            if micros < 0 { "-" } else { "" },
            fractional.trim_end_matches('0')
        );
    }
    let cents = (magnitude + 5_000) / 10_000;
    let mut whole = (cents / 100).to_string();
    let digits = whole.len();
    for index in (1..digits).rev() {
        if (digits - index).is_multiple_of(/*rhs*/ 3) {
            whole.insert(index, ',');
        }
    }
    format!(
        "{}{whole}.{:02}",
        if micros < 0 { "-" } else { "" },
        cents % 100
    )
}

pub(super) fn date(date: &str) -> String {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|date| date.format("%b %-d").to_string())
        .unwrap_or_else(|_| date.to_string())
}
