use std::iter::Peekable;

use proc_macro::{token_stream, TokenStream};

use crate::date::Date;
use crate::error::Error;
use crate::offset::Offset;
use crate::time::Time;
use crate::to_tokens::ToTokenStream;
use crate::{date, offset, time};

pub(crate) struct DateTime {
    date: Date,
    time: Time,
    offset: Option<Offset>,
}

pub(crate) fn parse(chars: &mut Peekable<token_stream::IntoIter>) -> Result<DateTime, Error> {
    let date = date::parse(chars)?;
    let time = time::parse(chars)?;
    let offset = match offset::parse(chars) {
        Ok(offset) => Some(offset),
        Err(Error::UnexpectedEndOfInput | Error::MissingComponent { name: "sign", .. }) => None,
        Err(err) => return Err(err),
    };

    if let Some(token) = chars.peek() {
        return Err(Error::UnexpectedToken {
            tree: token.clone(),
        });
    }

    Ok(DateTime { date, time, offset })
}

impl ToTokenStream for DateTime {
    fn append_to(self, ts: &mut TokenStream) {
        let maybe_offset = match self.offset {
            Some(offset) => quote_! { .assume_offset(#S(offset)) },
            None => quote_! {},
        };

        quote_append! { ts
            ::time::PrimitiveDateTime::new(
                #S(self.date),
                #S(self.time),
            ) #S(maybe_offset)
        }
    }
}
