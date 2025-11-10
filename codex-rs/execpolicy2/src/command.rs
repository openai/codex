use crate::error::Error;
use crate::error::Result;

pub fn tokenize_command(raw: &str) -> Result<Vec<String>> {
    shlex::split(raw).ok_or_else(|| Error::TokenizationFailed {
        example: raw.to_string(),
        reason: "invalid shell tokens".to_string(),
    })
}
