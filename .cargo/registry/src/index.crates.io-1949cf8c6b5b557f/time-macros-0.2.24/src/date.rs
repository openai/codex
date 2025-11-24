use std::iter::Peekable;

use num_conv::Truncate;
use proc_macro::token_stream;
use time_core::util::{days_in_year, weeks_in_year};

use crate::helpers::{
    consume_any_ident, consume_number, consume_punct, days_in_year_month, ymd_to_yo, ywd_to_yo,
};
use crate::to_tokens::ToTokenStream;
use crate::Error;

#[cfg(feature = "large-dates")]
const MAX_YEAR: i32 = 999_999;
#[cfg(not(feature = "large-dates"))]
const MAX_YEAR: i32 = 9_999;

pub(crate) struct Date {
    pub(crate) year: i32,
    pub(crate) ordinal: u16,
}

pub(crate) fn parse(chars: &mut Peekable<token_stream::IntoIter>) -> Result<Date, Error> {
    let (year_sign_span, year_sign, explicit_sign) = if let Ok(span) = consume_punct('-', chars) {
        (Some(span), -1, true)
    } else if let Ok(span) = consume_punct('+', chars) {
        (Some(span), 1, true)
    } else {
        (None, 1, false)
    };
    let (year_span, mut year) = consume_number::<i32>("year", chars)?;
    year *= year_sign;
    if year.abs() > MAX_YEAR {
        return Err(Error::InvalidComponent {
            name: "year",
            value: year.to_string(),
            span_start: Some(year_sign_span.unwrap_or(year_span)),
            span_end: Some(year_span),
        });
    }
    if !explicit_sign && year.abs() >= 10_000 {
        return Err(Error::Custom {
            message: "years with more than four digits must have an explicit sign".into(),
            span_start: Some(year_sign_span.unwrap_or(year_span)),
            span_end: Some(year_span),
        });
    }

    consume_punct('-', chars)?;

    // year-week-day
    if let Ok(w_span) = consume_any_ident(&["W"], chars) {
        let (week_span, week) = consume_number::<u8>("week", chars)?;
        consume_punct('-', chars)?;
        let (day_span, day) = consume_number::<u8>("day", chars)?;

        if week > weeks_in_year(year) {
            return Err(Error::InvalidComponent {
                name: "week",
                value: week.to_string(),
                span_start: Some(w_span),
                span_end: Some(week_span),
            });
        }
        if day == 0 || day > 7 {
            return Err(Error::InvalidComponent {
                name: "day",
                value: day.to_string(),
                span_start: Some(day_span),
                span_end: Some(day_span),
            });
        }

        let (year, ordinal) = ywd_to_yo(year, week, day);

        return Ok(Date { year, ordinal });
    }

    // We don't yet know whether it's year-month-day or year-ordinal.
    let (month_or_ordinal_span, month_or_ordinal) =
        consume_number::<u16>("month or ordinal", chars)?;

    // year-month-day
    if consume_punct('-', chars).is_ok() {
        let (month_span, month) = (month_or_ordinal_span, month_or_ordinal);
        let (day_span, day) = consume_number::<u8>("day", chars)?;

        if month == 0 || month > 12 {
            return Err(Error::InvalidComponent {
                name: "month",
                value: month.to_string(),
                span_start: Some(month_span),
                span_end: Some(month_span),
            });
        }
        let month = month.truncate();
        if day == 0 || day > days_in_year_month(year, month) {
            return Err(Error::InvalidComponent {
                name: "day",
                value: day.to_string(),
                span_start: Some(day_span),
                span_end: Some(day_span),
            });
        }

        let (year, ordinal) = ymd_to_yo(year, month, day);

        Ok(Date { year, ordinal })
    }
    // year-ordinal
    else {
        let (ordinal_span, ordinal) = (month_or_ordinal_span, month_or_ordinal);

        if ordinal == 0 || ordinal > days_in_year(year) {
            return Err(Error::InvalidComponent {
                name: "ordinal",
                value: ordinal.to_string(),
                span_start: Some(ordinal_span),
                span_end: Some(ordinal_span),
            });
        }

        Ok(Date { year, ordinal })
    }
}

impl ToTokenStream for Date {
    fn append_to(self, ts: &mut proc_macro::TokenStream) {
        quote_append! { ts
            unsafe {
                ::time::Date::__from_ordinal_date_unchecked(
                    #(self.year),
                    #(self.ordinal),
                )
            }
        }
    }
}
