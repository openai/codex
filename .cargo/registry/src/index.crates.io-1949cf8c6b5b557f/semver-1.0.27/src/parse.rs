use crate::error::{ErrorKind, Position};
use crate::identifier::Identifier;
use crate::{BuildMetadata, Comparator, Op, Prerelease, Version, VersionReq};
use alloc::vec::Vec;
use core::str::FromStr;

/// Error parsing a SemVer version or version requirement.
///
/// # Example
///
/// ```
/// use semver::Version;
///
/// fn main() {
///     let err = Version::parse("1.q.r").unwrap_err();
///
///     // "unexpected character 'q' while parsing minor version number"
///     eprintln!("{}", err);
/// }
/// ```
pub struct Error {
    pub(crate) kind: ErrorKind,
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Err(Error::new(ErrorKind::Empty));
        }

        let mut pos = Position::Major;
        let (major, text) = numeric_identifier(text, pos)?;
        let text = dot(text, pos)?;

        pos = Position::Minor;
        let (minor, text) = numeric_identifier(text, pos)?;
        let text = dot(text, pos)?;

        pos = Position::Patch;
        let (patch, text) = numeric_identifier(text, pos)?;

        if text.is_empty() {
            return Ok(Version::new(major, minor, patch));
        }

        let (pre, text) = if let Some(text) = text.strip_prefix('-') {
            pos = Position::Pre;
            let (pre, text) = prerelease_identifier(text)?;
            if pre.is_empty() {
                return Err(Error::new(ErrorKind::EmptySegment(pos)));
            }
            (pre, text)
        } else {
            (Prerelease::EMPTY, text)
        };

        let (build, text) = if let Some(text) = text.strip_prefix('+') {
            pos = Position::Build;
            let (build, text) = build_identifier(text)?;
            if build.is_empty() {
                return Err(Error::new(ErrorKind::EmptySegment(pos)));
            }
            (build, text)
        } else {
            (BuildMetadata::EMPTY, text)
        };

        if let Some(unexpected) = text.chars().next() {
            return Err(Error::new(ErrorKind::UnexpectedCharAfter(pos, unexpected)));
        }

        Ok(Version {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }
}

impl FromStr for VersionReq {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim_start_matches(' ');
        if let Some((ch, text)) = wildcard(text) {
            let rest = text.trim_start_matches(' ');
            if rest.is_empty() {
                return Ok(VersionReq::STAR);
            } else if rest.starts_with(',') {
                return Err(Error::new(ErrorKind::WildcardNotTheOnlyComparator(ch)));
            } else {
                return Err(Error::new(ErrorKind::UnexpectedAfterWildcard));
            }
        }

        let depth = 0;
        let mut comparators = Vec::new();
        let len = version_req(text, &mut comparators, depth)?;
        unsafe { comparators.set_len(len) }
        Ok(VersionReq { comparators })
    }
}

impl FromStr for Comparator {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim_start_matches(' ');
        let (comparator, pos, rest) = comparator(text)?;
        if !rest.is_empty() {
            let unexpected = rest.chars().next().unwrap();
            return Err(Error::new(ErrorKind::UnexpectedCharAfter(pos, unexpected)));
        }
        Ok(comparator)
    }
}

impl FromStr for Prerelease {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (pre, rest) = prerelease_identifier(text)?;
        if !rest.is_empty() {
            return Err(Error::new(ErrorKind::IllegalCharacter(Position::Pre)));
        }
        Ok(pre)
    }
}

impl FromStr for BuildMetadata {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (build, rest) = build_identifier(text)?;
        if !rest.is_empty() {
            return Err(Error::new(ErrorKind::IllegalCharacter(Position::Build)));
        }
        Ok(build)
    }
}

impl Error {
    fn new(kind: ErrorKind) -> Self {
        Error { kind }
    }
}

impl Op {
    const DEFAULT: Self = Op::Caret;
}

