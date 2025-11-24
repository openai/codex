use std::{fmt, str::FromStr};

use crate::{AuthMechanism, Error, Guid, OwnedGuid, Result};

// The plain-text SASL profile authentication protocol described here:
// <https://dbus.freedesktop.org/doc/dbus-specification.html#auth-protocol>
//
// These are all the known commands, which can be parsed from or serialized to text.
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
pub(super) enum Command {
    Auth(Option<AuthMechanism>, Option<Vec<u8>>),
    Cancel,
    Begin,
    Data(Option<Vec<u8>>),
    Error(String),
    NegotiateUnixFD,
    Rejected(Vec<AuthMechanism>),
    Ok(OwnedGuid),
    AgreeUnixFD,
}

impl From<&Command> for Vec<u8> {
    fn from(c: &Command) -> Self {
        c.to_string().into()
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Auth(mech, resp) => match (mech, resp) {
                (Some(mech), Some(resp)) => write!(f, "AUTH {mech} {}", hex::encode(resp)),
                (Some(mech), None) => write!(f, "AUTH {mech}"),
                _ => write!(f, "AUTH"),
            },
            Command::Cancel => write!(f, "CANCEL"),
            Command::Begin => write!(f, "BEGIN"),
            Command::Data(data) => match data {
                None => write!(f, "DATA"),
                Some(data) => write!(f, "DATA {}", hex::encode(data)),
            },
            Command::Error(expl) => write!(f, "ERROR {expl}"),
            Command::NegotiateUnixFD => write!(f, "NEGOTIATE_UNIX_FD"),
            Command::Rejected(mechs) => {
                write!(
                    f,
                    "REJECTED {}",
                    mechs
                        .iter()
                        .map(|m| m.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
            Command::Ok(guid) => write!(f, "OK {guid}"),
            Command::AgreeUnixFD => write!(f, "AGREE_UNIX_FD"),
        }
    }
}

impl FromStr for Command {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut words = s.split_ascii_whitespace();
        let cmd = match words.next() {
            Some("AUTH") => {
                let mech = if let Some(m) = words.next() {
                    Some(m.parse()?)
                } else {
                    None
                };
                let resp = match words.next() {
                    Some(resp) => Some(hex::decode(resp)?),
                    None => None,
                };
                Command::Auth(mech, resp)
            }
            Some("CANCEL") => Command::Cancel,
            Some("BEGIN") => Command::Begin,
            Some("DATA") => {
                let data = match words.next() {
                    Some(data) => Some(hex::decode(data)?),
                    None => None,
                };

                Command::Data(data)
            }
            Some("ERROR") => Command::Error(s.into()),
            Some("NEGOTIATE_UNIX_FD") => Command::NegotiateUnixFD,
            Some("REJECTED") => {
                let mechs = words.map(|m| m.parse()).collect::<Result<_>>()?;
                Command::Rejected(mechs)
            }
            Some("OK") => {
                let guid = words
                    .next()
                    .ok_or_else(|| Error::Handshake("Missing OK server GUID!".into()))?;
                Command::Ok(Guid::from_str(guid)?.into())
            }
            Some("AGREE_UNIX_FD") => Command::AgreeUnixFD,
            _ => return Err(Error::Handshake(format!("Unknown command: {s}"))),
        };
        Ok(cmd)
    }
}
