use std::path::Component;
use std::path::Path;

pub(crate) fn parse_git_boolean(value: &[u8]) -> Option<bool> {
    if value.eq_ignore_ascii_case(b"true")
        || value.eq_ignore_ascii_case(b"yes")
        || value.eq_ignore_ascii_case(b"on")
    {
        return Some(true);
    }
    if value.is_empty()
        || value.eq_ignore_ascii_case(b"false")
        || value.eq_ignore_ascii_case(b"no")
        || value.eq_ignore_ascii_case(b"off")
    {
        return Some(false);
    }

    // Git parses the remaining boolean spellings through `git_parse_int`: C
    // base-0 syntax, an optional binary-unit suffix, and signed `int` bounds.
    let value = std::str::from_utf8(value)
        .ok()?
        .trim_start_matches(|value: char| value.is_ascii_whitespace());
    let (negative, unsigned) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        Some(_) => (false, value),
        None => return None,
    };
    let (base, unsigned) = if unsigned.starts_with("0x") || unsigned.starts_with("0X") {
        (16, &unsigned[2..])
    } else if unsigned.starts_with('0') {
        (8, unsigned)
    } else {
        (10, unsigned)
    };
    let digit_count = unsigned
        .bytes()
        .take_while(|byte| match base {
            8 => matches!(byte, b'0'..=b'7'),
            10 => byte.is_ascii_digit(),
            16 => byte.is_ascii_hexdigit(),
            _ => false,
        })
        .count();
    if digit_count == 0 {
        return None;
    }
    let (digits, suffix) = unsigned.split_at(digit_count);
    let factor = if suffix.is_empty() {
        1_i128
    } else if suffix.eq_ignore_ascii_case("k") {
        1024
    } else if suffix.eq_ignore_ascii_case("m") {
        1024 * 1024
    } else if suffix.eq_ignore_ascii_case("g") {
        1024 * 1024 * 1024
    } else {
        return None;
    };
    let magnitude = i128::from_str_radix(digits, base).ok()?;
    let signed = if negative { -magnitude } else { magnitude };
    let value = signed.checked_mul(factor)?;
    (i128::from(i32::MIN)..=i128::from(i32::MAX))
        .contains(&value)
        .then_some(value != 0)
}

pub(crate) fn path_is_within(path: &Path, root: &Path) -> bool {
    let mut path_components = path.components();
    for root_component in root.components() {
        let Some(path_component) = path_components.next() else {
            return false;
        };
        if !components_equal(path_component, root_component) {
            return false;
        }
    }
    true
}

#[cfg(windows)]
fn components_equal(left: Component<'_>, right: Component<'_>) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

#[cfg(not(windows))]
fn components_equal(left: Component<'_>, right: Component<'_>) -> bool {
    left == right
}

#[cfg(test)]
#[path = "git_config_tests.rs"]
mod tests;
