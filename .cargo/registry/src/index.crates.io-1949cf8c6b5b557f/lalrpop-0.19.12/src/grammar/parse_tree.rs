//! The "parse-tree" is what is produced by the parser. We use it do
//! some pre-expansion and so forth before creating the proper AST.

use crate::grammar::consts::{INPUT_LIFETIME, LALR, RECURSIVE_ASCENT, TABLE_DRIVEN, TEST_ALL};
use crate::grammar::pattern::Pattern;
use crate::grammar::repr::{self as r, NominalTypeRepr, TypeRepr};
use crate::lexer::dfa::DFA;
use crate::message::builder::InlineBuilder;
use crate::message::Content;
use crate::tls::Tls;
use crate::util::Sep;
use std::fmt::{Debug, Display, Error, Formatter};
use string_cache::DefaultAtom as Atom;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grammar {
    // see field `prefix` in `grammar::repr::Grammar`
    pub prefix: String,
    pub span: Span,
    pub type_parameters: Vec<TypeParameter>,
    pub parameters: Vec<Parameter>,
    pub where_clauses: Vec<WhereClause<TypeRef>>,
    pub items: Vec<GrammarItem>,
    pub annotations: Vec<Annotation>,
    pub module_attributes: Vec<String>,
}

#[derive(Copy, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span(pub usize, pub usize);

