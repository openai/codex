use proc_macro::{Ident, Span, TokenStream};

use super::modifier;
use crate::to_tokens::ToTokenStream;

macro_rules! declare_component {
    ($($name:ident)*) => {
        pub(crate) enum Component {$(
            $name(modifier::$name),
        )*}

        impl ToTokenStream for Component {
            fn append_to(self, ts: &mut TokenStream) {
                let mut mts = TokenStream::new();

                let component = match self {$(
                    Self::$name(modifier) => {
                        modifier.append_to(&mut mts);
                        stringify!($name)
                    }
                )*};
                let component = Ident::new(component, Span::mixed_site());

                quote_append! { ts
                    Component::#(component)(#S(mts))
                }
            }
        }
    };
}

declare_component! {
    Day
    Month
    Ordinal
    Weekday
    WeekNumber
    Year
    Hour
    Minute
    Period
    Second
    Subsecond
    OffsetHour
    OffsetMinute
    OffsetSecond
    Ignore
    UnixTimestamp
    End
}
