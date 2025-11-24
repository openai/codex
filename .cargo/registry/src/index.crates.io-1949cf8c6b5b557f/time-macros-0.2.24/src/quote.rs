macro_rules! quote_ {
    () => (proc_macro::TokenStream::new());
    ($($x:tt)*) => {{
        use proc_macro::*;
        let mut ts = TokenStream::new();
        let ts_mut = &mut ts;
        quote_inner!(ts_mut $($x)*);
        ts
    }};
}

macro_rules! quote_append {
    ($ts:ident $($x:tt)*) => {{
        use proc_macro::*;
        quote_inner!($ts $($x)*);
    }};
}

macro_rules! quote_group {
    ({ $($x:tt)* }) => {{
        use proc_macro::*;
        TokenTree::Group(Group::new(
            Delimiter::Brace,
            quote_!($($x)*)
        ))
    }};
}

macro_rules! sym {
    ($ts:ident $x:tt $y:tt) => {
        $ts.extend([
            TokenTree::from(Punct::new($x, Spacing::Joint)),
            TokenTree::from(Punct::new($y, Spacing::Alone)),
        ]);
    };
    ($ts:ident $x:tt) => {
        $ts.extend([TokenTree::from(Punct::new($x, Spacing::Alone))]);
    };
}

#[allow(unused_macro_rules)] // Varies by feature flag combination.
macro_rules! quote_inner {
    // Base case
    ($ts:ident) => {};

    // Single or double symbols
    ($ts:ident :: $($tail:tt)*) => { sym!($ts ':' ':'); quote_inner!($ts $($tail)*); };
    ($ts:ident : $($tail:tt)*) => { sym!($ts ':'); quote_inner!($ts $($tail)*); };
    ($ts:ident = $($tail:tt)*) => { sym!($ts '='); quote_inner!($ts $($tail)*); };
    ($ts:ident ; $($tail:tt)*) => { sym!($ts ';'); quote_inner!($ts $($tail)*); };
    ($ts:ident , $($tail:tt)*) => { sym!($ts ','); quote_inner!($ts $($tail)*); };
    ($ts:ident . $($tail:tt)*) => { sym!($ts '.'); quote_inner!($ts $($tail)*); };
    ($ts:ident & $($tail:tt)*) => { sym!($ts '&'); quote_inner!($ts $($tail)*); };
    ($ts:ident < $($tail:tt)*) => { sym!($ts '<'); quote_inner!($ts $($tail)*); };
    ($ts:ident >> $($tail:tt)*) => { sym!($ts '>' '>'); quote_inner!($ts $($tail)*); };
    ($ts:ident > $($tail:tt)*) => { sym!($ts '>'); quote_inner!($ts $($tail)*); };
    ($ts:ident -> $($tail:tt)*) => { sym!($ts '-' '>'); quote_inner!($ts $($tail)*); };
    ($ts:ident ? $($tail:tt)*) => { sym!($ts '?'); quote_inner!($ts $($tail)*); };
    ($ts:ident ! $($tail:tt)*) => { sym!($ts '!'); quote_inner!($ts $($tail)*); };
    ($ts:ident | $($tail:tt)*) => { sym!($ts '|'); quote_inner!($ts $($tail)*); };
    ($ts:ident * $($tail:tt)*) => { sym!($ts '*'); quote_inner!($ts $($tail)*); };
    ($ts:ident + $($tail:tt)*) => { sym!($ts '+'); quote_inner!($ts $($tail)*); };

    // Identifier
    ($ts:ident $i:ident $($tail:tt)*) => {
        $ts.extend([TokenTree::from(Ident::new(
            &stringify!($i),
            Span::mixed_site(),
        ))]);
        quote_inner!($ts $($tail)*);
    };

    // Literal
    ($ts:ident 0 $($tail:tt)*) => {
        $ts.extend([TokenTree::from(Literal::usize_unsuffixed(0))]);
        quote_inner!($ts $($tail)*);
    };
    ($ts:ident $l:literal $($tail:tt)*) => {
        $ts.extend([TokenTree::from(Literal::string(&$l))]);
        quote_inner!($ts $($tail)*);
    };

    // Lifetime
    ($ts:ident $l:lifetime $($tail:tt)*) => {
        $ts.extend([
            TokenTree::from(
                Punct::new('\'', Spacing::Joint)
            ),
            TokenTree::from(Ident::new(
                stringify!($l).trim_start_matches(|c| c == '\''),
                Span::mixed_site(),
            )),
        ]);
        quote_inner!($ts $($tail)*);
    };

    // Attribute
    ($ts:ident #[$($inner:tt)*] $($tail:tt)*) => {
        $ts.extend([
            TokenTree::from(
                Punct::new('#', Spacing::Alone)
            ),
            TokenTree::Group(Group::new(
                Delimiter::Bracket,
                quote_!($($inner)*)
            )),
        ]);
        quote_inner!($ts $($tail)*);
    };

    // Groups
    ($ts:ident ($($inner:tt)*) $($tail:tt)*) => {
        $ts.extend([TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            quote_!($($inner)*)
        ))]);
        quote_inner!($ts $($tail)*);
    };
    ($ts:ident [$($inner:tt)*] $($tail:tt)*) => {
        $ts.extend([TokenTree::Group(Group::new(
            Delimiter::Bracket,
            quote_!($($inner)*)
        ))]);
        quote_inner!($ts $($tail)*);
    };
    ($ts:ident {$($inner:tt)*} $($tail:tt)*) => {
        $ts.extend([TokenTree::Group(Group::new(
            Delimiter::Brace,
            quote_!($($inner)*)
        ))]);
        quote_inner!($ts $($tail)*);
    };

    // Interpolated values
    // TokenTree by default
    ($ts:ident #($e:expr) $($tail:tt)*) => {
        $ts.extend([$crate::to_tokens::ToTokenTree::into_token_tree($e)]);
        quote_inner!($ts $($tail)*);
    };
    // Allow a TokenStream by request. It's more expensive, so avoid if possible.
    ($ts:ident #S($e:expr) $($tail:tt)*) => {
        $crate::to_tokens::ToTokenStream::append_to($e, $ts);
        quote_inner!($ts $($tail)*);
    };
}
