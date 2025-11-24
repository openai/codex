use std::num::NonZero;

use proc_macro::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};

use crate::to_tokens::{ToTokenStream, ToTokenTree};

macro_rules! to_tokens {
    (
        $(#[$struct_attr:meta])*
        $struct_vis:vis struct $struct_name:ident {$(
            $(#[$field_attr:meta])*
            $field_vis:vis $field_name:ident : $field_ty:ty = $default:pat
        ),* $(,)?}
    ) => {
        $(#[$struct_attr])*
        $struct_vis struct $struct_name {$(
            $(#[$field_attr])*
            $field_vis $field_name: $field_ty
        ),*}

        impl ToTokenTree for $struct_name {
            fn into_token_tree(self) -> TokenTree {
                let Self {$($field_name),*} = self;

                #[allow(clippy::redundant_pattern_matching)]
                if matches!(($(&$field_name,)*), ($($default,)*)) {
                    return TokenTree::Group(Group::new(
                        Delimiter::None,
                        quote_! { $struct_name::default() }
                    ));
                }

                let mut tokens = quote_! {
                    let mut value = $struct_name::default();
                };
                $(
                    #[allow(clippy::redundant_pattern_matching)]
                    if !matches!($field_name, $default) {
                        quote_append!(tokens value.$field_name =);
                        $field_name.append_to(&mut tokens);
                        quote_append!(tokens ;);
                    }
                )*
                quote_append!(tokens value);

                TokenTree::Group(Group::new(
                    Delimiter::Brace,
                    tokens,
                ))
            }
        }
    };

    (
        $(#[$enum_attr:meta])*
        $enum_vis:vis enum $enum_name:ident {$(
            $(#[$variant_attr:meta])*
            $variant_name:ident
        ),+ $(,)?}
    ) => {
        $(#[$enum_attr])*
        $enum_vis enum $enum_name {$(
            $(#[$variant_attr])*
            $variant_name
        ),+}

        impl ToTokenStream for $enum_name {
            fn append_to(self, ts: &mut TokenStream) {
                quote_append! { ts
                    $enum_name::
                };
                let name = match self {
                    $(Self::$variant_name => stringify!($variant_name)),+
                };
                ts.extend([TokenTree::Ident(Ident::new(name, Span::mixed_site()))]);
            }
        }
    }
}

to_tokens! {
    pub(crate) struct Day {
        pub(crate) padding: Padding = Padding::Zero,
    }
}

to_tokens! {
    pub(crate) enum MonthRepr {
        Numerical,
        Long,
        Short,
    }
}

to_tokens! {
    pub(crate) struct Month {
        pub(crate) padding: Padding = Padding::Zero,
        pub(crate) repr: MonthRepr = MonthRepr::Numerical,
        pub(crate) case_sensitive: bool = true,
    }
}

to_tokens! {
    pub(crate) struct Ordinal {
        pub(crate) padding: Padding = Padding::Zero,
    }
}

to_tokens! {
    pub(crate) enum WeekdayRepr {
        Short,
        Long,
        Sunday,
        Monday,
    }
}

to_tokens! {
    pub(crate) struct Weekday {
        pub(crate) repr: WeekdayRepr = WeekdayRepr::Long,
        pub(crate) one_indexed: bool = true,
        pub(crate) case_sensitive: bool = true,
    }
}

to_tokens! {
    pub(crate) enum WeekNumberRepr {
        Iso,
        Sunday,
        Monday,
    }
}

to_tokens! {
    pub(crate) struct WeekNumber {
        pub(crate) padding: Padding = Padding::Zero,
        pub(crate) repr: WeekNumberRepr = WeekNumberRepr::Iso,
    }
}

to_tokens! {
    pub(crate) enum YearRepr {
        Full,
        Century,
        LastTwo,
    }
}

to_tokens! {
    pub(crate) enum YearRange {
        Standard,
        Extended,
    }
}

to_tokens! {
    pub(crate) struct Year {
        pub(crate) padding: Padding = Padding::Zero,
        pub(crate) repr: YearRepr = YearRepr::Full,
        pub(crate) range: YearRange = YearRange::Extended,
        pub(crate) iso_week_based: bool = false,
        pub(crate) sign_is_mandatory: bool = false,
    }
}

to_tokens! {
    pub(crate) struct Hour {
        pub(crate) padding: Padding = Padding::Zero,
        pub(crate) is_12_hour_clock: bool = false,
    }
}

to_tokens! {
    pub(crate) struct Minute {
        pub(crate) padding: Padding = Padding::Zero,
    }
}

to_tokens! {
    pub(crate) struct Period {
        pub(crate) is_uppercase: bool = true,
        pub(crate) case_sensitive: bool = true,
    }
}

to_tokens! {
    pub(crate) struct Second {
        pub(crate) padding: Padding = Padding::Zero,
    }
}

to_tokens! {
    pub(crate) enum SubsecondDigits {
        One,
        Two,
        Three,
        Four,
        Five,
        Six,
        Seven,
        Eight,
        Nine,
        OneOrMore,
    }
}

to_tokens! {
    pub(crate) struct Subsecond {
        pub(crate) digits: SubsecondDigits = SubsecondDigits::OneOrMore,
    }
}

to_tokens! {
    pub(crate) struct OffsetHour {
        pub(crate) sign_is_mandatory: bool = false,
        pub(crate) padding: Padding = Padding::Zero,
    }
}

to_tokens! {
    pub(crate) struct OffsetMinute {
        pub(crate) padding: Padding = Padding::Zero,
    }
}

to_tokens! {
    pub(crate) struct OffsetSecond {
        pub(crate) padding: Padding = Padding::Zero,
    }
}

to_tokens! {
    pub(crate) enum Padding {
        Space,
        Zero,
        None,
    }
}

pub(crate) struct Ignore {
    pub(crate) count: NonZero<u16>,
}

impl ToTokenTree for Ignore {
    fn into_token_tree(self) -> TokenTree {
        quote_group! {{
            Ignore::count(#(self.count))
        }}
    }
}

to_tokens! {
    pub(crate) enum UnixTimestampPrecision {
        Second,
        Millisecond,
        Microsecond,
        Nanosecond,
    }
}

to_tokens! {
    pub(crate) struct UnixTimestamp {
        pub(crate) precision: UnixTimestampPrecision = UnixTimestampPrecision::Second,
        pub(crate) sign_is_mandatory: bool = false,
    }
}

to_tokens! {
    pub(crate) struct End {}
}
