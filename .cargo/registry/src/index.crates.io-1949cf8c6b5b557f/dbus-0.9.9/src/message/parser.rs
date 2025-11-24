use std::error::Error as stdError;
use std::fmt::{Debug, Formatter, Display};
use crate::message::MatchRule;
use crate::{MessageType, Path};
use std::convert::TryFrom;
use crate::strings::{Interface, BusName, Member};

// Our grammar:
// rules: rule (, rule)*
// rule: sender | type | interface | member | path | path_namespace | destination | arg | arg_path
// bool : 'true' | 'false'
// type: "type" "=" message_type
// message_type: "'signal'" | "'method_call'" | "'method_return'" | "'error'"
// sender: "sender" "=" string
// interface: "interface" "=" string
// member: "member" "=" string
// path: "path" "=" string
// path_namespace: "path_namespace" "=" string
// destination: "destination" "=" string
// arg: "arg" 0-63 "=" string
// arg_path: "arg" 0-63 "path" "=" string
// eavesdrop: "eavesdrop" "=" bool


#[derive(Clone, Debug)]
/// Error type that covers errors that might happen during parsing.
pub enum Error {
    /// The type specified in the match rule is unknown
    UnknownType,
    /// The key is wrong / unsupported
    UnknownKey,
    /// Boolean could not be parsed
    BadBoolean,
    /// Error that occurred while converting a string to a DBus format
    BadConversion(String),
    /// Key with no value
    NoValue,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error while parsing MatchRule: ")?;
        match self {
            Error::UnknownType => {
                write!(f, "Unsupported message type")
            }
            Error::UnknownKey => {
                write!(f, "Unknown key used")
            }
            Error::BadBoolean => {
                write!(f, "Got bad boolean value")
            }
            Error::BadConversion(err) => {
                write!(f, "Error while converting: {}", err)
            }
            Error::NoValue => {
                write!(f, "Key with no value")
            }
        }
    }
}

impl stdError for Error {}

/// Key-Value-pair
pub type TokenRule<'a> = (&'a str, &'a str);
/// Fixed size buffer for match rule tokens
pub type TokenBuffer<'a> = Vec<TokenRule<'a>>;

#[derive(Clone, Debug)]
/// Tokenizes a match rule into key-value-pairs
struct Tokenizer<'a> {
    text: &'a str,
}

impl<'a> Tokenizer<'a> {
    /// Builds a new tokenizer for the input &str
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
        }
    }

    /// Parse the key part of the key-value-pair. This is rather easy as all keys are rather
    /// easily defined and may only contain `[a-z0-9]` so we can simply split at `'='`.
    fn key(&self) -> (&'a str, &'a str) {
        let index = self.text.find('=').unwrap_or_else(|| self.text.len());

        ((&self.text[..index]).trim(), &self.text[index + 1..])
    }

    /// Parses values as generic strings.
    /// This does not do any validation (yet) with regards to supported characters.
    fn value(&self) -> (&'a str, &'a str) {
        let mut i = 0;
        let mut quoted = false;
        let mut escape = false;

        for c in self.text.chars() {
            match c {
                '\'' if !escape => {
                    quoted = !quoted;
                }
                ',' if !quoted => {
                    break;
                }
                '\\' if !quoted => {
                    escape = true;
                    i += 1;
                    continue;
                }
                _ => {}
            }
            escape = false;

            i += 1;
        }

        // Skip comma if there is still space in the buffer
        let j = if self.text.len() == i { i } else { i + 1 };
        ((&self.text[..i]).trim(), &self.text[j..])
    }

    /// Tokenizes a string into key-value-pairs
    pub fn tokenize(&mut self) -> Result<TokenBuffer<'a>, Error> {
        let mut rules = TokenBuffer::new();

        while !self.text.is_empty() {
            let (key, rest) = self.key();
            self.text = rest;
            if self.text.is_empty()
                { return Err(Error::NoValue) }
            let (value, rest) = self.value();
            self.text = rest;
            rules.push((key, value))
        }
        Ok(rules)
    }
}

#[derive(Clone, Debug)]
/// Helper struct for parsing MatchRule's
pub struct Parser<'a> {
    tokens: TokenBuffer<'a>,
}