fn numeric_identifier(input: &str, pos: Position) -> Result<(u64, &str), Error> {
    let mut len = 0;
    let mut value = 0u64;

    while let Some(&digit) = input.as_bytes().get(len) {
        if digit < b'0' || digit > b'9' {
            break;
        }
        if value == 0 && len > 0 {
            return Err(Error::new(ErrorKind::LeadingZero(pos)));
        }
        match value
            .checked_mul(10)
            .and_then(|value| value.checked_add((digit - b'0') as u64))
        {
            Some(sum) => value = sum,
            None => return Err(Error::new(ErrorKind::Overflow(pos))),
        }
        len += 1;
    }

    if len > 0 {
        Ok((value, &input[len..]))
    } else if let Some(unexpected) = input[len..].chars().next() {
        Err(Error::new(ErrorKind::UnexpectedChar(pos, unexpected)))
    } else {
        Err(Error::new(ErrorKind::UnexpectedEnd(pos)))
    }
}

fn wildcard(input: &str) -> Option<(char, &str)> {
    if let Some(rest) = input.strip_prefix('*') {
        Some(('*', rest))
    } else if let Some(rest) = input.strip_prefix('x') {
        Some(('x', rest))
    } else if let Some(rest) = input.strip_prefix('X') {
        Some(('X', rest))
    } else {
        None
    }
}

fn dot(input: &str, pos: Position) -> Result<&str, Error> {
    if let Some(rest) = input.strip_prefix('.') {
        Ok(rest)
    } else if let Some(unexpected) = input.chars().next() {
        Err(Error::new(ErrorKind::UnexpectedCharAfter(pos, unexpected)))
    } else {
        Err(Error::new(ErrorKind::UnexpectedEnd(pos)))
    }
}

fn prerelease_identifier(input: &str) -> Result<(Prerelease, &str), Error> {
    let (string, rest) = identifier(input, Position::Pre)?;
    let identifier = unsafe { Identifier::new_unchecked(string) };
    Ok((Prerelease { identifier }, rest))
}

fn build_identifier(input: &str) -> Result<(BuildMetadata, &str), Error> {
    let (string, rest) = identifier(input, Position::Build)?;
    let identifier = unsafe { Identifier::new_unchecked(string) };
    Ok((BuildMetadata { identifier }, rest))
}

fn identifier(input: &str, pos: Position) -> Result<(&str, &str), Error> {
    let mut accumulated_len = 0;
    let mut segment_len = 0;
    let mut segment_has_nondigit = false;

    loop {
        match input.as_bytes().get(accumulated_len + segment_len) {
            Some(b'A'..=b'Z') | Some(b'a'..=b'z') | Some(b'-') => {
                segment_len += 1;
                segment_has_nondigit = true;
            }
            Some(b'0'..=b'9') => {
                segment_len += 1;
            }
            boundary => {
                if segment_len == 0 {
                    if accumulated_len == 0 && boundary != Some(&b'.') {
                        return Ok(("", input));
                    } else {
                        return Err(Error::new(ErrorKind::EmptySegment(pos)));
                    }
                }
                if pos == Position::Pre
                    && segment_len > 1
                    && !segment_has_nondigit
                    && input[accumulated_len..].starts_with('0')
                {
                    return Err(Error::new(ErrorKind::LeadingZero(pos)));
                }
                accumulated_len += segment_len;
                if boundary == Some(&b'.') {
                    accumulated_len += 1;
                    segment_len = 0;
                    segment_has_nondigit = false;
                } else {
                    return Ok(input.split_at(accumulated_len));
                }
            }
        }
    }
}

fn op(input: &str) -> (Op, &str) {
    let bytes = input.as_bytes();
    if bytes.first() == Some(&b'=') {
        (Op::Exact, &input[1..])
    } else if bytes.first() == Some(&b'>') {
        if bytes.get(1) == Some(&b'=') {
            (Op::GreaterEq, &input[2..])
        } else {
            (Op::Greater, &input[1..])
        }
    } else if bytes.first() == Some(&b'<') {
        if bytes.get(1) == Some(&b'=') {
            (Op::LessEq, &input[2..])
        } else {
            (Op::Less, &input[1..])
        }
    } else if bytes.first() == Some(&b'~') {
        (Op::Tilde, &input[1..])
    } else if bytes.first() == Some(&b'^') {
        (Op::Caret, &input[1..])
    } else {
        (Op::DEFAULT, input)
    }
}

