use codex_exec_server::HttpHeader;
use reqwest::header::WWW_AUTHENTICATE;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct InsufficientScopeChallenge {
    pub(super) www_authenticate_header: String,
    pub(super) required_scope: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct BearerInsufficientScope {
    required_scope: Option<String>,
}

type AuthParameter<'a> = (&'a str, Option<String>);
type ChallengeStart<'a> = (&'a str, Option<AuthParameter<'a>>);

#[derive(Default)]
enum Parameter {
    #[default]
    Missing,
    Value(String),
    Invalid,
}

#[derive(Default)]
struct BearerChallenge {
    error: Parameter,
    scope: Parameter,
}

impl BearerChallenge {
    fn add_parameter(&mut self, name: &str, value: Option<String>) {
        let parameter = if name.eq_ignore_ascii_case("error") {
            &mut self.error
        } else if name.eq_ignore_ascii_case("scope") {
            &mut self.scope
        } else {
            return;
        };

        *parameter = match (&*parameter, value) {
            (Parameter::Missing, Some(value)) => Parameter::Value(value),
            (Parameter::Missing, None) | (Parameter::Value(_), _) | (Parameter::Invalid, _) => {
                Parameter::Invalid
            }
        };
    }

    fn into_insufficient_scope(self) -> Option<BearerInsufficientScope> {
        match self.error {
            Parameter::Value(error) if error == "insufficient_scope" => {
                Some(BearerInsufficientScope {
                    required_scope: match self.scope {
                        Parameter::Value(scope) if valid_scope(&scope) => Some(scope),
                        Parameter::Missing | Parameter::Value(_) | Parameter::Invalid => None,
                    },
                })
            }
            Parameter::Missing | Parameter::Value(_) | Parameter::Invalid => None,
        }
    }

    fn has_error(&self, expected: &str) -> bool {
        matches!(&self.error, Parameter::Value(error) if error == expected)
    }
}

/// Finds a Bearer insufficient-scope challenge among all `WWW-Authenticate`
/// response header field values.
pub(super) fn insufficient_scope_challenge(
    headers: &[HttpHeader],
) -> Option<InsufficientScopeChallenge> {
    headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case(WWW_AUTHENTICATE.as_str()))
        .find_map(|header| {
            parse_bearer_insufficient_scope(&header.value).map(|challenge| {
                InsufficientScopeChallenge {
                    www_authenticate_header: header.value.clone(),
                    required_scope: challenge.required_scope,
                }
            })
        })
}

/// Returns whether any `WWW-Authenticate` response header field value contains
/// a Bearer challenge with an `invalid_token` error.
pub(super) fn has_bearer_invalid_token_challenge(headers: &[HttpHeader]) -> bool {
    headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case(WWW_AUTHENTICATE.as_str()))
        .any(|header| parse_bearer_challenge(&header.value, "invalid_token").is_some())
}

/// Parses a Bearer `WWW-Authenticate` challenge with an `insufficient_scope`
/// error and extracts its optional required scope.
///
/// RFC 9110 section 11.2 defines challenge parameters as `auth-param` values
/// whose values are either `token` or `quoted-string`. Quoted strings use HTTP
/// syntax rather than JSON: section 5.6.4 requires recipients to replace each
/// `quoted-pair` with its escaped octet.
///
/// RFC 6750 section 3 permits `scope` in the Bearer challenge at most once.
/// After HTTP quoted-string processing, each scope token can contain `%x21`,
/// `%x23-5B`, or `%x5D-7E`, with `%x20` separating multiple tokens. Therefore
/// returned scopes cannot contain `"` or `\`, even when those characters occur
/// in the header encoding.
///
/// RMCP has related parsing logic, but it is private to that crate.
fn parse_bearer_insufficient_scope(header: &str) -> Option<BearerInsufficientScope> {
    parse_bearer_challenge(header, "insufficient_scope")
        .and_then(BearerChallenge::into_insufficient_scope)
}

fn parse_bearer_challenge(header: &str, expected_error: &str) -> Option<BearerChallenge> {
    let segments = split_unquoted_segments(header)?;
    let mut bearer_challenge: Option<BearerChallenge> = None;

    for segment in segments {
        if let Some((name, value)) = parse_auth_param(segment) {
            if let Some(challenge) = bearer_challenge.as_mut() {
                challenge.add_parameter(name, value);
            }
            continue;
        }

        if let Some(challenge) = bearer_challenge.take()
            && challenge.has_error(expected_error)
        {
            return Some(challenge);
        }

        let (scheme, parameter) = parse_challenge_start(segment)?;
        if scheme.eq_ignore_ascii_case("Bearer") {
            let mut challenge = BearerChallenge::default();
            if let Some((name, value)) = parameter {
                challenge.add_parameter(name, value);
            }
            bearer_challenge = Some(challenge);
        }
    }

    bearer_challenge.filter(|challenge| challenge.has_error(expected_error))
}

fn parse_challenge_start(segment: &str) -> Option<ChallengeStart<'_>> {
    let segment = segment.trim();
    let parameter_start = segment.find(char::is_whitespace);
    let (scheme, parameter) = match parameter_start {
        Some(parameter_start) => (
            &segment[..parameter_start],
            parse_auth_param(&segment[parameter_start..]),
        ),
        None => (segment, None),
    };

    is_http_token(scheme).then_some((scheme, parameter))
}

fn parse_auth_param(segment: &str) -> Option<AuthParameter<'_>> {
    let (name, value) = segment.trim().split_once('=')?;
    let name = name.trim();
    is_http_token(name).then_some((name, parse_auth_param_value(value.trim())))
}

fn parse_auth_param_value(value: &str) -> Option<String> {
    if let Some(quoted_value) = value.strip_prefix('"') {
        let quoted_value = quoted_value.strip_suffix('"')?;
        let mut decoded = String::with_capacity(quoted_value.len());
        let mut characters = quoted_value.chars();
        while let Some(character) = characters.next() {
            if character == '\\' {
                decoded.push(characters.next()?);
            } else {
                decoded.push(character);
            }
        }
        Some(decoded)
    } else {
        is_http_token(value).then(|| value.to_string())
    }
}

fn split_unquoted_segments(header: &str) -> Option<Vec<&str>> {
    let mut segments = Vec::new();
    let mut segment_start = 0;
    let mut in_quotes = false;
    let mut escaped = false;

    for (position, character) in header.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            ',' | ';' if !in_quotes => {
                segments.push(&header[segment_start..position]);
                segment_start = position + character.len_utf8();
            }
            _ => {}
        }
    }

    if in_quotes || escaped {
        None
    } else {
        segments.push(&header[segment_start..]);
        Some(segments)
    }
}

fn valid_scope(scope: &str) -> bool {
    scope.split(' ').all(|token| {
        !token.is_empty()
            && token
                .bytes()
                .all(|byte| matches!(byte, b'!' | b'#'..=b'[' | b']'..=b'~'))
    })
}

fn is_http_token(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

#[cfg(test)]
#[path = "www_authenticate_tests.rs"]
mod tests;
