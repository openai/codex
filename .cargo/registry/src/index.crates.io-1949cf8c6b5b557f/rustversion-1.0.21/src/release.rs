use crate::error::{Error, Result};
use crate::iter::Iter;
use crate::token;
use proc_macro::Group;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Release {
    pub minor: u16,
    pub patch: Option<u16>,
}

pub fn parse(paren: Group, iter: Iter) -> Result<Release> {
    try_parse(iter).map_err(|()| Error::group(paren, "expected rustc release number, like 1.31"))
}

fn try_parse(iter: Iter) -> Result<Release, ()> {
    let major_minor = token::parse_literal(iter).map_err(drop)?;
    let string = major_minor.to_string();

    if !string.starts_with("1.") {
        return Err(());
    }

    let minor: u16 = string[2..].parse().map_err(drop)?;

    let patch = if token::parse_optional_punct(iter, '.').is_some() {
        let int = token::parse_literal(iter).map_err(drop)?;
        Some(int.to_string().parse().map_err(drop)?)
    } else {
        None
    };

    Ok(Release { minor, patch })
}
