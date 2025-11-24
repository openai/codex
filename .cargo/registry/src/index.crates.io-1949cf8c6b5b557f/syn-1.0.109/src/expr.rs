use super::*;
use crate::punctuated::Punctuated;
#[cfg(feature = "full")]
use crate::reserved::Reserved;
use proc_macro2::{Span, TokenStream};
#[cfg(feature = "printing")]
use quote::IdentFragment;
#[cfg(feature = "printing")]
use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};
#[cfg(feature = "parsing")]
use std::mem;

ast_enum_of_structs! {
    /// A Rust expression.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature, but most of the variants are not available unless "full" is enabled.*
    ///
    /// # Syntax tree enums
    ///
    /// This type is a syntax tree enum. In Syn this and other syntax tree enums
    /// are designed to be traversed using the following rebinding idiom.
    ///
    /// ```
    /// # use syn::Expr;
    /// #
    /// # fn example(expr: Expr) {
    /// # const IGNORE: &str = stringify! {
    /// let expr: Expr = /* ... */;
    /// # };
    /// match expr {
    ///     Expr::MethodCall(expr) => {
    ///         /* ... */
    ///     }
    ///     Expr::Cast(expr) => {
    ///         /* ... */
    ///     }
    ///     Expr::If(expr) => {
    ///         /* ... */
    ///     }
    ///
    ///     /* ... */
    ///     # _ => {}
    /// # }
    /// # }
    /// ```
    ///
    /// We begin with a variable `expr` of type `Expr` that has no fields
    /// (because it is an enum), and by matching on it and rebinding a variable
    /// with the same name `expr` we effectively imbue our variable with all of
    /// the data fields provided by the variant that it turned out to be. So for
    /// example above if we ended up in the `MethodCall` case then we get to use
    /// `expr.receiver`, `expr.args` etc; if we ended up in the `If` case we get
    /// to use `expr.cond`, `expr.then_branch`, `expr.else_branch`.
    ///
    /// This approach avoids repeating the variant names twice on every line.
    ///
    /// ```
    /// # use syn::{Expr, ExprMethodCall};
    /// #
    /// # fn example(expr: Expr) {
    /// // Repetitive; recommend not doing this.
    /// match expr {
    ///     Expr::MethodCall(ExprMethodCall { method, args, .. }) => {
    /// # }
    /// # _ => {}
    /// # }
    /// # }
    /// ```
    ///
    /// In general, the name to which a syntax tree enum variant is bound should
    /// be a suitable name for the complete syntax tree enum type.
    ///
    /// ```
    /// # use syn::{Expr, ExprField};
    /// #
    /// # fn example(discriminant: ExprField) {
    /// // Binding is called `base` which is the name I would use if I were
    /// // assigning `*discriminant.base` without an `if let`.
    /// if let Expr::Tuple(base) = *discriminant.base {
    /// # }
    /// # }
    /// ```
    ///
    /// A sign that you may not be choosing the right variable names is if you
    /// see names getting repeated in your code, like accessing
    /// `receiver.receiver` or `pat.pat` or `cond.cond`.
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    #[cfg_attr(not(syn_no_non_exhaustive), non_exhaustive)]
    pub enum Expr {
        /// A slice literal expression: `[a, b, c, d]`.
        Array(ExprArray),

        /// An assignment expression: `a = compute()`.
        Assign(ExprAssign),

        /// A compound assignment expression: `counter += 1`.
        AssignOp(ExprAssignOp),

        /// An async block: `async { ... }`.
        Async(ExprAsync),

        /// An await expression: `fut.await`.
        Await(ExprAwait),

        /// A binary operation: `a + b`, `a * b`.
        Binary(ExprBinary),

        /// A blocked scope: `{ ... }`.
        Block(ExprBlock),

        /// A box expression: `box f`.
        Box(ExprBox),

        /// A `break`, with an optional label to break and an optional
        /// expression.
        Break(ExprBreak),

        /// A function call expression: `invoke(a, b)`.
        Call(ExprCall),

        /// A cast expression: `foo as f64`.
        Cast(ExprCast),

        /// A closure expression: `|a, b| a + b`.
        Closure(ExprClosure),

        /// A `continue`, with an optional label.
        Continue(ExprContinue),

        /// Access of a named struct field (`obj.k`) or unnamed tuple struct
        /// field (`obj.0`).
        Field(ExprField),

        /// A for loop: `for pat in expr { ... }`.
        ForLoop(ExprForLoop),

        /// An expression contained within invisible delimiters.
        ///
        /// This variant is important for faithfully representing the precedence
        /// of expressions and is related to `None`-delimited spans in a
        /// `TokenStream`.
        Group(ExprGroup),

        /// An `if` expression with an optional `else` block: `if expr { ... }
        /// else { ... }`.
        ///
        /// The `else` branch expression may only be an `If` or `Block`
        /// expression, not any of the other types of expression.
        If(ExprIf),

        /// A square bracketed indexing expression: `vector[2]`.
        Index(ExprIndex),

        /// A `let` guard: `let Some(x) = opt`.
        Let(ExprLet),

        /// A literal in place of an expression: `1`, `"foo"`.
        Lit(ExprLit),

        /// Conditionless loop: `loop { ... }`.
        Loop(ExprLoop),

        /// A macro invocation expression: `format!("{}", q)`.
        Macro(ExprMacro),

        /// A `match` expression: `match n { Some(n) => {}, None => {} }`.
        Match(ExprMatch),

        /// A method call expression: `x.foo::<T>(a, b)`.
        MethodCall(ExprMethodCall),

        /// A parenthesized expression: `(a + b)`.
        Paren(ExprParen),

        /// A path like `std::mem::replace` possibly containing generic
        /// parameters and a qualified self-type.
        ///
        /// A plain identifier like `x` is a path of length 1.
        Path(ExprPath),

        /// A range expression: `1..2`, `1..`, `..2`, `1..=2`, `..=2`.
        Range(ExprRange),

        /// A referencing operation: `&a` or `&mut a`.
        Reference(ExprReference),

        /// An array literal constructed from one repeated element: `[0u8; N]`.
        Repeat(ExprRepeat),

        /// A `return`, with an optional value to be returned.
        Return(ExprReturn),

        /// A struct literal expression: `Point { x: 1, y: 1 }`.
        ///
        /// The `rest` provides the value of the remaining fields as in `S { a:
        /// 1, b: 1, ..rest }`.
        Struct(ExprStruct),

        /// A try-expression: `expr?`.
        Try(ExprTry),

        /// A try block: `try { ... }`.
        TryBlock(ExprTryBlock),

        /// A tuple expression: `(a, b, c, d)`.
        Tuple(ExprTuple),

        /// A type ascription expression: `foo: f64`.
        Type(ExprType),

        /// A unary operation: `!x`, `*x`.
        Unary(ExprUnary),

        /// An unsafe block: `unsafe { ... }`.
        Unsafe(ExprUnsafe),

        /// Tokens in expression position not interpreted by Syn.
        Verbatim(TokenStream),

        /// A while loop: `while expr { ... }`.
        While(ExprWhile),

        /// A yield expression: `yield expr`.
        Yield(ExprYield),

        // Not public API.
        //
        // For testing exhaustiveness in downstream code, use the following idiom:
        //
        //     match expr {
        //         Expr::Array(expr) => {...}
        //         Expr::Assign(expr) => {...}
        //         ...
        //         Expr::Yield(expr) => {...}
        //
        //         #[cfg_attr(test, deny(non_exhaustive_omitted_patterns))]
        //         _ => { /* some sane fallback */ }
        //     }
        //
        // This way we fail your tests but don't break your library when adding
        // a variant. You will be notified by a test failure when a variant is
        // added, so that you can add code to handle it, but your library will
        // continue to compile and work for downstream users in the interim.
        #[cfg(syn_no_non_exhaustive)]
        #[doc(hidden)]
        __NonExhaustive,
    }
}

ast_struct! {
    /// A slice literal expression: `[a, b, c, d]`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprArray #full {
        pub attrs: Vec<Attribute>,
        pub bracket_token: token::Bracket,
        pub elems: Punctuated<Expr, Token![,]>,
    }
}

ast_struct! {
    /// An assignment expression: `a = compute()`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprAssign #full {
        pub attrs: Vec<Attribute>,
        pub left: Box<Expr>,
        pub eq_token: Token![=],
        pub right: Box<Expr>,
    }
}

ast_struct! {
    /// A compound assignment expression: `counter += 1`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprAssignOp #full {
        pub attrs: Vec<Attribute>,
        pub left: Box<Expr>,
        pub op: BinOp,
        pub right: Box<Expr>,
    }
}

ast_struct! {
    /// An async block: `async { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprAsync #full {
        pub attrs: Vec<Attribute>,
        pub async_token: Token![async],
        pub capture: Option<Token![move]>,
        pub block: Block,
    }
}

ast_struct! {
    /// An await expression: `fut.await`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprAwait #full {
        pub attrs: Vec<Attribute>,
        pub base: Box<Expr>,
        pub dot_token: Token![.],
        pub await_token: token::Await,
    }
}

ast_struct! {
    /// A binary operation: `a + b`, `a * b`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ExprBinary {
        pub attrs: Vec<Attribute>,
        pub left: Box<Expr>,
        pub op: BinOp,
        pub right: Box<Expr>,
    }
}

ast_struct! {
    /// A blocked scope: `{ ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprBlock #full {
        pub attrs: Vec<Attribute>,
        pub label: Option<Label>,
        pub block: Block,
    }
}

ast_struct! {
    /// A box expression: `box f`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprBox #full {
        pub attrs: Vec<Attribute>,
        pub box_token: Token![box],
        pub expr: Box<Expr>,
    }
}

ast_struct! {
    /// A `break`, with an optional label to break and an optional
    /// expression.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprBreak #full {
        pub attrs: Vec<Attribute>,
        pub break_token: Token![break],
        pub label: Option<Lifetime>,
        pub expr: Option<Box<Expr>>,
    }
}

ast_struct! {
    /// A function call expression: `invoke(a, b)`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ExprCall {
        pub attrs: Vec<Attribute>,
        pub func: Box<Expr>,
        pub paren_token: token::Paren,
        pub args: Punctuated<Expr, Token![,]>,
    }
}

ast_struct! {
    /// A cast expression: `foo as f64`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ExprCast {
        pub attrs: Vec<Attribute>,
        pub expr: Box<Expr>,
        pub as_token: Token![as],
        pub ty: Box<Type>,
    }
}

