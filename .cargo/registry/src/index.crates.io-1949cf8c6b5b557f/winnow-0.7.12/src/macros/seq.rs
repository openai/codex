/// Initialize a struct or tuple out of a sequences of parsers
///
/// Unlike normal struct initialization syntax:
/// - `_` fields can exist to run a parser but ignore the result
/// - Parse results for a field can later be referenced using the field name
///
/// Unlike normal tuple initialization syntax:
/// - Struct-style initialization (`{ 0: _, 1: _}`) is not supported
/// - `_: <parser>` fields can exist to run a parser but ignore the result
///
///# Example
///
/// ```
/// # use winnow::prelude::*;
/// # use winnow::ascii::{alphanumeric1, dec_uint, space0};
/// # use winnow::combinator::delimited;
/// # use winnow::combinator::empty;
/// # use winnow::error::ContextError;
/// # use winnow::error::ErrMode;
/// use winnow::combinator::seq;
///
/// #[derive(Default, Debug, PartialEq)]
/// struct Field {
///     namespace: u32,
///     name: Vec<u8>,
///     value: Vec<u8>,
///     point: (u32, u32),
///     metadata: Vec<u8>,
/// }
///
/// // Parse into structs / tuple-structs
/// fn field(input: &mut &[u8]) -> ModalResult<Field> {
///     seq!{Field {
///         namespace: empty.value(5),
///         name: alphanumeric1.map(|s: &[u8]| s.to_owned()),
///         // `_` fields are ignored when building the struct
///         _: (space0, b':', space0),
///         value: alphanumeric1.map(|s: &[u8]| s.to_owned()),
///         _: (space0, b':', space0),
///         point: point,
///         // default initialization also works
///         ..Default::default()
///     }}.parse_next(input)
/// }
///
/// // Or parse into tuples
/// fn point(input: &mut &[u8]) -> ModalResult<(u32, u32)> {
///     let mut num = dec_uint::<_, u32, ErrMode<ContextError>>;
///     seq!(num, _: (space0, b',', space0), num).parse_next(input)
/// }
///
/// assert_eq!(
///     field.parse_peek(&b"test: data: 123 , 4"[..]),
///     Ok((
///         &b""[..],
///         Field {
///             namespace: 5,
///             name: b"test"[..].to_owned(),
///             value: b"data"[..].to_owned(),
///             point: (123, 4),
///             metadata: Default::default(),
///         },
///     )),
/// );
/// ```
#[macro_export]
#[doc(alias = "tuple")]
#[doc(alias = "preceded")]
#[doc(alias = "terminated")]
#[doc(alias = "delimited")]
#[doc(alias = "pair")]
#[doc(alias = "separated_pair")]
#[doc(alias = "struct_parser")]
#[doc(hidden)] // forced to be visible in intended location
macro_rules! seq {
    ($($name: ident)::* { $($fields: tt)* }) => {
        $crate::combinator::trace(stringify!($($name)::*), move |input: &mut _| {
            $crate::seq_parse_struct_fields!(
                ( $($fields)* );
                ( _0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15, _16, _17, _18, _19, _20 );
                input ;
            );
            Ok($crate::seq_init_struct_fields!(
                ( $($fields)* );
                $($name)::* ;
            ))
        })
    };
    ($($name: ident)::* ( $($fields: tt)* )) => {
        $crate::combinator::trace(stringify!($($name)::*), move |input: &mut _| {
            $crate::seq_parse_tuple_fields!(
                ( $($fields)* );
                ( _0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15, _16, _17, _18, _19, _20 );
                input;
            );
            Ok($crate::seq_init_tuple_fields!(
                ( $($fields)* );
                ( _0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15, _16, _17, _18, _19, _20 );
                $($name)::*;
            ))
        })
    };
    (( $($fields: tt)* )) => {
        $crate::combinator::trace("tuple", move |input: &mut _| {
            $crate::seq_parse_tuple_fields!(
                ( $($fields)* );
                ( _0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15, _16, _17, _18, _19, _20 );
                input;
            );
            Ok($crate::seq_init_tuple_fields!(
                ( $($fields)* );
                ( _0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15, _16, _17, _18, _19, _20 );
                ;
            ))
        })
    };
    ($($fields: tt)*) => {
        $crate::seq!((
            $($fields)*
        ))
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! seq_parse_struct_fields {
    (
        ( _ : $head_parser: expr, $($fields: tt)* );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $input: ident ;
    ) => {
        let $unnamed1 = $crate::Parser::parse_next(&mut $head_parser, $input)?;
        $crate::seq_parse_struct_fields!(
            ( $($fields)* );
            ( $($unnamed),* );
            $input ;
        )
    };
    (
        ( _ : $head_parser: expr );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $input: ident ;
    ) => {
        let $unnamed1 = $crate::Parser::parse_next(&mut $head_parser, $input)?;
    };
    (
        ( $head_field: ident : $head_parser: expr, $($fields: tt)* );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $input: ident ;
    ) => {
        let $head_field = $crate::Parser::parse_next(&mut $head_parser, $input)?;
        $crate::seq_parse_struct_fields!(
            ( $($fields)* );
            ( $($unnamed),* );
            $input ;
        )
    };
    (
        ( $head_field: ident : $head_parser: expr );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $input: ident ;
    ) => {
        let $head_field = $crate::Parser::parse_next(&mut $head_parser, $input)?;
    };
    (
        ( .. $update: expr );
        ( $($unnamed: ident),* );
        $input: expr ;
    ) => {};
    (
        ( $(,)? );
        ( $($unnamed: ident),* );
        $input: expr ;
    ) => {};
}

#[macro_export]
#[doc(hidden)]
macro_rules! seq_parse_tuple_fields {
    (
        ( $(_ :)? $head_parser: expr, $($fields: tt)* );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $input: ident;
    ) => {
        let $unnamed1 = $crate::Parser::parse_next(&mut $head_parser, $input)?;
        $crate::seq_parse_tuple_fields!(
            ( $($fields)* );
            ( $($unnamed),* );
            $input ;
        )
    };
    (
        ( $(_ :)? $head_parser: expr );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $input: ident;
    ) => {
        let $unnamed1 = $crate::Parser::parse_next(&mut $head_parser, $input)?;
    };
    (
        ( $(,)? );
        ( $($unnamed: ident),* );
        $input: expr;
    ) => {};
}

#[macro_export]
#[doc(hidden)]
macro_rules! seq_init_struct_fields {
    (
        ( _ : $head_parser: expr, $($fields: tt)* );
        $($name: ident)::* ;
        $($inits: tt)*
    ) => {
        $crate::seq_init_struct_fields!(
            ( $($fields)* );
            $($name)::* ;
            $($inits)*
        )
    };
    (
        ( _ : $head_parser: expr );
        $($name: ident)::* ;
        $($inits: tt)*
    ) => {
        $crate::seq_init_struct_fields!(
            ();
            $($name)::* ;
            $($inits)*
        )
    };
    (
        ( $head_field: ident : $head_parser: expr, $($fields: tt)* );
        $($name: ident)::* ;
        $($inits: tt)*
    ) =>
    {
        $crate::seq_init_struct_fields!(
            ( $($fields)* );
            $($name)::* ;
            $($inits)* $head_field,
        )
    };
    (
        ( $head_field: ident : $head_parser: expr );
        $($name: ident)::* ;
        $($inits: tt)*
    ) => {
        $crate::seq_init_struct_fields!(
            ();
            $($name)::* ;
            $($inits)* $head_field,
        )
    };
    (
        ( .. $update: expr );
        $($name: ident)::* ;
        $($inits: tt)*
    ) => {
        $($name)::* { $($inits)* ..$update }
    };
    (
        ( $(,)? );
        $($name: ident)::* ;
        $($inits: tt)*
    ) => {
        $($name)::* { $($inits)* }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! seq_init_tuple_fields {
    (
        ( _ : $head_parser: expr, $($fields: tt)* );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $($name: ident)::*;
        $($inits: tt)*
    ) => {
        $crate::seq_init_tuple_fields!(
            ( $($fields)* );
            ( $($unnamed),* );
            $($name)::* ;
            $($inits)*
        )
    };
    (
        ( _ : $head_parser: expr );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $($name: ident)::*;
        $($inits: tt)*
    ) => {
        $crate::seq_init_tuple_fields!(
            ();
            ( $($unnamed),* );
            $($name)::* ;
            $($inits)*
        )
    };
    (
        ( $head_parser: expr, $($fields: tt)* );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $($name: ident)::*;
        $($inits: tt)*
    ) =>
    {
        $crate::seq_init_tuple_fields!(
            ( $($fields)* );
            ( $($unnamed),* );
            $($name)::* ;
            $($inits)* $unnamed1,
        )
    };
    (
        ( $head_parser: expr );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $($name: ident)::*;
        $($inits: tt)*
    ) => {
        $crate::seq_init_tuple_fields!(
            ();
            ( $($unnamed),* );
            $($name)::* ;
            $($inits)* $unnamed1,
        )
    };
    (
        ( $(,)? );
        ( $unnamed1: ident, $($unnamed: ident),* );
        $($name: ident)::*;
        $($inits: tt)*
    ) => {
        $($name)::* ( $($inits)* )
    };
}
