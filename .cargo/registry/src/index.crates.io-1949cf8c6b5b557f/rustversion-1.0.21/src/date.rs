use crate::error::{Error, Result};
use crate::iter::Iter;
use crate::{time, token};
use proc_macro::Group;
use std::fmt::{self, Display};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Display for Date {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day,
        )
    }
}

pub fn parse(paren: Group, iter: Iter) -> Result<Date> {
    try_parse(iter).map_err(|()| {
        let msg = format!("expected nightly date, like {}", time::today());
        Error::group(paren, msg)
    })
}

fn try_parse(iter: Iter) -> Result<Date, ()> {
    let year = token::parse_literal(iter).map_err(drop)?;
    token::parse_punct(iter, '-').map_err(drop)?;
    let month = token::parse_literal(iter).map_err(drop)?;
    token::parse_punct(iter, '-').map_err(drop)?;
    let day = token::parse_literal(iter).map_err(drop)?;

    let year = year.to_string().parse::<u64>().map_err(drop)?;
    let month = month.to_string().parse::<u64>().map_err(drop)?;
    let day = day.to_string().parse::<u64>().map_err(drop)?;
    if year >= 3000 || month > 12 || day > 31 {
        return Err(());
    }

    Ok(Date {
        year: year as u16,
        month: month as u8,
        day: day as u8,
    })
}
