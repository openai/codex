use std::iter::Peekable;

use proc_macro::{token_stream, TokenStream};

use crate::date::Date;
use crate::error::Error;
use crate::time::Time;
use crate::to_tokens::ToTokenStream;
use crate::{date, time};

pub(crate) struct UtcDateTime {
    date: Date,
    time: Time,
}

pub(crate) fn parse(chars: &mut Peekable<token_stream::IntoIter>) -> Result<UtcDateTime, Error> {
    let date = date::parse(chars)?;
    let time = time::parse(chars)?;

    if let Some(token) = chars.peek() {
        return Err(Error::UnexpectedToken {
            tree: token.clone(),
        });
    }

    Ok(UtcDateTime { date, time })
}

impl ToTokenStream for UtcDateTime {
    fn append_to(self, ts: &mut TokenStream) {
        quote_append! { ts
            ::time::UtcDateTime::new(
                #S(self.date),
                #S(self.time),
            )
        }
    }
}