fn comparator(input: &str) -> Result<(Comparator, Position, &str), Error> {
    let (mut op, text) = op(input);
    let default_op = input.len() == text.len();
    let text = text.trim_start_matches(' ');

    let mut pos = Position::Major;
    let (major, text) = numeric_identifier(text, pos)?;
    let mut has_wildcard = false;

    let (minor, text) = if let Some(text) = text.strip_prefix('.') {
        pos = Position::Minor;
        if let Some((_, text)) = wildcard(text) {
            has_wildcard = true;
            if default_op {
                op = Op::Wildcard;
            }
            (None, text)
        } else {
            let (minor, text) = numeric_identifier(text, pos)?;
            (Some(minor), text)
        }
    } else {
        (None, text)
    };

    let (patch, text) = if let Some(text) = text.strip_prefix('.') {
        pos = Position::Patch;
        if let Some((_, text)) = wildcard(text) {
            if default_op {
                op = Op::Wildcard;
            }
            (None, text)
        } else if has_wildcard {
            return Err(Error::new(ErrorKind::UnexpectedAfterWildcard));
        } else {
            let (patch, text) = numeric_identifier(text, pos)?;
            (Some(patch), text)
        }
    } else {
        (None, text)
    };

    let (pre, text) = if patch.is_some() && text.starts_with('-') {
        pos = Position::Pre;
        let text = &text[1..];
        let (pre, text) = prerelease_identifier(text)?;
        if pre.is_empty() {
            return Err(Error::new(ErrorKind::EmptySegment(pos)));
        }
        (pre, text)
    } else {
        (Prerelease::EMPTY, text)
    };

    let text = if patch.is_some() && text.starts_with('+') {
        pos = Position::Build;
        let text = &text[1..];
        let (build, text) = build_identifier(text)?;
        if build.is_empty() {
            return Err(Error::new(ErrorKind::EmptySegment(pos)));
        }
        text
    } else {
        text
    };

    let text = text.trim_start_matches(' ');

    let comparator = Comparator {
        op,
        major,
        minor,
        patch,
        pre,
    };

    Ok((comparator, pos, text))
}

fn version_req(input: &str, out: &mut Vec<Comparator>, depth: usize) -> Result<usize, Error> {
    let (comparator, pos, text) = match comparator(input) {
        Ok(success) => success,
        Err(mut error) => {
            if let Some((ch, mut rest)) = wildcard(input) {
                rest = rest.trim_start_matches(' ');
                if rest.is_empty() || rest.starts_with(',') {
                    error.kind = ErrorKind::WildcardNotTheOnlyComparator(ch);
                }
            }
            return Err(error);
        }
    };

    if text.is_empty() {
        out.reserve_exact(depth + 1);
        unsafe { out.as_mut_ptr().add(depth).write(comparator) }
        return Ok(depth + 1);
    }

    let text = if let Some(text) = text.strip_prefix(',') {
        text.trim_start_matches(' ')
    } else {
        let unexpected = text.chars().next().unwrap();
        return Err(Error::new(ErrorKind::ExpectedCommaFound(pos, unexpected)));
    };

    const MAX_COMPARATORS: usize = 32;
    if depth + 1 == MAX_COMPARATORS {
        return Err(Error::new(ErrorKind::ExcessiveComparators));
    }

    // Recurse to collect parsed Comparator objects on the stack. We perform a
    // single allocation to allocate exactly the right sized Vec only once the
    // total number of comparators is known.
    let len = version_req(text, out, depth + 1)?;
    unsafe { out.as_mut_ptr().add(depth).write(comparator) }
    Ok(len)
}