ast_struct! {
    /// A closure expression: `|a, b| a + b`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprClosure #full {
        pub attrs: Vec<Attribute>,
        pub movability: Option<Token![static]>,
        pub asyncness: Option<Token![async]>,
        pub capture: Option<Token![move]>,
        pub or1_token: Token![|],
        pub inputs: Punctuated<Pat, Token![,]>,
        pub or2_token: Token![|],
        pub output: ReturnType,
        pub body: Box<Expr>,
    }
}

ast_struct! {
    /// A `continue`, with an optional label.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprContinue #full {
        pub attrs: Vec<Attribute>,
        pub continue_token: Token![continue],
        pub label: Option<Lifetime>,
    }
}

ast_struct! {
    /// Access of a named struct field (`obj.k`) or unnamed tuple struct
    /// field (`obj.0`).
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ExprField {
        pub attrs: Vec<Attribute>,
        pub base: Box<Expr>,
        pub dot_token: Token![.],
        pub member: Member,
    }
}

ast_struct! {
    /// A for loop: `for pat in expr { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprForLoop #full {
        pub attrs: Vec<Attribute>,
        pub label: Option<Label>,
        pub for_token: Token![for],
        pub pat: Pat,
        pub in_token: Token![in],
        pub expr: Box<Expr>,
        pub body: Block,
    }
}

ast_struct! {
    /// An expression contained within invisible delimiters.
    ///
    /// This variant is important for faithfully representing the precedence
    /// of expressions and is related to `None`-delimited spans in a
    /// `TokenStream`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprGroup #full {
        pub attrs: Vec<Attribute>,
        pub group_token: token::Group,
        pub expr: Box<Expr>,
    }
}

ast_struct! {
    /// An `if` expression with an optional `else` block: `if expr { ... }
    /// else { ... }`.
    ///
    /// The `else` branch expression may only be an `If` or `Block`
    /// expression, not any of the other types of expression.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprIf #full {
        pub attrs: Vec<Attribute>,
        pub if_token: Token![if],
        pub cond: Box<Expr>,
        pub then_branch: Block,
        pub else_branch: Option<(Token![else], Box<Expr>)>,
    }
}

ast_struct! {
    /// A square bracketed indexing expression: `vector[2]`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ExprIndex {
        pub attrs: Vec<Attribute>,
        pub expr: Box<Expr>,
        pub bracket_token: token::Bracket,
        pub index: Box<Expr>,
    }
}

ast_struct! {
    /// A `let` guard: `let Some(x) = opt`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprLet #full {
        pub attrs: Vec<Attribute>,
        pub let_token: Token![let],
        pub pat: Pat,
        pub eq_token: Token![=],
        pub expr: Box<Expr>,
    }
}

ast_struct! {
    /// A literal in place of an expression: `1`, `"foo"`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ExprLit {
        pub attrs: Vec<Attribute>,
        pub lit: Lit,
    }
}

ast_struct! {
    /// Conditionless loop: `loop { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprLoop #full {
        pub attrs: Vec<Attribute>,
        pub label: Option<Label>,
        pub loop_token: Token![loop],
        pub body: Block,
    }
}

ast_struct! {
    /// A macro invocation expression: `format!("{}", q)`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprMacro #full {
        pub attrs: Vec<Attribute>,
        pub mac: Macro,
    }
}

ast_struct! {
    /// A `match` expression: `match n { Some(n) => {}, None => {} }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprMatch #full {
        pub attrs: Vec<Attribute>,
        pub match_token: Token![match],
        pub expr: Box<Expr>,
        pub brace_token: token::Brace,
        pub arms: Vec<Arm>,
    }
}

ast_struct! {
    /// A method call expression: `x.foo::<T>(a, b)`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprMethodCall #full {
        pub attrs: Vec<Attribute>,
        pub receiver: Box<Expr>,
        pub dot_token: Token![.],
        pub method: Ident,
        pub turbofish: Option<MethodTurbofish>,
        pub paren_token: token::Paren,
        pub args: Punctuated<Expr, Token![,]>,
    }
}

ast_struct! {
    /// A parenthesized expression: `(a + b)`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ExprParen {
        pub attrs: Vec<Attribute>,
        pub paren_token: token::Paren,
        pub expr: Box<Expr>,
    }
}

ast_struct! {
    /// A path like `std::mem::replace` possibly containing generic
    /// parameters and a qualified self-type.
    ///
    /// A plain identifier like `x` is a path of length 1.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ExprPath {
        pub attrs: Vec<Attribute>,
        pub qself: Option<QSelf>,
        pub path: Path,
    }
}

ast_struct! {
    /// A range expression: `1..2`, `1..`, `..2`, `1..=2`, `..=2`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprRange #full {
        pub attrs: Vec<Attribute>,
        pub from: Option<Box<Expr>>,
        pub limits: RangeLimits,
        pub to: Option<Box<Expr>>,
    }
}

ast_struct! {
    /// A referencing operation: `&a` or `&mut a`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprReference #full {
        pub attrs: Vec<Attribute>,
        pub and_token: Token![&],
        pub raw: Reserved,
        pub mutability: Option<Token![mut]>,
        pub expr: Box<Expr>,
    }
}

ast_struct! {
    /// An array literal constructed from one repeated element: `[0u8; N]`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprRepeat #full {
        pub attrs: Vec<Attribute>,
        pub bracket_token: token::Bracket,
        pub expr: Box<Expr>,
        pub semi_token: Token![;],
        pub len: Box<Expr>,
    }
}

ast_struct! {
    /// A `return`, with an optional value to be returned.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprReturn #full {
        pub attrs: Vec<Attribute>,
        pub return_token: Token![return],
        pub expr: Option<Box<Expr>>,
    }
}

ast_struct! {
    /// A struct literal expression: `Point { x: 1, y: 1 }`.
    ///
    /// The `rest` provides the value of the remaining fields as in `S { a:
    /// 1, b: 1, ..rest }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprStruct #full {
        pub attrs: Vec<Attribute>,
        pub path: Path,
        pub brace_token: token::Brace,
        pub fields: Punctuated<FieldValue, Token![,]>,
        pub dot2_token: Option<Token![..]>,
        pub rest: Option<Box<Expr>>,
    }
}

ast_struct! {
    /// A try-expression: `expr?`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprTry #full {
        pub attrs: Vec<Attribute>,
        pub expr: Box<Expr>,
        pub question_token: Token![?],
    }
}

ast_struct! {
    /// A try block: `try { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprTryBlock #full {
        pub attrs: Vec<Attribute>,
        pub try_token: Token![try],
        pub block: Block,
    }
}

ast_struct! {
    /// A tuple expression: `(a, b, c, d)`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprTuple #full {
        pub attrs: Vec<Attribute>,
        pub paren_token: token::Paren,
        pub elems: Punctuated<Expr, Token![,]>,
    }
}

ast_struct! {
    /// A type ascription expression: `foo: f64`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprType #full {
        pub attrs: Vec<Attribute>,
        pub expr: Box<Expr>,
        pub colon_token: Token![:],
        pub ty: Box<Type>,
    }
}

ast_struct! {
    /// A unary operation: `!x`, `*x`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ExprUnary {
        pub attrs: Vec<Attribute>,
        pub op: UnOp,
        pub expr: Box<Expr>,
    }
}

ast_struct! {
    /// An unsafe block: `unsafe { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprUnsafe #full {
        pub attrs: Vec<Attribute>,
        pub unsafe_token: Token![unsafe],
        pub block: Block,
    }
}

ast_struct! {
    /// A while loop: `while expr { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprWhile #full {
        pub attrs: Vec<Attribute>,
        pub label: Option<Label>,
        pub while_token: Token![while],
        pub cond: Box<Expr>,
        pub body: Block,
    }
}

ast_struct! {
    /// A yield expression: `yield expr`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ExprYield #full {
        pub attrs: Vec<Attribute>,
        pub yield_token: Token![yield],
        pub expr: Option<Box<Expr>>,
    }
}

impl Expr {
    #[cfg(all(feature = "parsing", not(syn_no_const_vec_new)))]
    const DUMMY: Self = Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: None,
        path: Path {
            leading_colon: None,
            segments: Punctuated::new(),
        },
    });

    #[cfg(all(feature = "parsing", feature = "full"))]
    pub(crate) fn replace_attrs(&mut self, new: Vec<Attribute>) -> Vec<Attribute> {
        match self {
            Expr::Box(ExprBox { attrs, .. })
            | Expr::Array(ExprArray { attrs, .. })
            | Expr::Call(ExprCall { attrs, .. })
            | Expr::MethodCall(ExprMethodCall { attrs, .. })
            | Expr::Tuple(ExprTuple { attrs, .. })
            | Expr::Binary(ExprBinary { attrs, .. })
            | Expr::Unary(ExprUnary { attrs, .. })
            | Expr::Lit(ExprLit { attrs, .. })
            | Expr::Cast(ExprCast { attrs, .. })
            | Expr::Type(ExprType { attrs, .. })
            | Expr::Let(ExprLet { attrs, .. })
            | Expr::If(ExprIf { attrs, .. })
            | Expr::While(ExprWhile { attrs, .. })
            | Expr::ForLoop(ExprForLoop { attrs, .. })
            | Expr::Loop(ExprLoop { attrs, .. })
            | Expr::Match(ExprMatch { attrs, .. })
            | Expr::Closure(ExprClosure { attrs, .. })
            | Expr::Unsafe(ExprUnsafe { attrs, .. })
            | Expr::Block(ExprBlock { attrs, .. })
            | Expr::Assign(ExprAssign { attrs, .. })
            | Expr::AssignOp(ExprAssignOp { attrs, .. })
            | Expr::Field(ExprField { attrs, .. })
            | Expr::Index(ExprIndex { attrs, .. })
            | Expr::Range(ExprRange { attrs, .. })
            | Expr::Path(ExprPath { attrs, .. })
            | Expr::Reference(ExprReference { attrs, .. })
            | Expr::Break(ExprBreak { attrs, .. })
            | Expr::Continue(ExprContinue { attrs, .. })
            | Expr::Return(ExprReturn { attrs, .. })
            | Expr::Macro(ExprMacro { attrs, .. })
            | Expr::Struct(ExprStruct { attrs, .. })
            | Expr::Repeat(ExprRepeat { attrs, .. })
            | Expr::Paren(ExprParen { attrs, .. })
            | Expr::Group(ExprGroup { attrs, .. })
            | Expr::Try(ExprTry { attrs, .. })
            | Expr::Async(ExprAsync { attrs, .. })
            | Expr::Await(ExprAwait { attrs, .. })
            | Expr::TryBlock(ExprTryBlock { attrs, .. })
            | Expr::Yield(ExprYield { attrs, .. }) => mem::replace(attrs, new),
            Expr::Verbatim(_) => Vec::new(),

            #[cfg(syn_no_non_exhaustive)]
            _ => unreachable!(),
        }
    }
}