impl Into<Box<dyn Content>> for Span {
    fn into(self) -> Box<dyn Content> {
        let file_text = Tls::file_text();
        let string = file_text.span_str(self);

        // Insert an Adjacent block to prevent wrapping inside this
        // string:
        InlineBuilder::new()
            .begin_adjacent()
            .text(string)
            .end()
            .end()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrammarItem {
    MatchToken(MatchToken),
    ExternToken(ExternToken),
    InternToken(InternToken),
    Nonterminal(NonterminalData),
    Use(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchToken {
    pub contents: Vec<MatchContents>,
    pub span: Span,
}

impl MatchToken {
    pub fn new(contents: MatchContents, span: Span) -> MatchToken {
        MatchToken {
            contents: vec![contents],
            span,
        }
    }

    // Not really sure if this is the best way to do it
    pub fn add(self, contents: MatchContents) -> MatchToken {
        let mut new_contents = self.contents.clone();
        new_contents.push(contents);
        MatchToken {
            contents: new_contents,
            span: self.span,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchContents {
    pub items: Vec<MatchItem>,
}

// FIXME: Validate that MatchSymbol is actually a TerminalString::Literal
//          and that MatchMapping is an Id or String
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchItem {
    CatchAll(Span),
    Unmapped(MatchSymbol, Span),
    Mapped(MatchSymbol, MatchMapping, Span),
}

impl MatchItem {
    pub fn is_catch_all(&self) -> bool {
        match *self {
            MatchItem::CatchAll(_) => true,
            _ => false,
        }
    }

    pub fn span(&self) -> Span {
        match *self {
            MatchItem::CatchAll(span) => span,
            MatchItem::Unmapped(_, span) => span,
            MatchItem::Mapped(_, _, span) => span,
        }
    }
}

pub type MatchSymbol = TerminalLiteral;

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd)]
pub enum MatchMapping {
    Terminal(TerminalString),
    Skip,
}

impl Debug for MatchMapping {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match self {
            MatchMapping::Terminal(term) => write!(fmt, "{:?}", term),
            MatchMapping::Skip => write!(fmt, "{{ }}"),
        }
    }
}
impl Display for MatchMapping {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match self {
            MatchMapping::Terminal(term) => write!(fmt, "{}", term),
            MatchMapping::Skip => write!(fmt, "{{ }}"),
        }
    }
}

/// Intern tokens are not typed by the user: they are synthesized in
/// the absence of an "extern" declaration with information about the
/// string literals etc that appear in the grammar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InternToken {
    /// Set of `r"foo"` and `"foo"` literals extracted from the
    /// grammar. Sorted by order of increasing precedence.
    pub match_entries: Vec<MatchEntry>,
    pub dfa: DFA,
}

/// In `token_check`, as we prepare to generate a tokenizer, we
/// combine any `match` declaration the user may have given with the
/// set of literals (e.g. `"foo"` or `r"[a-z]"`) that appear elsewhere
/// in their in the grammar to produce a series of `MatchEntry`. Each
/// `MatchEntry` roughly corresponds to one line in a `match` declaration.
///
/// So e.g. if you had
///
/// ```
/// match {
///    r"(?i)BEGIN" => "BEGIN",
///    "+" => "+",
/// } else {
///    _
/// }
///
/// ID = r"[a-zA-Z]+"
/// ```
///
/// This would correspond to three match entries:
/// - `MatchEntry { match_literal: r"(?i)BEGIN", user_name: "BEGIN", precedence: 2 }`
/// - `MatchEntry { match_literal: "+", user_name: "+", precedence: 3 }`
/// - `MatchEntry { match_literal: "r[a-zA-Z]+"", user_name: r"[a-zA-Z]+", precedence: 0 }`
///
/// A couple of things to note:
///
/// - Literals appearing in the grammar are converting into an "identity" mapping
/// - Each match group G is combined with the implicit priority IP of 1 for literals and 0 for
///   regex to yield the final precedence; the formula is `G*2 + IP`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MatchEntry {
    /// The precedence of this match entry.
    ///
    /// NB: This field must go first, so that `PartialOrd` sorts by precedence first!
    pub precedence: usize,
    pub match_literal: TerminalLiteral,
    pub user_name: MatchMapping,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternToken {
    pub span: Span,
    pub associated_types: Vec<AssociatedType>,
    pub enum_token: Option<EnumToken>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssociatedType {
    pub type_span: Span,
    pub type_name: Atom,
    pub type_ref: TypeRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumToken {
    pub type_name: TypeRef,
    pub type_span: Span,
    pub conversions: Vec<Conversion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversion {
    pub span: Span,
    pub from: TerminalString,
    pub to: Pattern<TypeRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    pub absolute: bool,
    pub ids: Vec<Atom>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeRef {
    // (T1, T2)
    Tuple(Vec<TypeRef>),

    // [T]
    Slice(Box<TypeRef>),

    // Foo<'a, 'b, T1, T2>, Foo::Bar, etc
    Nominal {
        path: Path,
        types: Vec<TypeRef>,
    },

    Ref {
        lifetime: Option<Lifetime>,
        mutable: bool,
        referent: Box<TypeRef>,
    },

    // `dyn Trait`
    TraitObject {
        path: Path,
        types: Vec<TypeRef>,
    },

    // 'x ==> only should appear within nominal types, but what do we care
    Lifetime(Lifetime),

    // Foo or Bar ==> treated specially since macros may care
    Id(Atom),

    // <N> ==> type of a nonterminal, emitted by macro expansion
    OfSymbol(SymbolKind),

    Fn {
        forall: Vec<TypeParameter>,
        path: Path,
        parameters: Vec<TypeRef>,
        ret: Option<Box<TypeRef>>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WhereClause<T> {
    // 'a: 'b + 'c
    Lifetime {
        lifetime: Lifetime,
        bounds: Vec<Lifetime>,
    },
    // where for<'a> &'a T: Debug + Into<usize>
    Type {
        forall: Vec<TypeParameter>,
        ty: T,
        bounds: Vec<TypeBound<T>>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeBound<T> {
    // The `'a` in `T: 'a`.
    Lifetime(Lifetime),

    // `for<'a> FnMut(&'a usize)`
    Fn {
        forall: Vec<TypeParameter>,
        path: Path,
        parameters: Vec<T>,
        ret: Option<T>,
    },

    // `some::Trait` or `some::Trait<Param, ...>` or `some::Trait<Item = Assoc>`
    // or `for<'a> Trait<'a, T>`
    Trait {
        forall: Vec<TypeParameter>,
        path: Path,
        parameters: Vec<TypeBoundParameter<T>>,
    },
}

impl<T> TypeBound<T> {
    pub fn map<F, U>(&self, mut f: F) -> TypeBound<U>
    where
        F: FnMut(&T) -> U,
    {
        match *self {
            TypeBound::Lifetime(ref l) => TypeBound::Lifetime(l.clone()),
            TypeBound::Fn {
                ref forall,
                ref path,
                ref parameters,
                ref ret,
            } => TypeBound::Fn {
                forall: forall.clone(),
                path: path.clone(),
                parameters: parameters.iter().map(&mut f).collect(),
                ret: ret.as_ref().map(f),
            },
            TypeBound::Trait {
                ref forall,
                ref path,
                ref parameters,
            } => TypeBound::Trait {
                forall: forall.clone(),
                path: path.clone(),
                parameters: parameters.iter().map(|p| p.map(&mut f)).collect(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeBoundParameter<T> {
    // 'a
    Lifetime(Lifetime),
    // `T` or `'a`
    TypeParameter(T),
    // `Item = T`
    Associated(Atom, T),
}

impl<T> TypeBoundParameter<T> {
    pub fn map<F, U>(&self, mut f: F) -> TypeBoundParameter<U>
    where
        F: FnMut(&T) -> U,
    {
        match *self {
            TypeBoundParameter::Lifetime(ref l) => TypeBoundParameter::Lifetime(l.clone()),
            TypeBoundParameter::TypeParameter(ref t) => TypeBoundParameter::TypeParameter(f(t)),
            TypeBoundParameter::Associated(ref id, ref t) => {
                TypeBoundParameter::Associated(id.clone(), f(t))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TypeParameter {
    Lifetime(Lifetime),
    Id(Atom),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    pub name: Atom,
    pub ty: TypeRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Visibility {
    Pub(Option<Path>),
    PubIn(Path),
    Priv,
}

impl Visibility {
    pub fn is_pub(&self) -> bool {
        match *self {
            Visibility::Pub(_) => true,
            Visibility::PubIn(_) => true,
            Visibility::Priv => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonterminalData {
    pub visibility: Visibility,
    pub name: NonterminalString,
    pub annotations: Vec<Annotation>,
    pub span: Span,
    pub args: Vec<NonterminalString>, // macro arguments
    pub type_decl: Option<TypeRef>,
    pub alternatives: Vec<Alternative>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub id_span: Span,
    pub id: Atom,
    pub arg: Option<(Atom, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alternative {
    pub span: Span,

    pub expr: ExprSymbol,

    // if C, only legal in macros
    pub condition: Option<Condition>,

    // => { code }
    pub action: Option<ActionKind>,

    pub annotations: Vec<Annotation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionKind {
    User(String),
    Fallible(String),
    Lookahead,
    Lookbehind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub span: Span,
    pub lhs: NonterminalString, // X
    pub rhs: Atom,              // "Foo"
    pub op: ConditionOp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionOp {
    // X == "Foo", equality
    Equals,

    // X != "Foo", inequality
    NotEquals,

    // X ~~ "Foo", regexp match
    Match,

    // X !~ "Foo", regexp non-match
    NotMatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub span: Span,
    pub kind: SymbolKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    // (X Y)
    Expr(ExprSymbol),

    // foo, before name resolution
    AmbiguousId(Atom),

    // "foo" and foo (after name resolution)
    Terminal(TerminalString),

    // foo, after name resolution
    Nonterminal(NonterminalString),

    // foo<..>
    Macro(MacroSymbol),

    // X+, X?, X*
    Repeat(Box<RepeatSymbol>),

    // <X>
    Choose(Box<Symbol>),

    // <x:X> or <mut x:X>
    Name(Name, Box<Symbol>),

    // @L
    Lookahead,

    // @R
    Lookbehind,

    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub mutable: bool,
    pub name: Atom,
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalString {
    Literal(TerminalLiteral),
    Bare(Atom),
    Error,
}

impl TerminalString {
    pub fn as_literal(&self) -> Option<TerminalLiteral> {
        match *self {
            TerminalString::Literal(ref l) => Some(l.clone()),
            _ => None,
        }
    }

    pub fn display_len(&self) -> usize {
        match *self {
            TerminalString::Literal(ref x) => x.display_len(),
            TerminalString::Bare(ref x) => x.len(),
            TerminalString::Error => "error".len(),
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalLiteral {
    Quoted(Atom),
    Regex(Atom),
}

impl TerminalLiteral {
    /// The *base precedence* is the precedence within a `match { }`
    /// block level. It indicates that quoted things like `"foo"` get
    /// precedence over regex matches.
    pub fn base_precedence(&self) -> usize {
        match *self {
            TerminalLiteral::Quoted(_) => 1,
            TerminalLiteral::Regex(_) => 0,
        }
    }

    pub fn display_len(&self) -> usize {
        match *self {
            TerminalLiteral::Quoted(ref x) => x.len(),
            TerminalLiteral::Regex(ref x) => x.len() + "####r".len(),
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonterminalString(pub Atom);

impl NonterminalString {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl Into<Box<dyn Content>> for NonterminalString {
    fn into(self) -> Box<dyn Content> {
        let session = Tls::session();

        InlineBuilder::new()
            .text(self)
            .styled(session.nonterminal_symbol)
            .end()
    }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lifetime(pub Atom);

impl Lifetime {
    pub fn anonymous() -> Self {
        Lifetime(Atom::from("'_"))
    }

    pub fn is_anonymous(&self) -> bool {
        *self == Self::anonymous()
    }

    pub fn statik() -> Self {
        Lifetime(Atom::from("'static"))
    }

    pub fn input() -> Self {
        Lifetime(Atom::from(INPUT_LIFETIME))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RepeatOp {
    Star,
    Plus,
    Question,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepeatSymbol {
    pub op: RepeatOp,
    pub symbol: Symbol,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExprSymbol {
    pub symbols: Vec<Symbol>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroSymbol {
    pub name: NonterminalString,
    pub args: Vec<Symbol>,
}

impl TerminalString {
    pub fn quoted(i: Atom) -> TerminalString {
        TerminalString::Literal(TerminalLiteral::Quoted(i))
    }

    pub fn regex(i: Atom) -> TerminalString {
        TerminalString::Literal(TerminalLiteral::Regex(i))
    }
}

impl Into<Box<dyn Content>> for TerminalString {
    fn into(self) -> Box<dyn Content> {
        let session = Tls::session();
        InlineBuilder::new()
            .text(self)
            .styled(session.terminal_symbol)
            .end()
    }
}

impl Grammar {
    pub fn extern_token(&self) -> Option<&ExternToken> {
        self.items
            .iter()
            .flat_map(GrammarItem::as_extern_token)
            .next()
    }

    pub fn enum_token(&self) -> Option<&EnumToken> {
        self.items
            .iter()
            .flat_map(GrammarItem::as_extern_token)
            .flat_map(|et| et.enum_token.as_ref())
            .next()
    }

    pub fn intern_token(&self) -> Option<&InternToken> {
        self.items
            .iter()
            .flat_map(GrammarItem::as_intern_token)
            .next()
    }

    pub fn match_token(&self) -> Option<&MatchToken> {
        self.items
            .iter()
            .flat_map(GrammarItem::as_match_token)
            .next()
    }
}

impl GrammarItem {
    pub fn is_macro_def(&self) -> bool {
        match *self {
            GrammarItem::Nonterminal(ref d) => d.is_macro_def(),
            _ => false,
        }
    }

    pub fn as_nonterminal(&self) -> Option<&NonterminalData> {
        match *self {
            GrammarItem::Nonterminal(ref d) => Some(d),
            GrammarItem::Use(..) => None,
            GrammarItem::MatchToken(..) => None,
            GrammarItem::ExternToken(..) => None,
            GrammarItem::InternToken(..) => None,
        }
    }

    pub fn as_match_token(&self) -> Option<&MatchToken> {
        match *self {
            GrammarItem::Nonterminal(..) => None,
            GrammarItem::Use(..) => None,
            GrammarItem::MatchToken(ref d) => Some(d),
            GrammarItem::ExternToken(..) => None,
            GrammarItem::InternToken(..) => None,
        }
    }

    pub fn as_extern_token(&self) -> Option<&ExternToken> {
        match *self {
            GrammarItem::Nonterminal(..) => None,
            GrammarItem::Use(..) => None,
            GrammarItem::MatchToken(..) => None,
            GrammarItem::ExternToken(ref d) => Some(d),
            GrammarItem::InternToken(..) => None,
        }
    }

    pub fn as_intern_token(&self) -> Option<&InternToken> {
        match *self {
            GrammarItem::Nonterminal(..) => None,
            GrammarItem::Use(..) => None,
            GrammarItem::MatchToken(..) => None,
            GrammarItem::ExternToken(..) => None,
            GrammarItem::InternToken(ref d) => Some(d),
        }
    }
}

impl NonterminalData {
    pub fn is_macro_def(&self) -> bool {
        !self.args.is_empty()
    }
}

impl Symbol {
    pub fn new(span: Span, kind: SymbolKind) -> Symbol {
        Symbol { span, kind }
    }

    pub fn canonical_form(&self) -> String {
        format!("{}", self)
    }
}

impl Name {
    pub fn new(mutable: bool, name: Atom) -> Self {
        Name { mutable, name }
    }

    pub fn immut(name: Atom) -> Self {
        Name::new(false, name)
    }
}

impl Display for Visibility {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            Visibility::Pub(Some(ref path)) => write!(fmt, "pub({}) ", path),
            Visibility::Pub(None) => write!(fmt, "pub "),
            Visibility::PubIn(ref path) => write!(fmt, "pub(in {}) ", path),
            Visibility::Priv => Ok(()),
        }
    }
}

impl<T: Display> Display for WhereClause<T> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            WhereClause::Lifetime {
                ref lifetime,
                ref bounds,
            } => {
                write!(fmt, "{}:", lifetime)?;
                for (i, b) in bounds.iter().enumerate() {
                    if i != 0 {
                        write!(fmt, " +")?;
                    }
                    write!(fmt, " {}", b)?;
                }
                Ok(())
            }
            WhereClause::Type {
                ref forall,
                ref ty,
                ref bounds,
            } => {
                if !forall.is_empty() {
                    write!(fmt, "for<")?;
                    for (i, l) in forall.iter().enumerate() {
                        if i != 0 {
                            write!(fmt, ", ")?;
                        }
                        write!(fmt, "{}", l)?;
                    }
                    write!(fmt, "> ")?;
                }

                write!(fmt, "{}: ", ty)?;
                for (i, b) in bounds.iter().enumerate() {
                    if i != 0 {
                        write!(fmt, " +")?;
                    }
                    write!(fmt, " {}", b)?;
                }
                Ok(())
            }
        }
    }
}

impl<T: Display> Display for TypeBound<T> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            TypeBound::Lifetime(ref l) => write!(fmt, "{}", l),
            TypeBound::Fn {
                ref forall,
                ref path,
                ref parameters,
                ref ret,
            } => {
                if !forall.is_empty() {
                    write!(fmt, "for<")?;
                    for (i, l) in forall.iter().enumerate() {
                        if i != 0 {
                            write!(fmt, ", ")?;
                        }
                        write!(fmt, "{}", l)?;
                    }
                    write!(fmt, "> ")?;
                }

                write!(fmt, "{}(", path)?;
                for (i, p) in parameters.iter().enumerate() {
                    if i != 0 {
                        write!(fmt, ", ")?;
                    }
                    write!(fmt, "{}", p)?;
                }
                write!(fmt, ")")?;

                if let Some(ref ret) = *ret {
                    write!(fmt, " -> {}", ret)?;
                }

                Ok(())
            }
            TypeBound::Trait {
                ref forall,
                ref path,
                ref parameters,
            } => {
                if !forall.is_empty() {
                    write!(fmt, "for<")?;
                    for (i, l) in forall.iter().enumerate() {
                        if i != 0 {
                            write!(fmt, ", ")?;
                        }
                        write!(fmt, "{}", l)?;
                    }
                    write!(fmt, "> ")?;
                }

                write!(fmt, "{}", path)?;
                if parameters.is_empty() {
                    return Ok(());
                }

                write!(fmt, "<")?;
                for (i, p) in parameters.iter().enumerate() {
                    if i != 0 {
                        write!(fmt, ", ")?;
                    }
                    write!(fmt, "{}", p)?;
                }
                write!(fmt, ">")
            }
        }
    }
}

impl<T: Display> Display for TypeBoundParameter<T> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            TypeBoundParameter::Lifetime(ref l) => write!(fmt, "{}", l),
            TypeBoundParameter::TypeParameter(ref t) => write!(fmt, "{}", t),
            TypeBoundParameter::Associated(ref id, ref t) => write!(fmt, "{} = {}", id, t),
        }
    }
}

impl Display for TerminalString {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            TerminalString::Literal(ref s) => write!(fmt, "{}", s),
            TerminalString::Bare(ref s) => write!(fmt, "{}", s),
            TerminalString::Error => write!(fmt, "error"),
        }
    }
}

impl Debug for TerminalString {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        Display::fmt(self, fmt)
    }
}

impl Display for Lifetime {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        Display::fmt(&self.0, fmt)
    }
}

impl Debug for Lifetime {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        Display::fmt(self, fmt)
    }
}

impl Display for TerminalLiteral {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            TerminalLiteral::Quoted(ref s) => write!(fmt, "{:?}", s.as_ref()), // the Debug impl adds the `"` and escaping
            TerminalLiteral::Regex(ref s) => write!(fmt, "r#{:?}#", s.as_ref()), // FIXME -- need to determine proper number of #
        }
    }
}

impl Debug for TerminalLiteral {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}", self)
    }
}

impl Display for Path {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(
            fmt,
            "{}{}",
            if self.absolute { "::" } else { "" },
            Sep("::", &self.ids),
        )
    }
}

impl Display for NonterminalString {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}", self.0)
    }
}

impl Debug for NonterminalString {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        Display::fmt(self, fmt)
    }
}

impl Display for Symbol {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        Display::fmt(&self.kind, fmt)
    }
}

impl Display for SymbolKind {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            SymbolKind::Expr(ref expr) => write!(fmt, "{}", expr),
            SymbolKind::Terminal(ref s) => write!(fmt, "{}", s),
            SymbolKind::Nonterminal(ref s) => write!(fmt, "{}", s),
            SymbolKind::AmbiguousId(ref s) => write!(fmt, "{}", s),
            SymbolKind::Macro(ref m) => write!(fmt, "{}", m),
            SymbolKind::Repeat(ref r) => write!(fmt, "{}", r),
            SymbolKind::Choose(ref s) => write!(fmt, "<{}>", s),
            SymbolKind::Name(ref n, ref s) => write!(fmt, "{}:{}", n, s),
            SymbolKind::Lookahead => write!(fmt, "@L"),
            SymbolKind::Lookbehind => write!(fmt, "@R"),
            SymbolKind::Error => write!(fmt, "error"),
        }
    }
}

impl Display for Name {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        if self.mutable {
            write!(fmt, "mut {}", self.name)
        } else {
            Display::fmt(&self.name, fmt)
        }
    }
}

impl Display for RepeatSymbol {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}{}", self.symbol, self.op)
    }
}

impl Display for RepeatOp {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            RepeatOp::Plus => write!(fmt, "+"),
            RepeatOp::Star => write!(fmt, "*"),
            RepeatOp::Question => write!(fmt, "?"),
        }
    }
}

impl Display for ExprSymbol {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "({})", Sep(" ", &self.symbols))
    }
}

impl ExternToken {
    pub fn associated_type(&self, name: Atom) -> Option<&AssociatedType> {
        self.associated_types.iter().find(|a| a.type_name == name)
    }
}

impl ExprSymbol {
    pub fn canonical_form(&self) -> String {
        format!("{}", self)
    }
}

impl MacroSymbol {
    pub fn canonical_form(&self) -> String {
        format!("{}", self)
    }
}

impl RepeatSymbol {
    pub fn canonical_form(&self) -> String {
        format!("{}", self)
    }
}

impl Display for MacroSymbol {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}<{}>", self.name, Sep(", ", &self.args))
    }
}

impl Display for TypeParameter {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            TypeParameter::Lifetime(ref s) => write!(fmt, "{}", s),
            TypeParameter::Id(ref s) => write!(fmt, "{}", s),
        }
    }
}

impl Display for TypeRef {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            TypeRef::Tuple(ref types) => write!(fmt, "({})", Sep(", ", types)),
            TypeRef::Slice(ref ty) => write!(fmt, "[{}]", ty),
            TypeRef::Nominal {
                ref path,
                ref types,
            } if types.is_empty() => write!(fmt, "{}", path),
            TypeRef::Nominal {
                ref path,
                ref types,
            } => write!(fmt, "{}<{}>", path, Sep(", ", types)),
            TypeRef::TraitObject {
                ref path,
                ref types,
            } if types.is_empty() => write!(fmt, "dyn {}", path),
            TypeRef::TraitObject {
                ref path,
                ref types,
            } => write!(fmt, "dyn {}<{}>", path, Sep(", ", types)),
            TypeRef::Lifetime(ref s) => write!(fmt, "{}", s),
            TypeRef::Id(ref s) => write!(fmt, "{}", s),
            TypeRef::OfSymbol(ref s) => write!(fmt, "`{}`", s),
            TypeRef::Ref {
                lifetime: None,
                mutable: false,
                ref referent,
            } => write!(fmt, "&{}", referent),
            TypeRef::Ref {
                lifetime: Some(ref l),
                mutable: false,
                ref referent,
            } => write!(fmt, "&{} {}", l, referent),
            TypeRef::Ref {
                lifetime: None,
                mutable: true,
                ref referent,
            } => write!(fmt, "&mut {}", referent),
            TypeRef::Ref {
                lifetime: Some(ref l),
                mutable: true,
                ref referent,
            } => write!(fmt, "&{} mut {}", l, referent),
            TypeRef::Fn {
                ref forall,
                ref path,
                ref parameters,
                ref ret,
            } => {
                write!(fmt, "dyn ")?;
                if !forall.is_empty() {
                    write!(fmt, "for<{}> ", Sep(", ", forall),)?;
                }
                write!(fmt, "{}({})", path, Sep(", ", parameters))?;
                if let Some(ret) = ret {
                    write!(fmt, " -> {}", ret)?;
                }
                Ok(())
            }
        }
    }
}

impl TypeRef {
    // Converts a TypeRef to a TypeRepr, assuming no inference is
    // required etc. This is safe for all types a user can directly
    // type, but not safe for the result of expanding macros.
    pub fn type_repr(&self) -> TypeRepr {
        match *self {
            TypeRef::Tuple(ref types) => {
                TypeRepr::Tuple(types.iter().map(TypeRef::type_repr).collect())
            }
            TypeRef::Slice(ref ty) => TypeRepr::Slice(Box::new(ty.type_repr())),
            TypeRef::Nominal {
                ref path,
                ref types,
            } => TypeRepr::Nominal(NominalTypeRepr {
                path: path.clone(),
                types: types.iter().map(TypeRef::type_repr).collect(),
            }),
            TypeRef::Lifetime(ref id) => TypeRepr::Lifetime(id.clone()),
            TypeRef::Id(ref id) => TypeRepr::Nominal(NominalTypeRepr {
                path: Path::from_id(id.clone()),
                types: vec![],
            }),
            TypeRef::OfSymbol(_) => unreachable!("OfSymbol produced by parser"),
            TypeRef::Ref {
                ref lifetime,
                mutable,
                ref referent,
            } => TypeRepr::Ref {
                lifetime: lifetime.clone(),
                mutable,
                referent: Box::new(referent.type_repr()),
            },
            TypeRef::TraitObject {
                ref path,
                ref types,
            } => TypeRepr::TraitObject(NominalTypeRepr {
                path: path.clone(),
                types: types.iter().map(TypeRef::type_repr).collect(),
            }),
            TypeRef::Fn {
                ref forall,
                ref path,
                ref parameters,
                ref ret,
            } => TypeRepr::Fn {
                forall: forall.clone(),
                path: path.clone(),
                parameters: parameters.iter().map(TypeRef::type_repr).collect(),
                ret: ret.as_ref().map(|t| Box::new(TypeRef::type_repr(t))),
            },
        }
    }
}

impl Path {
    pub fn from_id(id: Atom) -> Path {
        Path {
            absolute: false,
            ids: vec![id],
        }
    }

