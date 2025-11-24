//! Validating parsers.

use crate::parser::str::{
    find_split2_hole, find_split_hole, satisfy_chars_with_pct_encoded, starts_with_double_hexdigits,
};
use crate::template::components::MaybeOperator;
use crate::template::error::{Error, ErrorKind};

use crate::template::parser::char::{
    is_ascii_literal_char, is_ascii_varchar_continue, is_ascii_varchar_start,
};

/// Returns `Ok(())` if the given string is a valid literal.
fn validate_literal(s: &str, offset: usize) -> Result<(), Error> {
    match s
        .chars()
        .position(|c| !c.is_ascii() || !is_ascii_literal_char(c as u8))
    {
        Some(pos) => Err(Error::new(ErrorKind::InvalidCharacter, offset + pos)),
        None => Ok(()),
    }
}

/// Returns `Ok(())` if the given string is a valid varspec.
fn validate_varspec(s: &str, offset: usize) -> Result<(), Error> {
    match find_split2_hole(s, b':', b'*') {
        Some((maybe_varname, b':', maybe_len)) => {
            validate_varname(maybe_varname, offset)?;
            if !(1..=5).contains(&maybe_len.len()) {
                return Err(Error::new(
                    ErrorKind::InvalidExpression,
                    offset + maybe_varname.len() + 2,
                ));
            }
            if let Some(pos) = maybe_len.bytes().position(|b| !b.is_ascii_digit()) {
                return Err(Error::new(
                    ErrorKind::InvalidExpression,
                    offset + maybe_varname.len() + 2 + pos,
                ));
            }
        }
        Some((maybe_varname, b'*', extra)) => {
            validate_varname(maybe_varname, offset)?;
            if !extra.is_empty() {
                return Err(Error::new(
                    ErrorKind::InvalidExpression,
                    offset + maybe_varname.len() + 1,
                ));
            }
        }
        Some((_, sep, _)) => unreachable!("[consistency] the byte {sep:#02x} is not searched"),
        None => validate_varname(s, offset)?,
    }
    Ok(())
}

/// Returns `Ok(())` if the given string is a valid varname.
pub(crate) fn validate_varname(s: &str, offset: usize) -> Result<(), Error> {
    let rest = match s.as_bytes().first() {
        Some(b'%') if starts_with_double_hexdigits(&s.as_bytes()[1..]) => &s[3..],
        Some(b) if b.is_ascii() && is_ascii_varchar_start(*b) => &s[1..],
        _ => return Err(Error::new(ErrorKind::InvalidExpression, offset)),
    };
    let is_valid = satisfy_chars_with_pct_encoded(rest, is_ascii_varchar_continue, |_| false);
    if !is_valid {
        return Err(Error::new(ErrorKind::InvalidExpression, offset));
    }
    Ok(())
}

/// Returns `Ok(())` if the given string is a valid expression.
///
/// "Expression" here is the expression body inside `{` and `}`, but not including braces.
fn validate_expr_body(s: &str, mut offset: usize) -> Result<(), Error> {
    if s.is_empty() {
        return Err(Error::new(ErrorKind::InvalidExpression, offset));
    }

    // Skip the operator.
    let maybe_variable_list = match MaybeOperator::from_byte(s.as_bytes()[0]) {
        Some(MaybeOperator::Operator(_)) => {
            offset += 1;
            &s[1..]
        }
        Some(MaybeOperator::Reserved(_)) => {
            return Err(Error::new(ErrorKind::UnsupportedOperator, offset));
        }
        None => s,
    };

    // Validate varspecs.
    for (spec_i, maybe_varspec) in maybe_variable_list.split(',').enumerate() {
        if spec_i != 0 {
            // Add the length of the leading separator `,`.
            offset += 1;
        }
        validate_varspec(maybe_varspec, offset)?;
        offset += maybe_varspec.len();
    }

    Ok(())
}

/// Validates whether the given string is valid as a URI template.
///
/// Returns `Ok(())` if the given string is a valid URI template.
pub(in crate::template) fn validate_template_str(s: &str) -> Result<(), Error> {
    let mut rest = s;
    let mut offset = 0;
    while !rest.is_empty() {
        rest = match find_split2_hole(rest, b'%', b'{') {
            Some((literal, b'%', xdigits2_and_rest)) => {
                validate_literal(literal, offset)?;

                if xdigits2_and_rest.len() < 2 {
                    return Err(Error::new(
                        ErrorKind::InvalidPercentEncoding,
                        offset + literal.len(),
                    ));
                }
                let (xdigits2, new_rest) = xdigits2_and_rest.split_at(2);
                if !xdigits2.as_bytes()[0].is_ascii_hexdigit() {
                    return Err(Error::new(
                        ErrorKind::InvalidPercentEncoding,
                        offset + literal.len() + 1,
                    ));
                }
                if !xdigits2.as_bytes()[1].is_ascii_hexdigit() {
                    return Err(Error::new(
                        ErrorKind::InvalidPercentEncoding,
                        offset + literal.len() + 2,
                    ));
                }
                new_rest
            }
            Some((literal, b'{', expr_and_rest)) => {
                validate_literal(literal, offset)?;

                let (expr, new_rest) = match find_split_hole(expr_and_rest, b'}') {
                    Some(v) => v,
                    None => {
                        return Err(Error::new(
                            ErrorKind::ExpressionNotClosed,
                            offset + literal.len(),
                        ))
                    }
                };

                // +1 is `+ "{".len()`.
                validate_expr_body(expr, offset + literal.len() + 1)?;

                new_rest
            }
            Some(_) => unreachable!("[consistency] searching only `%` and `{{`"),
            None => return validate_literal(rest, offset),
        };
        offset = s.len() - rest.len();
    }

    Ok(())
}
