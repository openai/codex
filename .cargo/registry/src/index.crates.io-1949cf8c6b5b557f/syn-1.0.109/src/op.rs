ast_enum! {
    /// A binary operator: `+`, `+=`, `&`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum BinOp {
        /// The `+` operator (addition)
        Add(Token![+]),
        /// The `-` operator (subtraction)
        Sub(Token![-]),
        /// The `*` operator (multiplication)
        Mul(Token![*]),
        /// The `/` operator (division)
        Div(Token![/]),
        /// The `%` operator (modulus)
        Rem(Token![%]),
        /// The `&&` operator (logical and)
        And(Token![&&]),
        /// The `||` operator (logical or)
        Or(Token![||]),
        /// The `^` operator (bitwise xor)
        BitXor(Token![^]),
        /// The `&` operator (bitwise and)
        BitAnd(Token![&]),
        /// The `|` operator (bitwise or)
        BitOr(Token![|]),
        /// The `<<` operator (shift left)
        Shl(Token![<<]),
        /// The `>>` operator (shift right)
        Shr(Token![>>]),
        /// The `==` operator (equality)
        Eq(Token![==]),
        /// The `<` operator (less than)
        Lt(Token![<]),
        /// The `<=` operator (less than or equal to)
        Le(Token![<=]),
        /// The `!=` operator (not equal to)
        Ne(Token![!=]),
        /// The `>=` operator (greater than or equal to)
        Ge(Token![>=]),
        /// The `>` operator (greater than)
        Gt(Token![>]),
        /// The `+=` operator
        AddEq(Token![+=]),
        /// The `-=` operator
        SubEq(Token![-=]),
        /// The `*=` operator
        MulEq(Token![*=]),
        /// The `/=` operator
        DivEq(Token![/=]),
        /// The `%=` operator
        RemEq(Token![%=]),
        /// The `^=` operator
        BitXorEq(Token![^=]),
        /// The `&=` operator
        BitAndEq(Token![&=]),
        /// The `|=` operator
        BitOrEq(Token![|=]),
        /// The `<<=` operator
        ShlEq(Token![<<=]),
        /// The `>>=` operator
        ShrEq(Token![>>=]),
    }
}