    pub fn usize() -> Path {
        Path {
            absolute: false,
            ids: vec![Atom::from("usize")],
        }
    }

    pub fn str() -> Path {
        Path {
            absolute: false,
            ids: vec![Atom::from("str")],
        }
    }

    pub fn vec() -> Path {
        Path {
            absolute: false,
            ids: vec![Atom::from("alloc"), Atom::from("vec"), Atom::from("Vec")],
        }
    }

    pub fn option() -> Path {
        Path {
            absolute: false,
            ids: vec![
                Atom::from("core"),
                Atom::from("option"),
                Atom::from("Option"),
            ],
        }
    }

    pub fn as_id(&self) -> Option<Atom> {
        if !self.absolute && self.ids.len() == 1 {
            Some(self.ids[0].clone())
        } else {
            None
        }
    }
}

pub fn read_algorithm(annotations: &[Annotation], algorithm: &mut r::Algorithm) {
    for annotation in annotations {
        if annotation.id == *LALR {
            algorithm.lalr = true;
        } else if annotation.id == *TABLE_DRIVEN {
            algorithm.codegen = r::LrCodeGeneration::TableDriven;
        } else if annotation.id == *RECURSIVE_ASCENT {
            algorithm.codegen = r::LrCodeGeneration::RecursiveAscent;
        } else if annotation.id == *TEST_ALL {
            algorithm.codegen = r::LrCodeGeneration::TestAll;
        } else {
            panic!(
                "validation permitted unknown annotation: {:?}",
                annotation.id,
            );
        }
    }
}
