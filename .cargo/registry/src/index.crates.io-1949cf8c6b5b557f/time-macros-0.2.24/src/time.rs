use std::iter::Peekable;

use proc_macro::{token_stream, Span, TokenStream};
use time_core::convert::*;

use crate::helpers::{consume_any_ident, consume_number, consume_punct};
use crate::to_tokens::ToTokenStream;
use crate::Error;

enum Period {
    Am,
    Pm,
    _24,
}

pub(crate) struct Time {
    pub(crate) hour: u8,
    pub(crate) minute: u8,
    pub(crate) second: u8,
    pub(crate) nanosecond: u32,
}

pub(crate) fn parse(chars: &mut Peekable<token_stream::IntoIter>) -> Result<Time, Error> {
    fn consume_period(chars: &mut Peekable<token_stream::IntoIter>) -> (Option<Span>, Period) {
        if let Ok(span) = consume_any_ident(&["am", "AM"], chars) {
            (Some(span), Period::Am)
        } else if let Ok(span) = consume_any_ident(&["pm", "PM"], chars) {
            (Some(span), Period::Pm)
        } else {
            (None, Period::_24)
        }
    }

    let (hour_span, hour) = consume_number("hour", chars)?;

    let ((minute_span, minute), (second_span, second), (period_span, period)) =
        match consume_period(chars) {
            // Nothing but the 12-hour clock hour and AM/PM
            (period_span @ Some(_), period) => (
                (Span::mixed_site(), 0),
                (Span::mixed_site(), 0.),
                (period_span, period),
            ),
            (None, _) => {
                consume_punct(':', chars)?;
                let (minute_span, minute) = consume_number::<u8>("minute", chars)?;
                let (second_span, second): (_, f64) = if consume_punct(':', chars).is_ok() {
                    consume_number("second", chars)?
                } else {
                    (Span::mixed_site(), 0.)
                };
                let (period_span, period) = consume_period(chars);
                (
                    (minute_span, minute),
                    (second_span, second),
                    (period_span, period),
                )
            }
        };

    let hour = match (hour, period) {
        (0, Period::Am | Period::Pm) => {
            return Err(Error::InvalidComponent {
                name: "hour",
                value: hour.to_string(),
                span_start: Some(hour_span),
                span_end: Some(period_span.unwrap_or(hour_span)),
            });
        }
        (12, Period::Am) => 0,
        (12, Period::Pm) => 12,
        (hour, Period::Am | Period::_24) => hour,
        (hour, Period::Pm) => hour + 12,
    };

    if hour >= Hour::per_t(Day) {
        Err(Error::InvalidComponent {
            name: "hour",
            value: hour.to_string(),
            span_start: Some(hour_span),
            span_end: Some(period_span.unwrap_or(hour_span)),
        })
    } else if minute >= Minute::per_t(Hour) {
        Err(Error::InvalidComponent {
            name: "minute",
            value: minute.to_string(),
            span_start: Some(minute_span),
            span_end: Some(minute_span),
        })
    } else if second >= Second::per_t(Minute) {
        Err(Error::InvalidComponent {
            name: "second",
            value: second.to_string(),
            span_start: Some(second_span),
            span_end: Some(second_span),
        })
    } else {
        Ok(Time {
            hour,
            minute,
            second: second.trunc() as u8,
            nanosecond: (second.fract() * Nanosecond::per_t::<f64>(Second)).round() as u32,
        })
    }
}

impl ToTokenStream for Time {
    fn append_to(self, ts: &mut TokenStream) {
        quote_append! { ts
            unsafe {
                ::time::Time::__from_hms_nanos_unchecked(
                    #(self.hour),
                    #(self.minute),
                    #(self.second),
                    #(self.nanosecond),
                )
            }
        }
    }
}