ast_enum! {
    /// A unary operator: `*`, `!`, `-`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum UnOp {
        /// The `*` operator for dereferencing
        Deref(Token![*]),
        /// The `!` operator for logical inversion
        Not(Token![!]),
        /// The `-` operator for negation
        Neg(Token![-]),
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;
    use crate::parse::{Parse, ParseStream, Result};

    fn parse_binop(input: ParseStream) -> Result<BinOp> {
        if input.peek(Token![&&]) {
            input.parse().map(BinOp::And)
        } else if input.peek(Token![||]) {
            input.parse().map(BinOp::Or)
        } else if input.peek(Token![<<]) {
            input.parse().map(BinOp::Shl)
        } else if input.peek(Token![>>]) {
            input.parse().map(BinOp::Shr)
        } else if input.peek(Token![==]) {
            input.parse().map(BinOp::Eq)
        } else if input.peek(Token![<=]) {
            input.parse().map(BinOp::Le)
        } else if input.peek(Token![!=]) {
            input.parse().map(BinOp::Ne)
        } else if input.peek(Token![>=]) {
            input.parse().map(BinOp::Ge)
        } else if input.peek(Token![+]) {
            input.parse().map(BinOp::Add)
        } else if input.peek(Token![-]) {
            input.parse().map(BinOp::Sub)
        } else if input.peek(Token![*]) {
            input.parse().map(BinOp::Mul)
        } else if input.peek(Token![/]) {
            input.parse().map(BinOp::Div)
        } else if input.peek(Token![%]) {
            input.parse().map(BinOp::Rem)
        } else if input.peek(Token![^]) {
            input.parse().map(BinOp::BitXor)
        } else if input.peek(Token![&]) {
            input.parse().map(BinOp::BitAnd)
        } else if input.peek(Token![|]) {
            input.parse().map(BinOp::BitOr)
        } else if input.peek(Token![<]) {
            input.parse().map(BinOp::Lt)
        } else if input.peek(Token![>]) {
            input.parse().map(BinOp::Gt)
        } else {
            Err(input.error("expected binary operator"))
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for BinOp {
        #[cfg(not(feature = "full"))]
        fn parse(input: ParseStream) -> Result<Self> {
            parse_binop(input)
        }

        #[cfg(feature = "full")]
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Token![+=]) {
                input.parse().map(BinOp::AddEq)
            } else if input.peek(Token![-=]) {
                input.parse().map(BinOp::SubEq)
            } else if input.peek(Token![*=]) {
                input.parse().map(BinOp::MulEq)
            } else if input.peek(Token![/=]) {
                input.parse().map(BinOp::DivEq)
            } else if input.peek(Token![%=]) {
                input.parse().map(BinOp::RemEq)
            } else if input.peek(Token![^=]) {
                input.parse().map(BinOp::BitXorEq)
            } else if input.peek(Token![&=]) {
                input.parse().map(BinOp::BitAndEq)
            } else if input.peek(Token![|=]) {
                input.parse().map(BinOp::BitOrEq)
            } else if input.peek(Token![<<=]) {
                input.parse().map(BinOp::ShlEq)
            } else if input.peek(Token![>>=]) {
                input.parse().map(BinOp::ShrEq)
            } else {
                parse_binop(input)
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for UnOp {
        fn parse(input: ParseStream) -> Result<Self> {
            let lookahead = input.lookahead1();
            if lookahead.peek(Token![*]) {
                input.parse().map(UnOp::Deref)
            } else if lookahead.peek(Token![!]) {
                input.parse().map(UnOp::Not)
            } else if lookahead.peek(Token![-]) {
                input.parse().map(UnOp::Neg)
            } else {
                Err(lookahead.error())
            }
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::ToTokens;

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for BinOp {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match self {
                BinOp::Add(t) => t.to_tokens(tokens),
                BinOp::Sub(t) => t.to_tokens(tokens),
                BinOp::Mul(t) => t.to_tokens(tokens),
                BinOp::Div(t) => t.to_tokens(tokens),
                BinOp::Rem(t) => t.to_tokens(tokens),
                BinOp::And(t) => t.to_tokens(tokens),
                BinOp::Or(t) => t.to_tokens(tokens),
                BinOp::BitXor(t) => t.to_tokens(tokens),
                BinOp::BitAnd(t) => t.to_tokens(tokens),
                BinOp::BitOr(t) => t.to_tokens(tokens),
                BinOp::Shl(t) => t.to_tokens(tokens),
                BinOp::Shr(t) => t.to_tokens(tokens),
                BinOp::Eq(t) => t.to_tokens(tokens),
                BinOp::Lt(t) => t.to_tokens(tokens),
                BinOp::Le(t) => t.to_tokens(tokens),
                BinOp::Ne(t) => t.to_tokens(tokens),
                BinOp::Ge(t) => t.to_tokens(tokens),
                BinOp::Gt(t) => t.to_tokens(tokens),
                BinOp::AddEq(t) => t.to_tokens(tokens),
                BinOp::SubEq(t) => t.to_tokens(tokens),
                BinOp::MulEq(t) => t.to_tokens(tokens),
                BinOp::DivEq(t) => t.to_tokens(tokens),
                BinOp::RemEq(t) => t.to_tokens(tokens),
                BinOp::BitXorEq(t) => t.to_tokens(tokens),
                BinOp::BitAndEq(t) => t.to_tokens(tokens),
                BinOp::BitOrEq(t) => t.to_tokens(tokens),
                BinOp::ShlEq(t) => t.to_tokens(tokens),
                BinOp::ShrEq(t) => t.to_tokens(tokens),
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for UnOp {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match self {
                UnOp::Deref(t) => t.to_tokens(tokens),
                UnOp::Not(t) => t.to_tokens(tokens),
                UnOp::Neg(t) => t.to_tokens(tokens),
            }
        }
    }
}