impl<'a> Parser<'a> {
    /// Builds a new parser after tokenizing the input string `text`.
    pub fn new(text: &'a str) -> Result<Self, Error> {
        Ok(Self {
            tokens: Tokenizer::new(text).tokenize()?
        })
    }

    /// Cleans a string from the string syntax allowed by DBus. This includes concatenating
    /// things like `''\'''` to `'`. DBus strings sort of work like in a POSIX shell and
    /// concatenation is implied. There is only one escape sequence and that is in an unquoted
    /// substring a backslash may escape an ASCII apostrophe (U+0027).
    /// This method is the only one within the parser that allocates and in theory could be
    /// rewritten to taking a `&mut str` instead of returning a `String` since all
    /// strings are |output| <= |buf.len()|.
    fn clean_string(&self, buf: &str) -> String {
        let mut quoted = false;
        let mut escape = false;
        let mut outbuf = String::with_capacity(buf.len());

        for c in buf.chars() {
            match c {
                '\'' if !escape => {
                    quoted = !quoted;
                }
                '\\' if !quoted => {
                    escape = true;
                    continue;
                }
                c if c.is_whitespace() && !quoted => {
                    continue;
                }
                c => {
                    outbuf.push(c);
                }
            }
        }

        outbuf
    }

    /// Parses key-value-pair tokens into a MatchRule
    pub fn parse(&self) -> Result<MatchRule<'a>, Error> {
        let mut match_rule = MatchRule::new();

        for &(key, raw_value) in &self.tokens {
            let value = self.clean_string(raw_value);
            match key {
                "type" => {
                    match_rule = match_rule.with_type(MessageType::try_from(value.as_str()).map_err(|_| Error::UnknownType)?);
                    Ok(())
                }
                "interface" => {
                    match_rule.interface = Some(Interface::new(value).map_err(Error::BadConversion)?);
                    Ok(())
                }
                "sender" => {
                    match_rule.sender = Some(BusName::new(value).map_err(Error::BadConversion)?);
                    Ok(())
                }
                "member" => {
                    match_rule.member = Some(Member::new(value).map_err(Error::BadConversion)?);
                    Ok(())
                }
                "path" => {
                    match_rule.path = Some(Path::new(value).map_err(Error::BadConversion)?);
                    Ok(())
                }
                "path_namespace" => {
                    match_rule.path = Some(Path::new(value).map_err(Error::BadConversion)?);
                    match_rule.path_is_namespace = true;
                    Ok(())
                }
                "eavesdrop" => {
                    match raw_value {
                        "'true'" | "true" => {
                            match_rule = match_rule.with_eavesdrop();
                            Ok(())
                        }
                        "'false'" | "false" => {
                            Ok(())
                        }
                        _ => {
                            Err(Error::BadBoolean)
                        }
                    }
                }
                _ => {
                    // Args and Destination are not supported yet.
                    Err(Error::UnknownKey)
                }
            }?;
        }

        Ok(match_rule)
    }
}

#[cfg(test)]
mod tests {
    use crate::message::parser::Error;
    use crate::message::MatchRule;

    #[test]
    fn test_tokenizer() -> Result<(), Error> {
        let mr = MatchRule::parse(r"interface='org.freedesktop.Notifications',member='Notify'")?;
        assert_eq!(mr.match_str(), "interface='org.freedesktop.Notifications',member='Notify'");
        let mr = MatchRule::parse(r"interface='org.mpris.MediaPlayer2.Player' , path= /org/mpris/MediaPlayer2,member='Notify', eavesdrop ='true'")?;
        assert_eq!(mr.match_str(), "path='/org/mpris/MediaPlayer2',interface='org.mpris.MediaPlayer2.Player',member='Notify',eavesdrop='true'");
        Ok(())
    }

    #[test]
    fn test_malformed() {
        assert!(MatchRule::parse(r"interface='org.freedesktop.Notifications',member=").is_err());
    }

    #[test]
    fn test_spurious_comma() {
        assert!(MatchRule::parse(r"interface='org.freedesktop.Notifications',").is_ok());
    }
}