ast_enum! {
    /// A struct or tuple struct field accessed in a struct literal or field
    /// expression.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum Member {
        /// A named field like `self.x`.
        Named(Ident),
        /// An unnamed field like `self.0`.
        Unnamed(Index),
    }
}

impl From<Ident> for Member {
    fn from(ident: Ident) -> Member {
        Member::Named(ident)
    }
}

impl From<Index> for Member {
    fn from(index: Index) -> Member {
        Member::Unnamed(index)
    }
}

impl From<usize> for Member {
    fn from(index: usize) -> Member {
        Member::Unnamed(Index::from(index))
    }
}

impl Eq for Member {}

impl PartialEq for Member {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Member::Named(this), Member::Named(other)) => this == other,
            (Member::Unnamed(this), Member::Unnamed(other)) => this == other,
            _ => false,
        }
    }
}

impl Hash for Member {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Member::Named(m) => m.hash(state),
            Member::Unnamed(m) => m.hash(state),
        }
    }
}

#[cfg(feature = "printing")]
impl IdentFragment for Member {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Member::Named(m) => Display::fmt(m, formatter),
            Member::Unnamed(m) => Display::fmt(&m.index, formatter),
        }
    }

    fn span(&self) -> Option<Span> {
        match self {
            Member::Named(m) => Some(m.span()),
            Member::Unnamed(m) => Some(m.span),
        }
    }
}

ast_struct! {
    /// The index of an unnamed tuple struct field.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Index {
        pub index: u32,
        pub span: Span,
    }
}

impl From<usize> for Index {
    fn from(index: usize) -> Index {
        assert!(index < u32::max_value() as usize);
        Index {
            index: index as u32,
            span: Span::call_site(),
        }
    }
}

impl Eq for Index {}

impl PartialEq for Index {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl Hash for Index {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

#[cfg(feature = "printing")]
impl IdentFragment for Index {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.index, formatter)
    }

    fn span(&self) -> Option<Span> {
        Some(self.span)
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// The `::<>` explicit type parameters passed to a method call:
    /// `parse::<u64>()`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct MethodTurbofish {
        pub colon2_token: Token![::],
        pub lt_token: Token![<],
        pub args: Punctuated<GenericMethodArgument, Token![,]>,
        pub gt_token: Token![>],
    }
}

#[cfg(feature = "full")]
ast_enum! {
    /// An individual generic argument to a method, like `T`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub enum GenericMethodArgument {
        /// A type argument.
        Type(Type),
        /// A const expression. Must be inside of a block.
        ///
        /// NOTE: Identity expressions are represented as Type arguments, as
        /// they are indistinguishable syntactically.
        Const(Expr),
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// A field-value pair in a struct literal.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct FieldValue {
        /// Attributes tagged on the field.
        pub attrs: Vec<Attribute>,

        /// Name or index of the field.
        pub member: Member,

        /// The colon in `Struct { x: x }`. If written in shorthand like
        /// `Struct { x }`, there is no colon.
        pub colon_token: Option<Token![:]>,

        /// Value of the field.
        pub expr: Expr,
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// A lifetime labeling a `for`, `while`, or `loop`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct Label {
        pub name: Lifetime,
        pub colon_token: Token![:],
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// One arm of a `match` expression: `0...10 => { return true; }`.
    ///
    /// As in:
    ///
    /// ```
    /// # fn f() -> bool {
    /// #     let n = 0;
    /// match n {
    ///     0...10 => {
    ///         return true;
    ///     }
    ///     // ...
    ///     # _ => {}
    /// }
    /// #   false
    /// # }
    /// ```
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct Arm {
        pub attrs: Vec<Attribute>,
        pub pat: Pat,
        pub guard: Option<(Token![if], Box<Expr>)>,
        pub fat_arrow_token: Token![=>],
        pub body: Box<Expr>,
        pub comma: Option<Token![,]>,
    }
}

#[cfg(feature = "full")]
ast_enum! {
    /// Limit types of a range, inclusive or exclusive.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub enum RangeLimits {
        /// Inclusive at the beginning, exclusive at the end.
        HalfOpen(Token![..]),
        /// Inclusive at the beginning and end.
        Closed(Token![..=]),
    }
}

#[cfg(any(feature = "parsing", feature = "printing"))]
#[cfg(feature = "full")]
pub(crate) fn requires_terminator(expr: &Expr) -> bool {
    // see https://github.com/rust-lang/rust/blob/2679c38fc/src/librustc_ast/util/classify.rs#L7-L25
    match *expr {
        Expr::Unsafe(..)
        | Expr::Block(..)
        | Expr::If(..)
        | Expr::Match(..)
        | Expr::While(..)
        | Expr::Loop(..)
        | Expr::ForLoop(..)
        | Expr::Async(..)
        | Expr::TryBlock(..) => false,
        _ => true,
    }
}

#[cfg(feature = "parsing")]
pub(crate) mod parsing {
    use super::*;
    #[cfg(feature = "full")]
    use crate::parse::ParseBuffer;
    use crate::parse::{Parse, ParseStream, Result};
    use crate::path;
    #[cfg(feature = "full")]
    use proc_macro2::TokenTree;
    use std::cmp::Ordering;

    crate::custom_keyword!(raw);

    // When we're parsing expressions which occur before blocks, like in an if
    // statement's condition, we cannot parse a struct literal.
    //
    // Struct literals are ambiguous in certain positions
    // https://github.com/rust-lang/rfcs/pull/92
    pub struct AllowStruct(bool);

    enum Precedence {
        Any,
        Assign,
        Range,
        Or,
        And,
        Compare,
        BitOr,
        BitXor,
        BitAnd,
        Shift,
        Arithmetic,
        Term,
        Cast,
    }

    impl Precedence {
        fn of(op: &BinOp) -> Self {
            match *op {
                BinOp::Add(_) | BinOp::Sub(_) => Precedence::Arithmetic,
                BinOp::Mul(_) | BinOp::Div(_) | BinOp::Rem(_) => Precedence::Term,
                BinOp::And(_) => Precedence::And,
                BinOp::Or(_) => Precedence::Or,
                BinOp::BitXor(_) => Precedence::BitXor,
                BinOp::BitAnd(_) => Precedence::BitAnd,
                BinOp::BitOr(_) => Precedence::BitOr,
                BinOp::Shl(_) | BinOp::Shr(_) => Precedence::Shift,
                BinOp::Eq(_)
                | BinOp::Lt(_)
                | BinOp::Le(_)
                | BinOp::Ne(_)
                | BinOp::Ge(_)
                | BinOp::Gt(_) => Precedence::Compare,
                BinOp::AddEq(_)
                | BinOp::SubEq(_)
                | BinOp::MulEq(_)
                | BinOp::DivEq(_)
                | BinOp::RemEq(_)
                | BinOp::BitXorEq(_)
                | BinOp::BitAndEq(_)
                | BinOp::BitOrEq(_)
                | BinOp::ShlEq(_)
                | BinOp::ShrEq(_) => Precedence::Assign,
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Expr {
        fn parse(input: ParseStream) -> Result<Self> {
            ambiguous_expr(input, AllowStruct(true))
        }
    }

    impl Expr {
        /// An alternative to the primary `Expr::parse` parser (from the
        /// [`Parse`] trait) for ambiguous syntactic positions in which a
        /// trailing brace should not be taken as part of the expression.
        ///
        /// Rust grammar has an ambiguity where braces sometimes turn a path
        /// expression into a struct initialization and sometimes do not. In the
        /// following code, the expression `S {}` is one expression. Presumably
        /// there is an empty struct `struct S {}` defined somewhere which it is
        /// instantiating.
        ///
        /// ```
        /// # struct S;
        /// # impl std::ops::Deref for S {
        /// #     type Target = bool;
        /// #     fn deref(&self) -> &Self::Target {
        /// #         &true
        /// #     }
        /// # }
        /// let _ = *S {};
        ///
        /// // parsed by rustc as: `*(S {})`
        /// ```
        ///
        /// We would want to parse the above using `Expr::parse` after the `=`
        /// token.
        ///
        /// But in the following, `S {}` is *not* a struct init expression.
        ///
        /// ```
        /// # const S: &bool = &true;
        /// if *S {} {}
        ///
        /// // parsed by rustc as:
        /// //
        /// //    if (*S) {
        /// //        /* empty block */
        /// //    }
        /// //    {
        /// //        /* another empty block */
        /// //    }
        /// ```
        ///
        /// For that reason we would want to parse if-conditions using
        /// `Expr::parse_without_eager_brace` after the `if` token. Same for
        /// similar syntactic positions such as the condition expr after a
        /// `while` token or the expr at the top of a `match`.
        ///
        /// The Rust grammar's choices around which way this ambiguity is
        /// resolved at various syntactic positions is fairly arbitrary. Really
        /// either parse behavior could work in most positions, and language
        /// designers just decide each case based on which is more likely to be
        /// what the programmer had in mind most of the time.
        ///
        /// ```
        /// # struct S;
        /// # fn doc() -> S {
        /// if return S {} {}
        /// # unreachable!()
        /// # }
        ///
        /// // parsed by rustc as:
        /// //
        /// //    if (return (S {})) {
        /// //    }
        /// //
        /// // but could equally well have been this other arbitrary choice:
        /// //
        /// //    if (return S) {
        /// //    }
        /// //    {}
        /// ```
        ///
        /// Note the grammar ambiguity on trailing braces is distinct from
        /// precedence and is not captured by assigning a precedence level to
        /// the braced struct init expr in relation to other operators. This can
        /// be illustrated by `return 0..S {}` vs `match 0..S {}`. The former
        /// parses as `return (0..(S {}))` implying tighter precedence for
        /// struct init than `..`, while the latter parses as `match (0..S) {}`
        /// implying tighter precedence for `..` than struct init, a
        /// contradiction.
        #[cfg(feature = "full")]
        #[cfg_attr(doc_cfg, doc(cfg(all(feature = "full", feature = "parsing"))))]
        pub fn parse_without_eager_brace(input: ParseStream) -> Result<Expr> {
            ambiguous_expr(input, AllowStruct(false))
        }
    }

    impl Copy for AllowStruct {}

    impl Clone for AllowStruct {
        fn clone(&self) -> Self {
            *self
        }
    }

    impl Copy for Precedence {}

    impl Clone for Precedence {
        fn clone(&self) -> Self {
            *self
        }
    }

    impl PartialEq for Precedence {
        fn eq(&self, other: &Self) -> bool {
            *self as u8 == *other as u8
        }
    }

    impl PartialOrd for Precedence {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            let this = *self as u8;
            let other = *other as u8;
            Some(this.cmp(&other))
        }
    }

    #[cfg(feature = "full")]
    fn parse_expr(
        input: ParseStream,
        mut lhs: Expr,
        allow_struct: AllowStruct,
        base: Precedence,
    ) -> Result<Expr> {
        loop {
            if input
                .fork()
                .parse::<BinOp>()
                .ok()
                .map_or(false, |op| Precedence::of(&op) >= base)
            {
                let op: BinOp = input.parse()?;
                let precedence = Precedence::of(&op);
                let mut rhs = unary_expr(input, allow_struct)?;
                loop {
                    let next = peek_precedence(input);
                    if next > precedence || next == precedence && precedence == Precedence::Assign {
                        rhs = parse_expr(input, rhs, allow_struct, next)?;
                    } else {
                        break;
                    }
                }
                lhs = if precedence == Precedence::Assign {
                    Expr::AssignOp(ExprAssignOp {
                        attrs: Vec::new(),
                        left: Box::new(lhs),
                        op,
                        right: Box::new(rhs),
                    })
                } else {
                    Expr::Binary(ExprBinary {
                        attrs: Vec::new(),
                        left: Box::new(lhs),
                        op,
                        right: Box::new(rhs),
                    })
                };
            } else if Precedence::Assign >= base
                && input.peek(Token![=])
                && !input.peek(Token![==])
                && !input.peek(Token![=>])
            {
                let eq_token: Token![=] = input.parse()?;
                let mut rhs = unary_expr(input, allow_struct)?;
                loop {
                    let next = peek_precedence(input);
                    if next >= Precedence::Assign {
                        rhs = parse_expr(input, rhs, allow_struct, next)?;
                    } else {
                        break;
                    }
                }
                lhs = Expr::Assign(ExprAssign {
                    attrs: Vec::new(),
                    left: Box::new(lhs),
                    eq_token,
                    right: Box::new(rhs),
                });
            } else if Precedence::Range >= base && input.peek(Token![..]) {
                let limits: RangeLimits = input.parse()?;
                let rhs = if input.is_empty()
                    || input.peek(Token![,])
                    || input.peek(Token![;])
                    || input.peek(Token![.]) && !input.peek(Token![..])
                    || !allow_struct.0 && input.peek(token::Brace)
                {
                    None
                } else {
                    let mut rhs = unary_expr(input, allow_struct)?;
                    loop {
                        let next = peek_precedence(input);
                        if next > Precedence::Range {
                            rhs = parse_expr(input, rhs, allow_struct, next)?;
                        } else {
                            break;
                        }
                    }
                    Some(rhs)
                };
                lhs = Expr::Range(ExprRange {
                    attrs: Vec::new(),
                    from: Some(Box::new(lhs)),
                    limits,
                    to: rhs.map(Box::new),
                });
            } else if Precedence::Cast >= base && input.peek(Token![as]) {
                let as_token: Token![as] = input.parse()?;
                let allow_plus = false;
                let allow_group_generic = false;
                let ty = ty::parsing::ambig_ty(input, allow_plus, allow_group_generic)?;
                check_cast(input)?;
                lhs = Expr::Cast(ExprCast {
                    attrs: Vec::new(),
                    expr: Box::new(lhs),
                    as_token,
                    ty: Box::new(ty),
                });
            } else if Precedence::Cast >= base && input.peek(Token![:]) && !input.peek(Token![::]) {
                let colon_token: Token![:] = input.parse()?;
                let allow_plus = false;
                let allow_group_generic = false;
                let ty = ty::parsing::ambig_ty(input, allow_plus, allow_group_generic)?;
                check_cast(input)?;
                lhs = Expr::Type(ExprType {
                    attrs: Vec::new(),
                    expr: Box::new(lhs),
                    colon_token,
                    ty: Box::new(ty),
                });
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    #[cfg(not(feature = "full"))]
    fn parse_expr(
        input: ParseStream,
        mut lhs: Expr,
        allow_struct: AllowStruct,
        base: Precedence,
    ) -> Result<Expr> {
        loop {
            if input
                .fork()
                .parse::<BinOp>()
                .ok()
                .map_or(false, |op| Precedence::of(&op) >= base)
            {
                let op: BinOp = input.parse()?;
                let precedence = Precedence::of(&op);
                let mut rhs = unary_expr(input, allow_struct)?;
                loop {
                    let next = peek_precedence(input);
                    if next > precedence || next == precedence && precedence == Precedence::Assign {
                        rhs = parse_expr(input, rhs, allow_struct, next)?;
                    } else {
                        break;
                    }
                }
                lhs = Expr::Binary(ExprBinary {
                    attrs: Vec::new(),
                    left: Box::new(lhs),
                    op,
                    right: Box::new(rhs),
                });
            } else if Precedence::Cast >= base && input.peek(Token![as]) {
                let as_token: Token![as] = input.parse()?;
                let allow_plus = false;
                let allow_group_generic = false;
                let ty = ty::parsing::ambig_ty(input, allow_plus, allow_group_generic)?;
                check_cast(input)?;
                lhs = Expr::Cast(ExprCast {
                    attrs: Vec::new(),
                    expr: Box::new(lhs),
                    as_token,
                    ty: Box::new(ty),
                });
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn peek_precedence(input: ParseStream) -> Precedence {
        if let Ok(op) = input.fork().parse() {
            Precedence::of(&op)
        } else if input.peek(Token![=]) && !input.peek(Token![=>]) {
            Precedence::Assign
        } else if input.peek(Token![..]) {
            Precedence::Range
        } else if input.peek(Token![as])
            || cfg!(feature = "full") && input.peek(Token![:]) && !input.peek(Token![::])
        {
            Precedence::Cast
        } else {
            Precedence::Any
        }
    }

    // Parse an arbitrary expression.
    fn ambiguous_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        let lhs = unary_expr(input, allow_struct)?;
        parse_expr(input, lhs, allow_struct, Precedence::Any)
    }

    #[cfg(feature = "full")]
    fn expr_attrs(input: ParseStream) -> Result<Vec<Attribute>> {
        let mut attrs = Vec::new();
        loop {
            if input.peek(token::Group) {
                let ahead = input.fork();
                let group = crate::group::parse_group(&ahead)?;
                if !group.content.peek(Token![#]) || group.content.peek2(Token![!]) {
                    break;
                }
                let attr = group.content.call(attr::parsing::single_parse_outer)?;
                if !group.content.is_empty() {
                    break;
                }
                attrs.push(attr);
            } else if input.peek(Token![#]) {
                attrs.push(input.call(attr::parsing::single_parse_outer)?);
            } else {
                break;
            }
        }
        Ok(attrs)
    }

    // <UnOp> <trailer>
    // & <trailer>
    // &mut <trailer>
    // box <trailer>
    #[cfg(feature = "full")]
    fn unary_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        let begin = input.fork();
        let attrs = input.call(expr_attrs)?;
        if input.peek(Token![&]) {
            let and_token: Token![&] = input.parse()?;
            let raw: Option<raw> =
                if input.peek(raw) && (input.peek2(Token![mut]) || input.peek2(Token![const])) {
                    Some(input.parse()?)
                } else {
                    None
                };
            let mutability: Option<Token![mut]> = input.parse()?;
            if raw.is_some() && mutability.is_none() {
                input.parse::<Token![const]>()?;
            }
            let expr = Box::new(unary_expr(input, allow_struct)?);
            if raw.is_some() {
                Ok(Expr::Verbatim(verbatim::between(begin, input)))
            } else {
                Ok(Expr::Reference(ExprReference {
                    attrs,
                    and_token,
                    raw: Reserved::default(),
                    mutability,
                    expr,
                }))
            }
        } else if input.peek(Token![box]) {
            expr_box(input, attrs, allow_struct).map(Expr::Box)
        } else if input.peek(Token![*]) || input.peek(Token![!]) || input.peek(Token![-]) {
            expr_unary(input, attrs, allow_struct).map(Expr::Unary)
        } else {
            trailer_expr(begin, attrs, input, allow_struct)
        }
    }

    #[cfg(not(feature = "full"))]
    fn unary_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        if input.peek(Token![*]) || input.peek(Token![!]) || input.peek(Token![-]) {
            Ok(Expr::Unary(ExprUnary {
                attrs: Vec::new(),
                op: input.parse()?,
                expr: Box::new(unary_expr(input, allow_struct)?),
            }))
        } else {
            trailer_expr(input, allow_struct)
        }
    }

    // <atom> (..<args>) ...
    // <atom> . <ident> (..<args>) ...
    // <atom> . <ident> ...
    // <atom> . <lit> ...
    // <atom> [ <expr> ] ...
    // <atom> ? ...
    #[cfg(feature = "full")]
    fn trailer_expr(
        begin: ParseBuffer,
        mut attrs: Vec<Attribute>,
        input: ParseStream,
        allow_struct: AllowStruct,
    ) -> Result<Expr> {
        let atom = atom_expr(input, allow_struct)?;
        let mut e = trailer_helper(input, atom)?;

        if let Expr::Verbatim(tokens) = &mut e {
            *tokens = verbatim::between(begin, input);
        } else {
            let inner_attrs = e.replace_attrs(Vec::new());
            attrs.extend(inner_attrs);
            e.replace_attrs(attrs);
        }

        Ok(e)
    }

    #[cfg(feature = "full")]
    fn trailer_helper(input: ParseStream, mut e: Expr) -> Result<Expr> {
        loop {
            if input.peek(token::Paren) {
                let content;
                e = Expr::Call(ExprCall {
                    attrs: Vec::new(),
                    func: Box::new(e),
                    paren_token: parenthesized!(content in input),
                    args: content.parse_terminated(Expr::parse)?,
                });
            } else if input.peek(Token![.])
                && !input.peek(Token![..])
                && match e {
                    Expr::Range(_) => false,
                    _ => true,
                }
            {
                let mut dot_token: Token![.] = input.parse()?;

                let await_token: Option<token::Await> = input.parse()?;
                if let Some(await_token) = await_token {
                    e = Expr::Await(ExprAwait {
                        attrs: Vec::new(),
                        base: Box::new(e),
                        dot_token,
                        await_token,
                    });
                    continue;
                }

                let float_token: Option<LitFloat> = input.parse()?;
                if let Some(float_token) = float_token {
                    if multi_index(&mut e, &mut dot_token, float_token)? {
                        continue;
                    }
                }

                let member: Member = input.parse()?;
                let turbofish = if member.is_named() && input.peek(Token![::]) {
                    Some(input.parse::<MethodTurbofish>()?)
                } else {
                    None
                };

                if turbofish.is_some() || input.peek(token::Paren) {
                    if let Member::Named(method) = member {
                        let content;
                        e = Expr::MethodCall(ExprMethodCall {
                            attrs: Vec::new(),
                            receiver: Box::new(e),
                            dot_token,
                            method,
                            turbofish,
                            paren_token: parenthesized!(content in input),
                            args: content.parse_terminated(Expr::parse)?,
                        });
                        continue;
                    }
                }

                e = Expr::Field(ExprField {
                    attrs: Vec::new(),
                    base: Box::new(e),
                    dot_token,
                    member,
                });
            } else if input.peek(token::Bracket) {
                let content;
                e = Expr::Index(ExprIndex {
                    attrs: Vec::new(),
                    expr: Box::new(e),
                    bracket_token: bracketed!(content in input),
                    index: content.parse()?,
                });
            } else if input.peek(Token![?]) {
                e = Expr::Try(ExprTry {
                    attrs: Vec::new(),
                    expr: Box::new(e),
                    question_token: input.parse()?,
                });
            } else {
                break;
            }
        }
        Ok(e)
    }

    #[cfg(not(feature = "full"))]
    fn trailer_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        let mut e = atom_expr(input, allow_struct)?;

        loop {
            if input.peek(token::Paren) {
                let content;
                e = Expr::Call(ExprCall {
                    attrs: Vec::new(),
                    func: Box::new(e),
                    paren_token: parenthesized!(content in input),
                    args: content.parse_terminated(Expr::parse)?,
                });
            } else if input.peek(Token![.]) && !input.peek(Token![..]) && !input.peek2(token::Await)
            {
                let mut dot_token: Token![.] = input.parse()?;
                let float_token: Option<LitFloat> = input.parse()?;
                if let Some(float_token) = float_token {
                    if multi_index(&mut e, &mut dot_token, float_token)? {
                        continue;
                    }
                }
                e = Expr::Field(ExprField {
                    attrs: Vec::new(),
                    base: Box::new(e),
                    dot_token,
                    member: input.parse()?,
                });
            } else if input.peek(token::Bracket) {
                let content;
                e = Expr::Index(ExprIndex {
                    attrs: Vec::new(),
                    expr: Box::new(e),
                    bracket_token: bracketed!(content in input),
                    index: content.parse()?,
                });
            } else {
                break;
            }
        }

        Ok(e)
    }

    // Parse all atomic expressions which don't have to worry about precedence
    // interactions, as they are fully contained.
    #[cfg(feature = "full")]
    fn atom_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        if input.peek(token::Group)
            && !input.peek2(Token![::])
            && !input.peek2(Token![!])
            && !input.peek2(token::Brace)
        {
            input.call(expr_group).map(Expr::Group)
        } else if input.peek(Lit) {
            input.parse().map(Expr::Lit)
        } else if input.peek(Token![async])
            && (input.peek2(token::Brace) || input.peek2(Token![move]) && input.peek3(token::Brace))
        {
            input.parse().map(Expr::Async)
        } else if input.peek(Token![try]) && input.peek2(token::Brace) {
            input.parse().map(Expr::TryBlock)
        } else if input.peek(Token![|])
            || input.peek(Token![async]) && (input.peek2(Token![|]) || input.peek2(Token![move]))
            || input.peek(Token![static])
            || input.peek(Token![move])
        {
            expr_closure(input, allow_struct).map(Expr::Closure)
        } else if input.peek(Token![for])
            && input.peek2(Token![<])
            && (input.peek3(Lifetime) || input.peek3(Token![>]))
        {
            let begin = input.fork();
            input.parse::<BoundLifetimes>()?;
            expr_closure(input, allow_struct)?;
            let verbatim = verbatim::between(begin, input);
            Ok(Expr::Verbatim(verbatim))
        } else if input.peek(Ident)
            || input.peek(Token![::])
            || input.peek(Token![<])
            || input.peek(Token![self])
            || input.peek(Token![Self])
            || input.peek(Token![super])
            || input.peek(Token![crate])
        {
            path_or_macro_or_struct(input, allow_struct)
        } else if input.peek(token::Paren) {
            paren_or_tuple(input)
        } else if input.peek(Token![break]) {
            expr_break(input, allow_struct).map(Expr::Break)
        } else if input.peek(Token![continue]) {
            input.parse().map(Expr::Continue)
        } else if input.peek(Token![return]) {
            expr_ret(input, allow_struct).map(Expr::Return)
        } else if input.peek(token::Bracket) {
            array_or_repeat(input)
        } else if input.peek(Token![let]) {
            input.parse().map(Expr::Let)
        } else if input.peek(Token![if]) {
            input.parse().map(Expr::If)
        } else if input.peek(Token![while]) {
            input.parse().map(Expr::While)
        } else if input.peek(Token![for]) {
            input.parse().map(Expr::ForLoop)
        } else if input.peek(Token![loop]) {
            input.parse().map(Expr::Loop)
        } else if input.peek(Token![match]) {
            input.parse().map(Expr::Match)
        } else if input.peek(Token![yield]) {
            input.parse().map(Expr::Yield)
        } else if input.peek(Token![unsafe]) {
            input.parse().map(Expr::Unsafe)
        } else if input.peek(Token![const]) {
            input.call(expr_const).map(Expr::Verbatim)
        } else if input.peek(token::Brace) {
            input.parse().map(Expr::Block)
        } else if input.peek(Token![..]) {
            expr_range(input, allow_struct).map(Expr::Range)
        } else if input.peek(Token![_]) {
            Ok(Expr::Verbatim(TokenStream::from(
                input.parse::<TokenTree>()?,
            )))
        } else if input.peek(Lifetime) {
            let the_label: Label = input.parse()?;
            let mut expr = if input.peek(Token![while]) {
                Expr::While(input.parse()?)
            } else if input.peek(Token![for]) {
                Expr::ForLoop(input.parse()?)
            } else if input.peek(Token![loop]) {
                Expr::Loop(input.parse()?)
            } else if input.peek(token::Brace) {
                Expr::Block(input.parse()?)
            } else {
                return Err(input.error("expected loop or block expression"));
            };
            match &mut expr {
                Expr::While(ExprWhile { label, .. })
                | Expr::ForLoop(ExprForLoop { label, .. })
                | Expr::Loop(ExprLoop { label, .. })
                | Expr::Block(ExprBlock { label, .. }) => *label = Some(the_label),
                _ => unreachable!(),
            }
            Ok(expr)
        } else {
            Err(input.error("expected expression"))
        }
    }

    #[cfg(not(feature = "full"))]
    fn atom_expr(input: ParseStream, _allow_struct: AllowStruct) -> Result<Expr> {
        if input.peek(Lit) {
            input.parse().map(Expr::Lit)
        } else if input.peek(token::Paren) {
            input.call(expr_paren).map(Expr::Paren)
        } else if input.peek(Ident)
            || input.peek(Token![::])
            || input.peek(Token![<])
            || input.peek(Token![self])
            || input.peek(Token![Self])
            || input.peek(Token![super])
            || input.peek(Token![crate])
        {
            input.parse().map(Expr::Path)
        } else {
            Err(input.error("unsupported expression; enable syn's features=[\"full\"]"))
        }
    }

    #[cfg(feature = "full")]
    fn path_or_macro_or_struct(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        let begin = input.fork();
        let expr: ExprPath = input.parse()?;

        if expr.qself.is_none() && input.peek(Token![!]) && !input.peek(Token![!=]) {
            let mut contains_arguments = false;
            for segment in &expr.path.segments {
                match segment.arguments {
                    PathArguments::None => {}
                    PathArguments::AngleBracketed(_) | PathArguments::Parenthesized(_) => {
                        contains_arguments = true;
                    }
                }
            }

            if !contains_arguments {
                let bang_token: Token![!] = input.parse()?;
                let (delimiter, tokens) = mac::parse_delimiter(input)?;
                return Ok(Expr::Macro(ExprMacro {
                    attrs: Vec::new(),
                    mac: Macro {
                        path: expr.path,
                        bang_token,
                        delimiter,
                        tokens,
                    },
                }));
            }
        }

        if allow_struct.0 && input.peek(token::Brace) {
            let expr_struct = expr_struct_helper(input, expr.path)?;
            if expr.qself.is_some() {
                Ok(Expr::Verbatim(verbatim::between(begin, input)))
            } else {
                Ok(Expr::Struct(expr_struct))
            }
        } else {
            Ok(Expr::Path(expr))
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprMacro {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ExprMacro {
                attrs: Vec::new(),
                mac: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    fn paren_or_tuple(input: ParseStream) -> Result<Expr> {
        let content;
        let paren_token = parenthesized!(content in input);
        if content.is_empty() {
            return Ok(Expr::Tuple(ExprTuple {
                attrs: Vec::new(),
                paren_token,
                elems: Punctuated::new(),
            }));
        }

        let first: Expr = content.parse()?;
        if content.is_empty() {
            return Ok(Expr::Paren(ExprParen {
                attrs: Vec::new(),
                paren_token,
                expr: Box::new(first),
            }));
        }

        let mut elems = Punctuated::new();
        elems.push_value(first);
        while !content.is_empty() {
            let punct = content.parse()?;
            elems.push_punct(punct);
            if content.is_empty() {
                break;
            }
            let value = content.parse()?;
            elems.push_value(value);
        }
        Ok(Expr::Tuple(ExprTuple {
            attrs: Vec::new(),
            paren_token,
            elems,
        }))
    }

    #[cfg(feature = "full")]
    fn array_or_repeat(input: ParseStream) -> Result<Expr> {
        let content;
        let bracket_token = bracketed!(content in input);
        if content.is_empty() {
            return Ok(Expr::Array(ExprArray {
                attrs: Vec::new(),
                bracket_token,
                elems: Punctuated::new(),
            }));
        }

        let first: Expr = content.parse()?;
        if content.is_empty() || content.peek(Token![,]) {
            let mut elems = Punctuated::new();
            elems.push_value(first);
            while !content.is_empty() {
                let punct = content.parse()?;
                elems.push_punct(punct);
                if content.is_empty() {
                    break;
                }
                let value = content.parse()?;
                elems.push_value(value);
            }
            Ok(Expr::Array(ExprArray {
                attrs: Vec::new(),
                bracket_token,
                elems,
            }))
        } else if content.peek(Token![;]) {
            let semi_token: Token![;] = content.parse()?;
            let len: Expr = content.parse()?;
            Ok(Expr::Repeat(ExprRepeat {
                attrs: Vec::new(),
                bracket_token,
                expr: Box::new(first),
                semi_token,
                len: Box::new(len),
            }))
        } else {
            Err(content.error("expected `,` or `;`"))
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprArray {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            let bracket_token = bracketed!(content in input);
            let mut elems = Punctuated::new();

            while !content.is_empty() {
                let first: Expr = content.parse()?;
                elems.push_value(first);
                if content.is_empty() {
                    break;
                }
                let punct = content.parse()?;
                elems.push_punct(punct);
            }

            Ok(ExprArray {
                attrs: Vec::new(),
                bracket_token,
                elems,
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprRepeat {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            Ok(ExprRepeat {
                bracket_token: bracketed!(content in input),
                attrs: Vec::new(),
                expr: content.parse()?,
                semi_token: content.parse()?,
                len: content.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    pub(crate) fn expr_early(input: ParseStream) -> Result<Expr> {
        let mut attrs = input.call(expr_attrs)?;
        let mut expr = if input.peek(Token![if]) {
            Expr::If(input.parse()?)
        } else if input.peek(Token![while]) {
            Expr::While(input.parse()?)
        } else if input.peek(Token![for])
            && !(input.peek2(Token![<]) && (input.peek3(Lifetime) || input.peek3(Token![>])))
        {
            Expr::ForLoop(input.parse()?)
        } else if input.peek(Token![loop]) {
            Expr::Loop(input.parse()?)
        } else if input.peek(Token![match]) {
            Expr::Match(input.parse()?)
        } else if input.peek(Token![try]) && input.peek2(token::Brace) {
            Expr::TryBlock(input.parse()?)
        } else if input.peek(Token![unsafe]) {
            Expr::Unsafe(input.parse()?)
        } else if input.peek(Token![const]) {
            Expr::Verbatim(input.call(expr_const)?)
        } else if input.peek(token::Brace) {
            Expr::Block(input.parse()?)
        } else {
            let allow_struct = AllowStruct(true);
            let mut expr = unary_expr(input, allow_struct)?;

            attrs.extend(expr.replace_attrs(Vec::new()));
            expr.replace_attrs(attrs);

            return parse_expr(input, expr, allow_struct, Precedence::Any);
        };

        if input.peek(Token![.]) && !input.peek(Token![..]) || input.peek(Token![?]) {
            expr = trailer_helper(input, expr)?;

            attrs.extend(expr.replace_attrs(Vec::new()));
            expr.replace_attrs(attrs);

            let allow_struct = AllowStruct(true);
            return parse_expr(input, expr, allow_struct, Precedence::Any);
        }

        attrs.extend(expr.replace_attrs(Vec::new()));
        expr.replace_attrs(attrs);
        Ok(expr)
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprLit {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ExprLit {
                attrs: Vec::new(),
                lit: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    fn expr_group(input: ParseStream) -> Result<ExprGroup> {
        let group = crate::group::parse_group(input)?;
        Ok(ExprGroup {
            attrs: Vec::new(),
            group_token: group.token,
            expr: group.content.parse()?,
        })
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprParen {
        fn parse(input: ParseStream) -> Result<Self> {
            expr_paren(input)
        }
    }

    fn expr_paren(input: ParseStream) -> Result<ExprParen> {
        let content;
        Ok(ExprParen {
            attrs: Vec::new(),
            paren_token: parenthesized!(content in input),
            expr: content.parse()?,
        })
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for GenericMethodArgument {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Lit) {
                let lit = input.parse()?;
                return Ok(GenericMethodArgument::Const(Expr::Lit(lit)));
            }

            if input.peek(token::Brace) {
                let block: ExprBlock = input.parse()?;
                return Ok(GenericMethodArgument::Const(Expr::Block(block)));
            }

            input.parse().map(GenericMethodArgument::Type)
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for MethodTurbofish {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(MethodTurbofish {
                colon2_token: input.parse()?,
                lt_token: input.parse()?,
                args: {
                    let mut args = Punctuated::new();
                    loop {
                        if input.peek(Token![>]) {
                            break;
                        }
                        let value: GenericMethodArgument = input.parse()?;
                        args.push_value(value);
                        if input.peek(Token![>]) {
                            break;
                        }
                        let punct = input.parse()?;
                        args.push_punct(punct);
                    }
                    args
                },
                gt_token: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprLet {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ExprLet {
                attrs: Vec::new(),
                let_token: input.parse()?,
                pat: pat::parsing::multi_pat_with_leading_vert(input)?,
                eq_token: input.parse()?,
                expr: Box::new({
                    let allow_struct = AllowStruct(false);
                    let lhs = unary_expr(input, allow_struct)?;
                    parse_expr(input, lhs, allow_struct, Precedence::Compare)?
                }),
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprIf {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            Ok(ExprIf {
                attrs,
                if_token: input.parse()?,
                cond: Box::new(input.call(Expr::parse_without_eager_brace)?),
                then_branch: input.parse()?,
                else_branch: {
                    if input.peek(Token![else]) {
                        Some(input.call(else_block)?)
                    } else {
                        None
                    }
                },
            })
        }
    }

    #[cfg(feature = "full")]
    fn else_block(input: ParseStream) -> Result<(Token![else], Box<Expr>)> {
        let else_token: Token![else] = input.parse()?;

        let lookahead = input.lookahead1();
        let else_branch = if input.peek(Token![if]) {
            input.parse().map(Expr::If)?
        } else if input.peek(token::Brace) {
            Expr::Block(ExprBlock {
                attrs: Vec::new(),
                label: None,
                block: input.parse()?,
            })
        } else {
            return Err(lookahead.error());
        };

        Ok((else_token, Box::new(else_branch)))
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprForLoop {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut attrs = input.call(Attribute::parse_outer)?;
            let label: Option<Label> = input.parse()?;
            let for_token: Token![for] = input.parse()?;

            let pat = pat::parsing::multi_pat_with_leading_vert(input)?;

            let in_token: Token![in] = input.parse()?;
            let expr: Expr = input.call(Expr::parse_without_eager_brace)?;

            let content;
            let brace_token = braced!(content in input);
            attr::parsing::parse_inner(&content, &mut attrs)?;
            let stmts = content.call(Block::parse_within)?;

            Ok(ExprForLoop {
                attrs,
                label,
                for_token,
                pat,
                in_token,
                expr: Box::new(expr),
                body: Block { brace_token, stmts },
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprLoop {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut attrs = input.call(Attribute::parse_outer)?;
            let label: Option<Label> = input.parse()?;
            let loop_token: Token![loop] = input.parse()?;

            let content;
            let brace_token = braced!(content in input);
            attr::parsing::parse_inner(&content, &mut attrs)?;
            let stmts = content.call(Block::parse_within)?;

            Ok(ExprLoop {
                attrs,
                label,
                loop_token,
                body: Block { brace_token, stmts },
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprMatch {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut attrs = input.call(Attribute::parse_outer)?;
            let match_token: Token![match] = input.parse()?;
            let expr = Expr::parse_without_eager_brace(input)?;

            let content;
            let brace_token = braced!(content in input);
            attr::parsing::parse_inner(&content, &mut attrs)?;

            let mut arms = Vec::new();
            while !content.is_empty() {
                arms.push(content.call(Arm::parse)?);
            }

            Ok(ExprMatch {
                attrs,
                match_token,
                expr: Box::new(expr),
                brace_token,
                arms,
            })
        }
    }

    macro_rules! impl_by_parsing_expr {
        (
            $(
                $expr_type:ty, $variant:ident, $msg:expr,
            )*
        ) => {
            $(
                #[cfg(all(feature = "full", feature = "printing"))]
                #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
                impl Parse for $expr_type {
                    fn parse(input: ParseStream) -> Result<Self> {
                        let mut expr: Expr = input.parse()?;
                        loop {
                            match expr {
                                Expr::$variant(inner) => return Ok(inner),
                                Expr::Group(next) => expr = *next.expr,
                                _ => return Err(Error::new_spanned(expr, $msg)),
                            }
                        }
                    }
                }
            )*
        };
    }

    impl_by_parsing_expr! {
        ExprAssign, Assign, "expected assignment expression",
        ExprAssignOp, AssignOp, "expected compound assignment expression",
        ExprAwait, Await, "expected await expression",
        ExprBinary, Binary, "expected binary operation",
        ExprCall, Call, "expected function call expression",
        ExprCast, Cast, "expected cast expression",
        ExprField, Field, "expected struct field access",
        ExprIndex, Index, "expected indexing expression",
        ExprMethodCall, MethodCall, "expected method call expression",
        ExprRange, Range, "expected range expression",
        ExprTry, Try, "expected try expression",
        ExprTuple, Tuple, "expected tuple expression",
        ExprType, Type, "expected type ascription expression",
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprBox {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = Vec::new();
            let allow_struct = AllowStruct(true);
            expr_box(input, attrs, allow_struct)
        }
    }

    #[cfg(feature = "full")]
    fn expr_box(
        input: ParseStream,
        attrs: Vec<Attribute>,
        allow_struct: AllowStruct,
    ) -> Result<ExprBox> {
        Ok(ExprBox {
            attrs,
            box_token: input.parse()?,
            expr: Box::new(unary_expr(input, allow_struct)?),
        })
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprUnary {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = Vec::new();
            let allow_struct = AllowStruct(true);
            expr_unary(input, attrs, allow_struct)
        }
    }

    #[cfg(feature = "full")]
    fn expr_unary(
        input: ParseStream,
        attrs: Vec<Attribute>,
        allow_struct: AllowStruct,
    ) -> Result<ExprUnary> {
        Ok(ExprUnary {
            attrs,
            op: input.parse()?,
            expr: Box::new(unary_expr(input, allow_struct)?),
        })
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprClosure {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_struct = AllowStruct(true);
            expr_closure(input, allow_struct)
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprReference {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_struct = AllowStruct(true);
            Ok(ExprReference {
                attrs: Vec::new(),
                and_token: input.parse()?,
                raw: Reserved::default(),
                mutability: input.parse()?,
                expr: Box::new(unary_expr(input, allow_struct)?),
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprBreak {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_struct = AllowStruct(true);
            expr_break(input, allow_struct)
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprReturn {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_struct = AllowStruct(true);
            expr_ret(input, allow_struct)
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprTryBlock {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ExprTryBlock {
                attrs: Vec::new(),
                try_token: input.parse()?,
                block: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprYield {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ExprYield {
                attrs: Vec::new(),
                yield_token: input.parse()?,
                expr: {
                    if !input.is_empty() && !input.peek(Token![,]) && !input.peek(Token![;]) {
                        Some(input.parse()?)
                    } else {
                        None
                    }
                },
            })
        }
    }

    #[cfg(feature = "full")]
    fn expr_closure(input: ParseStream, allow_struct: AllowStruct) -> Result<ExprClosure> {
        let movability: Option<Token![static]> = input.parse()?;
        let asyncness: Option<Token![async]> = input.parse()?;
        let capture: Option<Token![move]> = input.parse()?;
        let or1_token: Token![|] = input.parse()?;

        let mut inputs = Punctuated::new();
        loop {
            if input.peek(Token![|]) {
                break;
            }
            let value = closure_arg(input)?;
            inputs.push_value(value);
            if input.peek(Token![|]) {
                break;
            }
            let punct: Token![,] = input.parse()?;
            inputs.push_punct(punct);
        }

        let or2_token: Token![|] = input.parse()?;

        let (output, body) = if input.peek(Token![->]) {
            let arrow_token: Token![->] = input.parse()?;
            let ty: Type = input.parse()?;
            let body: Block = input.parse()?;
            let output = ReturnType::Type(arrow_token, Box::new(ty));
            let block = Expr::Block(ExprBlock {
                attrs: Vec::new(),
                label: None,
                block: body,
            });
            (output, block)
        } else {
            let body = ambiguous_expr(input, allow_struct)?;
            (ReturnType::Default, body)
        };

        Ok(ExprClosure {
            attrs: Vec::new(),
            movability,
            asyncness,
            capture,
            or1_token,
            inputs,
            or2_token,
            output,
            body: Box::new(body),
        })
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprAsync {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ExprAsync {
                attrs: Vec::new(),
                async_token: input.parse()?,
                capture: input.parse()?,
                block: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    fn closure_arg(input: ParseStream) -> Result<Pat> {
        let attrs = input.call(Attribute::parse_outer)?;
        let mut pat: Pat = input.parse()?;

        if input.peek(Token![:]) {
            Ok(Pat::Type(PatType {
                attrs,
                pat: Box::new(pat),
                colon_token: input.parse()?,
                ty: input.parse()?,
            }))
        } else {
            match &mut pat {
                Pat::Box(pat) => pat.attrs = attrs,
                Pat::Ident(pat) => pat.attrs = attrs,
                Pat::Lit(pat) => pat.attrs = attrs,
                Pat::Macro(pat) => pat.attrs = attrs,
                Pat::Or(pat) => pat.attrs = attrs,
                Pat::Path(pat) => pat.attrs = attrs,
                Pat::Range(pat) => pat.attrs = attrs,
                Pat::Reference(pat) => pat.attrs = attrs,
                Pat::Rest(pat) => pat.attrs = attrs,
                Pat::Slice(pat) => pat.attrs = attrs,
                Pat::Struct(pat) => pat.attrs = attrs,
                Pat::Tuple(pat) => pat.attrs = attrs,
                Pat::TupleStruct(pat) => pat.attrs = attrs,
                Pat::Type(_) => unreachable!(),
                Pat::Verbatim(_) => {}
                Pat::Wild(pat) => pat.attrs = attrs,

                #[cfg(syn_no_non_exhaustive)]
                _ => unreachable!(),
            }
            Ok(pat)
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprWhile {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut attrs = input.call(Attribute::parse_outer)?;
            let label: Option<Label> = input.parse()?;
            let while_token: Token![while] = input.parse()?;
            let cond = Expr::parse_without_eager_brace(input)?;

            let content;
            let brace_token = braced!(content in input);
            attr::parsing::parse_inner(&content, &mut attrs)?;
            let stmts = content.call(Block::parse_within)?;

            Ok(ExprWhile {
                attrs,
                label,
                while_token,
                cond: Box::new(cond),
                body: Block { brace_token, stmts },
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Label {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Label {
                name: input.parse()?,
                colon_token: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Option<Label> {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Lifetime) {
                input.parse().map(Some)
            } else {
                Ok(None)
            }
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprContinue {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ExprContinue {
                attrs: Vec::new(),
                continue_token: input.parse()?,
                label: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    fn expr_break(input: ParseStream, allow_struct: AllowStruct) -> Result<ExprBreak> {
        Ok(ExprBreak {
            attrs: Vec::new(),
            break_token: input.parse()?,
            label: input.parse()?,
            expr: {
                if input.is_empty()
                    || input.peek(Token![,])
                    || input.peek(Token![;])
                    || !allow_struct.0 && input.peek(token::Brace)
                {
                    None
                } else {
                    let expr = ambiguous_expr(input, allow_struct)?;
                    Some(Box::new(expr))
                }
            },
        })
    }

    #[cfg(feature = "full")]
    fn expr_ret(input: ParseStream, allow_struct: AllowStruct) -> Result<ExprReturn> {
        Ok(ExprReturn {
            attrs: Vec::new(),
            return_token: input.parse()?,
            expr: {
                if input.is_empty() || input.peek(Token![,]) || input.peek(Token![;]) {
                    None
                } else {
                    // NOTE: return is greedy and eats blocks after it even when in a
                    // position where structs are not allowed, such as in if statement
                    // conditions. For example:
                    //
                    // if return { println!("A") } {} // Prints "A"
                    let expr = ambiguous_expr(input, allow_struct)?;
                    Some(Box::new(expr))
                }
            },
        })
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for FieldValue {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let member: Member = input.parse()?;
            let (colon_token, value) = if input.peek(Token![:]) || !member.is_named() {
                let colon_token: Token![:] = input.parse()?;
                let value: Expr = input.parse()?;
                (Some(colon_token), value)
            } else if let Member::Named(ident) = &member {
                let value = Expr::Path(ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path: Path::from(ident.clone()),
                });
                (None, value)
            } else {
                unreachable!()
            };

            Ok(FieldValue {
                attrs,
                member,
                colon_token,
                expr: value,
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprStruct {
        fn parse(input: ParseStream) -> Result<Self> {
            let path: Path = input.parse()?;
            expr_struct_helper(input, path)
        }
    }

    #[cfg(feature = "full")]
    fn expr_struct_helper(input: ParseStream, path: Path) -> Result<ExprStruct> {
        let content;
        let brace_token = braced!(content in input);

        let mut fields = Punctuated::new();
        while !content.is_empty() {
            if content.peek(Token![..]) {
                return Ok(ExprStruct {
                    attrs: Vec::new(),
                    brace_token,
                    path,
                    fields,
                    dot2_token: Some(content.parse()?),
                    rest: if content.is_empty() {
                        None
                    } else {
                        Some(Box::new(content.parse()?))
                    },
                });
            }

            fields.push(content.parse()?);
            if content.is_empty() {
                break;
            }
            let punct: Token![,] = content.parse()?;
            fields.push_punct(punct);
        }

        Ok(ExprStruct {
            attrs: Vec::new(),
            brace_token,
            path,
            fields,
            dot2_token: None,
            rest: None,
        })
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprUnsafe {
        fn parse(input: ParseStream) -> Result<Self> {
            let unsafe_token: Token![unsafe] = input.parse()?;

            let content;
            let brace_token = braced!(content in input);
            let inner_attrs = content.call(Attribute::parse_inner)?;
            let stmts = content.call(Block::parse_within)?;

            Ok(ExprUnsafe {
                attrs: inner_attrs,
                unsafe_token,
                block: Block { brace_token, stmts },
            })
        }
    }

    #[cfg(feature = "full")]
    pub(crate) fn expr_const(input: ParseStream) -> Result<TokenStream> {
        let begin = input.fork();
        input.parse::<Token![const]>()?;

        let content;
        braced!(content in input);
        content.call(Attribute::parse_inner)?;
        content.call(Block::parse_within)?;

        Ok(verbatim::between(begin, input))
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprBlock {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut attrs = input.call(Attribute::parse_outer)?;
            let label: Option<Label> = input.parse()?;

            let content;
            let brace_token = braced!(content in input);
            attr::parsing::parse_inner(&content, &mut attrs)?;
            let stmts = content.call(Block::parse_within)?;

            Ok(ExprBlock {
                attrs,
                label,
                block: Block { brace_token, stmts },
            })
        }
    }

    #[cfg(feature = "full")]
    fn expr_range(input: ParseStream, allow_struct: AllowStruct) -> Result<ExprRange> {
        Ok(ExprRange {
            attrs: Vec::new(),
            from: None,
            limits: input.parse()?,
            to: {
                if input.is_empty()
                    || input.peek(Token![,])
                    || input.peek(Token![;])
                    || input.peek(Token![.]) && !input.peek(Token![..])
                    || !allow_struct.0 && input.peek(token::Brace)
                {
                    None
                } else {
                    let to = ambiguous_expr(input, allow_struct)?;
                    Some(Box::new(to))
                }
            },
        })
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for RangeLimits {
        fn parse(input: ParseStream) -> Result<Self> {
            let lookahead = input.lookahead1();
            if lookahead.peek(Token![..=]) {
                input.parse().map(RangeLimits::Closed)
            } else if lookahead.peek(Token![...]) {
                let dot3: Token![...] = input.parse()?;
                Ok(RangeLimits::Closed(Token![..=](dot3.spans)))
            } else if lookahead.peek(Token![..]) {
                input.parse().map(RangeLimits::HalfOpen)
            } else {
                Err(lookahead.error())
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ExprPath {
        fn parse(input: ParseStream) -> Result<Self> {
            #[cfg(not(feature = "full"))]
            let attrs = Vec::new();
            #[cfg(feature = "full")]
            let attrs = input.call(Attribute::parse_outer)?;

            let (qself, path) = path::parsing::qpath(input, true)?;

            Ok(ExprPath { attrs, qself, path })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Member {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Ident) {
                input.parse().map(Member::Named)
            } else if input.peek(LitInt) {
                input.parse().map(Member::Unnamed)
            } else {
                Err(input.error("expected identifier or integer"))
            }
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Arm {
        fn parse(input: ParseStream) -> Result<Arm> {
            let requires_comma;
            Ok(Arm {
                attrs: input.call(Attribute::parse_outer)?,
                pat: pat::parsing::multi_pat_with_leading_vert(input)?,
                guard: {
                    if input.peek(Token![if]) {
                        let if_token: Token![if] = input.parse()?;
                        let guard: Expr = input.parse()?;
                        Some((if_token, Box::new(guard)))
                    } else {
                        None
                    }
                },
                fat_arrow_token: input.parse()?,
                body: {
                    let body = input.call(expr_early)?;
                    requires_comma = requires_terminator(&body);
                    Box::new(body)
                },
                comma: {
                    if requires_comma && !input.is_empty() {
                        Some(input.parse()?)
                    } else {
                        input.parse()?
                    }
                },
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Index {
        fn parse(input: ParseStream) -> Result<Self> {
            let lit: LitInt = input.parse()?;
            if lit.suffix().is_empty() {
                Ok(Index {
                    index: lit
                        .base10_digits()
                        .parse()
                        .map_err(|err| Error::new(lit.span(), err))?,
                    span: lit.span(),
                })
            } else {
                Err(Error::new(lit.span(), "expected unsuffixed integer"))
            }
        }
    }

    fn multi_index(e: &mut Expr, dot_token: &mut Token![.], float: LitFloat) -> Result<bool> {
        let mut float_repr = float.to_string();
        let trailing_dot = float_repr.ends_with('.');
        if trailing_dot {
            float_repr.truncate(float_repr.len() - 1);
        }
        for part in float_repr.split('.') {
            let index = crate::parse_str(part).map_err(|err| Error::new(float.span(), err))?;
            #[cfg(not(syn_no_const_vec_new))]
            let base = mem::replace(e, Expr::DUMMY);
            #[cfg(syn_no_const_vec_new)]
            let base = mem::replace(e, Expr::Verbatim(TokenStream::new()));
            *e = Expr::Field(ExprField {
                attrs: Vec::new(),
                base: Box::new(base),
                dot_token: Token![.](dot_token.span),
                member: Member::Unnamed(index),
            });
            *dot_token = Token![.](float.span());
        }
        Ok(!trailing_dot)
    }

    #[cfg(feature = "full")]
    impl Member {
        fn is_named(&self) -> bool {
            match *self {
                Member::Named(_) => true,
                Member::Unnamed(_) => false,
            }
        }
    }

    fn check_cast(input: ParseStream) -> Result<()> {
        let kind = if input.peek(Token![.]) && !input.peek(Token![..]) {
            if input.peek2(token::Await) {
                "`.await`"
            } else if input.peek2(Ident) && (input.peek3(token::Paren) || input.peek3(Token![::])) {
                "a method call"
            } else {
                "a field access"
            }
        } else if input.peek(Token![?]) {
            "`?`"
        } else if input.peek(token::Bracket) {
            "indexing"
        } else if input.peek(token::Paren) {
            "a function call"
        } else {
            return Ok(());
        };
        let msg = format!("casts cannot be followed by {}", kind);
        Err(input.error(msg))
    }
}

#[cfg(feature = "printing")]
pub(crate) mod printing {
    use super::*;
    #[cfg(feature = "full")]
    use crate::attr::FilterAttrs;
    use proc_macro2::{Literal, TokenStream};
    use quote::{ToTokens, TokenStreamExt};

    // If the given expression is a bare `ExprStruct`, wraps it in parenthesis
    // before appending it to `TokenStream`.
    #[cfg(feature = "full")]
    fn wrap_bare_struct(tokens: &mut TokenStream, e: &Expr) {
        if let Expr::Struct(_) = *e {
            token::Paren::default().surround(tokens, |tokens| {
                e.to_tokens(tokens);
            });
        } else {
            e.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    pub(crate) fn outer_attrs_to_tokens(attrs: &[Attribute], tokens: &mut TokenStream) {
        tokens.append_all(attrs.outer());
    }

    #[cfg(feature = "full")]
    fn inner_attrs_to_tokens(attrs: &[Attribute], tokens: &mut TokenStream) {
        tokens.append_all(attrs.inner());
    }

    #[cfg(not(feature = "full"))]
    pub(crate) fn outer_attrs_to_tokens(_attrs: &[Attribute], _tokens: &mut TokenStream) {}

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprBox {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.box_token.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprArray {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.bracket_token.surround(tokens, |tokens| {
                self.elems.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprCall {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.func.to_tokens(tokens);
            self.paren_token.surround(tokens, |tokens| {
                self.args.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprMethodCall {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.receiver.to_tokens(tokens);
            self.dot_token.to_tokens(tokens);
            self.method.to_tokens(tokens);
            self.turbofish.to_tokens(tokens);
            self.paren_token.surround(tokens, |tokens| {
                self.args.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for MethodTurbofish {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.colon2_token.to_tokens(tokens);
            self.lt_token.to_tokens(tokens);
            self.args.to_tokens(tokens);
            self.gt_token.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for GenericMethodArgument {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match self {
                GenericMethodArgument::Type(t) => t.to_tokens(tokens),
                GenericMethodArgument::Const(c) => c.to_tokens(tokens),
            }
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprTuple {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.paren_token.surround(tokens, |tokens| {
                self.elems.to_tokens(tokens);
                // If we only have one argument, we need a trailing comma to
                // distinguish ExprTuple from ExprParen.
                if self.elems.len() == 1 && !self.elems.trailing_punct() {
                    <Token![,]>::default().to_tokens(tokens);
                }
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprBinary {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.left.to_tokens(tokens);
            self.op.to_tokens(tokens);
            self.right.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprUnary {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.op.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprLit {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.lit.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprCast {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.expr.to_tokens(tokens);
            self.as_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprType {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.expr.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    fn maybe_wrap_else(tokens: &mut TokenStream, else_: &Option<(Token![else], Box<Expr>)>) {
        if let Some((else_token, else_)) = else_ {
            else_token.to_tokens(tokens);

            // If we are not one of the valid expressions to exist in an else
            // clause, wrap ourselves in a block.
            match **else_ {
                Expr::If(_) | Expr::Block(_) => {
                    else_.to_tokens(tokens);
                }
                _ => {
                    token::Brace::default().surround(tokens, |tokens| {
                        else_.to_tokens(tokens);
                    });
                }
            }
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprLet {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.let_token.to_tokens(tokens);
            self.pat.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.expr);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprIf {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.if_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.cond);
            self.then_branch.to_tokens(tokens);
            maybe_wrap_else(tokens, &self.else_branch);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprWhile {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.label.to_tokens(tokens);
            self.while_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.cond);
            self.body.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.body.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprForLoop {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.label.to_tokens(tokens);
            self.for_token.to_tokens(tokens);
            self.pat.to_tokens(tokens);
            self.in_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.expr);
            self.body.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.body.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprLoop {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.label.to_tokens(tokens);
            self.loop_token.to_tokens(tokens);
            self.body.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.body.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprMatch {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.match_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.expr);
            self.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                for (i, arm) in self.arms.iter().enumerate() {
                    arm.to_tokens(tokens);
                    // Ensure that we have a comma after a non-block arm, except
                    // for the last one.
                    let is_last = i == self.arms.len() - 1;
                    if !is_last && requires_terminator(&arm.body) && arm.comma.is_none() {
                        <Token![,]>::default().to_tokens(tokens);
                    }
                }
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprAsync {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.async_token.to_tokens(tokens);
            self.capture.to_tokens(tokens);
            self.block.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprAwait {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.base.to_tokens(tokens);
            self.dot_token.to_tokens(tokens);
            self.await_token.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprTryBlock {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.try_token.to_tokens(tokens);
            self.block.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprYield {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.yield_token.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprClosure {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.movability.to_tokens(tokens);
            self.asyncness.to_tokens(tokens);
            self.capture.to_tokens(tokens);
            self.or1_token.to_tokens(tokens);
            self.inputs.to_tokens(tokens);
            self.or2_token.to_tokens(tokens);
            self.output.to_tokens(tokens);
            self.body.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprUnsafe {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.unsafe_token.to_tokens(tokens);
            self.block.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.block.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprBlock {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.label.to_tokens(tokens);
            self.block.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.block.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprAssign {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.left.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.right.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprAssignOp {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.left.to_tokens(tokens);
            self.op.to_tokens(tokens);
            self.right.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprField {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.base.to_tokens(tokens);
            self.dot_token.to_tokens(tokens);
            self.member.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Member {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match self {
                Member::Named(ident) => ident.to_tokens(tokens),
                Member::Unnamed(index) => index.to_tokens(tokens),
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Index {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let mut lit = Literal::i64_unsuffixed(i64::from(self.index));
            lit.set_span(self.span);
            tokens.append(lit);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprIndex {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.expr.to_tokens(tokens);
            self.bracket_token.surround(tokens, |tokens| {
                self.index.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for RangeLimits {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match self {
                RangeLimits::HalfOpen(t) => t.to_tokens(tokens),
                RangeLimits::Closed(t) => t.to_tokens(tokens),
            }
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprRange {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.from.to_tokens(tokens);
            self.limits.to_tokens(tokens);
            self.to.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprPath {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            path::printing::print_path(tokens, &self.qself, &self.path);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprReference {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.and_token.to_tokens(tokens);
            self.mutability.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprBreak {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.break_token.to_tokens(tokens);
            self.label.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprContinue {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.continue_token.to_tokens(tokens);
            self.label.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprReturn {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.return_token.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprMacro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.mac.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprStruct {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.path.to_tokens(tokens);
            self.brace_token.surround(tokens, |tokens| {
                self.fields.to_tokens(tokens);
                if let Some(dot2_token) = &self.dot2_token {
                    dot2_token.to_tokens(tokens);
                } else if self.rest.is_some() {
                    Token![..](Span::call_site()).to_tokens(tokens);
                }
                self.rest.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprRepeat {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.bracket_token.surround(tokens, |tokens| {
                self.expr.to_tokens(tokens);
                self.semi_token.to_tokens(tokens);
                self.len.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprGroup {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.group_token.surround(tokens, |tokens| {
                self.expr.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprParen {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.paren_token.surround(tokens, |tokens| {
                self.expr.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ExprTry {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.expr.to_tokens(tokens);
            self.question_token.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Label {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.name.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for FieldValue {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.member.to_tokens(tokens);
            if let Some(colon_token) = &self.colon_token {
                colon_token.to_tokens(tokens);
                self.expr.to_tokens(tokens);
            }
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Arm {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(&self.attrs);
            self.pat.to_tokens(tokens);
            if let Some((if_token, guard)) = &self.guard {
                if_token.to_tokens(tokens);
                guard.to_tokens(tokens);
            }
            self.fat_arrow_token.to_tokens(tokens);
            self.body.to_tokens(tokens);
            self.comma.to_tokens(tokens);
        }
    }
}
