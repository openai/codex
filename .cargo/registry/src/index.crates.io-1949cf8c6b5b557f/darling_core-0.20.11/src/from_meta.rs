use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::btree_map::BTreeMap;
use std::collections::hash_map::HashMap;
use std::collections::HashSet;
use std::hash::BuildHasher;
use std::num;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use syn::{Expr, Lit, Meta};

use crate::ast::NestedMeta;
use crate::util::path_to_string;
use crate::{Error, Result};

/// Create an instance from an item in an attribute declaration.
///
/// # Implementing `FromMeta`
/// * Do not take a dependency on the `ident` of the passed-in meta item. The ident will be set by the field name of the containing struct.
/// * Implement only the `from_*` methods that you intend to support. The default implementations will return useful errors.
///
/// # Provided Implementations
/// ## bool
///
/// * Word with no value specified - becomes `true`.
/// * As a boolean literal, e.g. `foo = true`.
/// * As a string literal, e.g. `foo = "true"`.
///
/// ## char
/// * As a char literal, e.g. `foo = '#'`.
/// * As a string literal consisting of a single character, e.g. `foo = "#"`.
///
/// ## String
/// * As a string literal, e.g. `foo = "hello"`.
/// * As a raw string literal, e.g. `foo = r#"hello "world""#`.
///
/// ## Number
/// * As a string literal, e.g. `foo = "-25"`.
/// * As an unquoted positive value, e.g. `foo = 404`. Negative numbers must be in quotation marks.
///
/// ## ()
/// * Word with no value specified, e.g. `foo`. This is best used with `Option`.
///   See `darling::util::Flag` for a more strongly-typed alternative.
///
/// ## Option
/// * Any format produces `Some`.
///
/// ## `Result<T, darling::Error>`
/// * Allows for fallible parsing; will populate the target field with the result of the
///   parse attempt.
pub trait FromMeta: Sized {
    fn from_nested_meta(item: &NestedMeta) -> Result<Self> {
        (match *item {
            NestedMeta::Lit(ref lit) => Self::from_value(lit),
            NestedMeta::Meta(ref mi) => Self::from_meta(mi),
        })
        .map_err(|e| e.with_span(item))
    }

    /// Create an instance from a `syn::Meta` by dispatching to the format-appropriate
    /// trait function. This generally should not be overridden by implementers.
    ///
    /// # Error Spans
    /// If this method is overridden and can introduce errors that weren't passed up from
    /// other `from_meta` calls, the override must call `with_span` on the error using the
    /// `item` to make sure that the emitted diagnostic points to the correct location in
    /// source code.
    fn from_meta(item: &Meta) -> Result<Self> {
        (match *item {
            Meta::Path(_) => Self::from_word(),
            Meta::List(ref value) => {
                Self::from_list(&NestedMeta::parse_meta_list(value.tokens.clone())?[..])
            }
            Meta::NameValue(ref value) => Self::from_expr(&value.value),
        })
        .map_err(|e| e.with_span(item))
    }

    /// When a field is omitted from a parent meta-item, `from_none` is used to attempt
    /// recovery before a missing field error is generated.
    ///
    /// **Most types should not override this method.** `darling` already allows field-level
    /// missing-field recovery using `#[darling(default)]` and `#[darling(default = "...")]`,
    /// and users who add a `String` field to their `FromMeta`-deriving struct would be surprised
    /// if they get back `""` instead of a missing field error when that field is omitted.
    ///
    /// The primary use-case for this is `Option<T>` fields gracefully handlling absence without
    /// needing `#[darling(default)]`.
    fn from_none() -> Option<Self> {
        None
    }

    /// Create an instance from the presence of the word in the attribute with no
    /// additional options specified.
    fn from_word() -> Result<Self> {
        Err(Error::unsupported_format("word"))
    }

    /// Create an instance from a list of nested meta items.
    #[allow(unused_variables)]
    fn from_list(items: &[NestedMeta]) -> Result<Self> {
        Err(Error::unsupported_format("list"))
    }

    /// Create an instance from a literal value of either `foo = "bar"` or `foo("bar")`.
    /// This dispatches to the appropriate method based on the type of literal encountered,
    /// and generally should not be overridden by implementers.
    ///
    /// # Error Spans
    /// If this method is overridden, the override must make sure to add `value`'s span
    /// information to the returned error by calling `with_span(value)` on the `Error` instance.
    fn from_value(value: &Lit) -> Result<Self> {
        (match *value {
            Lit::Bool(ref b) => Self::from_bool(b.value),
            Lit::Str(ref s) => Self::from_string(&s.value()),
            Lit::Char(ref ch) => Self::from_char(ch.value()),
            _ => Err(Error::unexpected_lit_type(value)),
        })
        .map_err(|e| e.with_span(value))
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        match *expr {
            Expr::Lit(ref lit) => Self::from_value(&lit.lit),
            Expr::Group(ref group) => {
                // syn may generate this invisible group delimiter when the input to the darling
                // proc macro (specifically, the attributes) are generated by a
                // macro_rules! (e.g. propagating a macro_rules!'s expr)
                // Since we want to basically ignore these invisible group delimiters,
                // we just propagate the call to the inner expression.
                Self::from_expr(&group.expr)
            }
            _ => Err(Error::unexpected_expr_type(expr)),
        }
        .map_err(|e| e.with_span(expr))
    }

    /// Create an instance from a char literal in a value position.
    #[allow(unused_variables)]
    fn from_char(value: char) -> Result<Self> {
        Err(Error::unexpected_type("char"))
    }

    /// Create an instance from a string literal in a value position.
    #[allow(unused_variables)]
    fn from_string(value: &str) -> Result<Self> {
        Err(Error::unexpected_type("string"))
    }

    /// Create an instance from a bool literal in a value position.
    #[allow(unused_variables)]
    fn from_bool(value: bool) -> Result<Self> {
        Err(Error::unexpected_type("bool"))
    }
}

// FromMeta impls for std and syn types.

impl FromMeta for () {
    fn from_word() -> Result<Self> {
        Ok(())
    }
}

impl FromMeta for bool {
    fn from_word() -> Result<Self> {
        Ok(true)
    }

    #[allow(clippy::wrong_self_convention)] // false positive
    fn from_bool(value: bool) -> Result<Self> {
        Ok(value)
    }

    fn from_string(value: &str) -> Result<Self> {
        value.parse().map_err(|_| Error::unknown_value(value))
    }
}

impl FromMeta for AtomicBool {
    fn from_meta(mi: &Meta) -> Result<Self> {
        FromMeta::from_meta(mi)
            .map(AtomicBool::new)
            .map_err(|e| e.with_span(mi))
    }
}

impl FromMeta for char {
    #[allow(clippy::wrong_self_convention)] // false positive
    fn from_char(value: char) -> Result<Self> {
        Ok(value)
    }

    fn from_string(s: &str) -> Result<Self> {
        let mut chars = s.chars();
        let char1 = chars.next();
        let char2 = chars.next();

        if let (Some(char), None) = (char1, char2) {
            Ok(char)
        } else {
            Err(Error::unexpected_type("string"))
        }
    }
}

impl FromMeta for String {
    fn from_string(s: &str) -> Result<Self> {
        Ok(s.to_string())
    }
}

impl FromMeta for std::path::PathBuf {
    fn from_string(s: &str) -> Result<Self> {
        Ok(s.into())
    }
}

/// Generate an impl of `FromMeta` that will accept strings which parse to numbers or
/// integer literals.
macro_rules! from_meta_num {
    ($ty:path) => {
        impl FromMeta for $ty {
            fn from_string(s: &str) -> Result<Self> {
                s.parse().map_err(|_| Error::unknown_value(s))
            }

            fn from_value(value: &Lit) -> Result<Self> {
                (match *value {
                    Lit::Str(ref s) => Self::from_string(&s.value()),
                    Lit::Int(ref s) => s.base10_parse::<$ty>().map_err(Error::from),
                    _ => Err(Error::unexpected_lit_type(value)),
                })
                .map_err(|e| e.with_span(value))
            }
        }
    };
}

from_meta_num!(u8);
from_meta_num!(u16);
from_meta_num!(u32);
from_meta_num!(u64);
from_meta_num!(u128);
from_meta_num!(usize);
from_meta_num!(i8);
from_meta_num!(i16);
from_meta_num!(i32);
from_meta_num!(i64);
from_meta_num!(i128);
from_meta_num!(isize);
from_meta_num!(num::NonZeroU8);
from_meta_num!(num::NonZeroU16);
from_meta_num!(num::NonZeroU32);
from_meta_num!(num::NonZeroU64);
from_meta_num!(num::NonZeroU128);
from_meta_num!(num::NonZeroUsize);
from_meta_num!(num::NonZeroI8);
from_meta_num!(num::NonZeroI16);
from_meta_num!(num::NonZeroI32);
from_meta_num!(num::NonZeroI64);
from_meta_num!(num::NonZeroI128);
from_meta_num!(num::NonZeroIsize);

/// Generate an impl of `FromMeta` that will accept strings which parse to floats or
/// float literals.
macro_rules! from_meta_float {
    ($ty:ident) => {
        impl FromMeta for $ty {
            fn from_string(s: &str) -> Result<Self> {
                s.parse().map_err(|_| Error::unknown_value(s))
            }

            fn from_value(value: &Lit) -> Result<Self> {
                (match *value {
                    Lit::Str(ref s) => Self::from_string(&s.value()),
                    Lit::Float(ref s) => s.base10_parse::<$ty>().map_err(Error::from),
                    _ => Err(Error::unexpected_lit_type(value)),
                })
                .map_err(|e| e.with_span(value))
            }
        }
    };
}

from_meta_float!(f32);
from_meta_float!(f64);

/// Parsing support for punctuated. This attempts to preserve span information
/// when available, but also supports parsing strings with the call site as the
/// emitted span.
impl<T: syn::parse::Parse, P: syn::parse::Parse> FromMeta for syn::punctuated::Punctuated<T, P> {
    fn from_value(value: &Lit) -> Result<Self> {
        if let Lit::Str(ref ident) = *value {
            ident
                .parse_with(syn::punctuated::Punctuated::parse_terminated)
                .map_err(|_| Error::unknown_lit_str_value(ident))
        } else {
            Err(Error::unexpected_lit_type(value))
        }
    }
}

/// Support for arbitrary expressions as values in a meta item.
///
/// For backwards-compatibility to versions of `darling` based on `syn` 1,
/// string literals will be "unwrapped" and their contents will be parsed
/// as an expression.
///
/// See [`util::parse_expr`](crate::util::parse_expr) for functions to provide
/// alternate parsing modes for this type.
impl FromMeta for syn::Expr {
    fn from_expr(expr: &Expr) -> Result<Self> {
        match expr {
            Expr::Lit(syn::ExprLit {
                lit: lit @ syn::Lit::Str(_),
                ..
            }) => Self::from_value(lit),
            Expr::Group(group) => Self::from_expr(&group.expr), // see FromMeta::from_expr
            _ => Ok(expr.clone()),
        }
    }

    fn from_string(value: &str) -> Result<Self> {
        syn::parse_str(value).map_err(|_| Error::unknown_value(value))
    }

    fn from_value(value: &::syn::Lit) -> Result<Self> {
        if let ::syn::Lit::Str(ref v) = *value {
            v.parse::<syn::Expr>()
                .map_err(|_| Error::unknown_lit_str_value(v))
        } else {
            Err(Error::unexpected_lit_type(value))
        }
    }
}

/// Parser for paths that supports both quote-wrapped and bare values.
impl FromMeta for syn::Path {
    fn from_string(value: &str) -> Result<Self> {
        syn::parse_str(value).map_err(|_| Error::unknown_value(value))
    }

    fn from_value(value: &::syn::Lit) -> Result<Self> {
        if let ::syn::Lit::Str(ref v) = *value {
            v.parse().map_err(|_| Error::unknown_lit_str_value(v))
        } else {
            Err(Error::unexpected_lit_type(value))
        }
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        match expr {
            Expr::Lit(lit) => Self::from_value(&lit.lit),
            Expr::Path(path) => Ok(path.path.clone()),
            Expr::Group(group) => Self::from_expr(&group.expr), // see FromMeta::from_expr
            _ => Err(Error::unexpected_expr_type(expr)),
        }
    }
}

impl FromMeta for syn::Ident {
    fn from_string(value: &str) -> Result<Self> {
        syn::parse_str(value).map_err(|_| Error::unknown_value(value))
    }

    fn from_value(value: &syn::Lit) -> Result<Self> {
        if let syn::Lit::Str(ref v) = *value {
            v.parse().map_err(|_| Error::unknown_lit_str_value(v))
        } else {
            Err(Error::unexpected_lit_type(value))
        }
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        match expr {
            Expr::Lit(lit) => Self::from_value(&lit.lit),
            // All idents are paths, but not all paths are idents -
            // the get_ident() method does additional validation to
            // make sure the path is actually an ident.
            Expr::Path(path) => match path.path.get_ident() {
                Some(ident) => Ok(ident.clone()),
                None => Err(Error::unexpected_expr_type(expr)),
            },
            Expr::Group(group) => Self::from_expr(&group.expr), // see FromMeta::from_expr
            _ => Err(Error::unexpected_expr_type(expr)),
        }
    }
}

/// Adapter for various expression types.
///
/// Prior to syn 2.0, darling supported arbitrary expressions as long as they
/// were wrapped in quotation marks. This was helpful for people writing
/// libraries that needed expressions, but it now creates an ambiguity when
/// parsing a meta item.
///
/// To address this, the macro supports both formats; if it cannot parse the
/// item as an expression of the right type and the passed-in expression is
/// a string literal, it will fall back to parsing the string contents.
macro_rules! from_syn_expr_type {
    ($ty:path, $variant:ident) => {
        impl FromMeta for $ty {
            fn from_expr(expr: &syn::Expr) -> Result<Self> {
                match expr {
                    syn::Expr::$variant(body) => Ok(body.clone()),
                    syn::Expr::Lit(expr_lit) => Self::from_value(&expr_lit.lit),
                    syn::Expr::Group(group) => Self::from_expr(&group.expr), // see FromMeta::from_expr
                    _ => Err(Error::unexpected_expr_type(expr)),
                }
            }

            fn from_value(value: &::syn::Lit) -> Result<Self> {
                if let syn::Lit::Str(body) = &value {
                    body.parse::<$ty>()
                        .map_err(|_| Error::unknown_lit_str_value(body))
                } else {
                    Err(Error::unexpected_lit_type(value))
                }
            }
        }
    };
}

from_syn_expr_type!(syn::ExprArray, Array);
from_syn_expr_type!(syn::ExprPath, Path);
from_syn_expr_type!(syn::ExprRange, Range);

/// Adapter from `syn::parse::Parse` to `FromMeta` for items that cannot
/// be expressed in a [`syn::MetaNameValue`].
///
/// This cannot be a blanket impl, due to the `syn::Lit` family's need to handle non-string values.
/// Therefore, we use a macro and a lot of impls.
macro_rules! from_syn_parse {
    ($ty:path) => {
        impl FromMeta for $ty {
            fn from_string(value: &str) -> Result<Self> {
                syn::parse_str(value).map_err(|_| Error::unknown_value(value))
            }

            fn from_value(value: &::syn::Lit) -> Result<Self> {
                if let ::syn::Lit::Str(ref v) = *value {
                    v.parse::<$ty>()
                        .map_err(|_| Error::unknown_lit_str_value(v))
                } else {
                    Err(Error::unexpected_lit_type(value))
                }
            }
        }
    };
}

from_syn_parse!(syn::Type);
from_syn_parse!(syn::TypeArray);
from_syn_parse!(syn::TypeBareFn);
from_syn_parse!(syn::TypeGroup);
from_syn_parse!(syn::TypeImplTrait);
from_syn_parse!(syn::TypeInfer);
from_syn_parse!(syn::TypeMacro);
from_syn_parse!(syn::TypeNever);
from_syn_parse!(syn::TypeParam);
from_syn_parse!(syn::TypeParen);
from_syn_parse!(syn::TypePath);
from_syn_parse!(syn::TypePtr);
from_syn_parse!(syn::TypeReference);
from_syn_parse!(syn::TypeSlice);
from_syn_parse!(syn::TypeTraitObject);
from_syn_parse!(syn::TypeTuple);
from_syn_parse!(syn::Visibility);
from_syn_parse!(syn::WhereClause);

macro_rules! from_numeric_array {
    ($ty:ident) => {
        /// Parsing an unsigned integer array, i.e. `example = "[1, 2, 3, 4]"`.
        impl FromMeta for Vec<$ty> {
            fn from_expr(expr: &syn::Expr) -> Result<Self> {
                match expr {
                    syn::Expr::Array(expr_array) => expr_array
                        .elems
                        .iter()
                        .map(|expr| {
                            let unexpected = || {
                                Error::custom("Expected array of unsigned integers").with_span(expr)
                            };
                            match expr {
                                Expr::Lit(lit) => $ty::from_value(&lit.lit),
                                Expr::Group(group) => match &*group.expr {
                                    Expr::Lit(lit) => $ty::from_value(&lit.lit),
                                    _ => Err(unexpected()),
                                },
                                _ => Err(unexpected()),
                            }
                        })
                        .collect::<Result<Vec<$ty>>>(),
                    syn::Expr::Lit(expr_lit) => Self::from_value(&expr_lit.lit),
                    syn::Expr::Group(group) => Self::from_expr(&group.expr), // see FromMeta::from_expr
                    _ => Err(Error::unexpected_expr_type(expr)),
                }
            }

            fn from_value(value: &Lit) -> Result<Self> {
                let expr_array = syn::ExprArray::from_value(value)?;
                Self::from_expr(&syn::Expr::Array(expr_array))
            }
        }
    };
}

from_numeric_array!(u8);
from_numeric_array!(u16);
from_numeric_array!(u32);
from_numeric_array!(u64);
from_numeric_array!(usize);

impl FromMeta for syn::Lit {
    fn from_value(value: &Lit) -> Result<Self> {
        Ok(value.clone())
    }
}

macro_rules! from_meta_lit {
    ($impl_ty:path, $lit_variant:path) => {
        impl FromMeta for $impl_ty {
            fn from_value(value: &Lit) -> Result<Self> {
                if let $lit_variant(ref value) = *value {
                    Ok(value.clone())
                } else {
                    Err(Error::unexpected_lit_type(value))
                }
            }
        }

        impl FromMeta for Vec<$impl_ty> {
            fn from_list(items: &[NestedMeta]) -> Result<Self> {
                items
                    .iter()
                    .map(<$impl_ty as FromMeta>::from_nested_meta)
                    .collect()
            }

            fn from_value(value: &syn::Lit) -> Result<Self> {
                let expr_array = syn::ExprArray::from_value(value)?;
                Self::from_expr(&syn::Expr::Array(expr_array))
            }

            fn from_expr(expr: &syn::Expr) -> Result<Self> {
                match expr {
                    syn::Expr::Array(expr_array) => expr_array
                        .elems
                        .iter()
                        .map(<$impl_ty as FromMeta>::from_expr)
                        .collect::<Result<Vec<_>>>(),
                    syn::Expr::Lit(expr_lit) => Self::from_value(&expr_lit.lit),
                    syn::Expr::Group(g) => Self::from_expr(&g.expr),
                    _ => Err(Error::unexpected_expr_type(expr)),
                }
            }
        }
    };
}

from_meta_lit!(syn::LitInt, Lit::Int);
from_meta_lit!(syn::LitFloat, Lit::Float);
from_meta_lit!(syn::LitStr, Lit::Str);
from_meta_lit!(syn::LitByte, Lit::Byte);
from_meta_lit!(syn::LitByteStr, Lit::ByteStr);
from_meta_lit!(syn::LitChar, Lit::Char);
from_meta_lit!(syn::LitBool, Lit::Bool);
from_meta_lit!(proc_macro2::Literal, Lit::Verbatim);

impl FromMeta for syn::Meta {
    fn from_meta(value: &syn::Meta) -> Result<Self> {
        Ok(value.clone())
    }
}

impl FromMeta for Vec<syn::WherePredicate> {
    fn from_string(value: &str) -> Result<Self> {
        syn::WhereClause::from_string(&format!("where {}", value))
            .map(|c| c.predicates.into_iter().collect())
    }

    fn from_value(value: &Lit) -> Result<Self> {
        if let syn::Lit::Str(s) = value {
            syn::WhereClause::from_value(&syn::Lit::Str(syn::LitStr::new(
                &format!("where {}", s.value()),
                value.span(),
            )))
            .map(|c| c.predicates.into_iter().collect())
        } else {
            Err(Error::unexpected_lit_type(value))
        }
    }
}

impl FromMeta for ident_case::RenameRule {
    fn from_string(value: &str) -> Result<Self> {
        value.parse().map_err(|_| Error::unknown_value(value))
    }
}

impl<T: FromMeta> FromMeta for Option<T> {
    fn from_none() -> Option<Self> {
        Some(None)
    }

    fn from_meta(item: &Meta) -> Result<Self> {
        FromMeta::from_meta(item).map(Some)
    }
}

impl<T: FromMeta> FromMeta for Result<T> {
    fn from_none() -> Option<Self> {
        T::from_none().map(Ok)
    }

    // `#[darling(flatten)]` forwards directly to this method, so it's
    // necessary to declare it to avoid getting an unsupported format
    // error if it's invoked directly.
    fn from_list(items: &[NestedMeta]) -> Result<Self> {
        Ok(FromMeta::from_list(items))
    }

    fn from_meta(item: &Meta) -> Result<Self> {
        Ok(FromMeta::from_meta(item))
    }
}

/// Create an impl that forwards to an inner type `T` for parsing.
macro_rules! smart_pointer_t {
    ($ty:path, $map_fn:path) => {
        impl<T: FromMeta> FromMeta for $ty {
            fn from_none() -> Option<Self> {
                T::from_none().map($map_fn)
            }

            // `#[darling(flatten)]` forwards directly to this method, so it's
            // necessary to declare it to avoid getting an unsupported format
            // error if it's invoked directly.
            fn from_list(items: &[NestedMeta]) -> Result<Self> {
                FromMeta::from_list(items).map($map_fn)
            }

            fn from_meta(item: &Meta) -> Result<Self> {
                FromMeta::from_meta(item).map($map_fn)
            }
        }
    };
}

smart_pointer_t!(Box<T>, Box::new);
smart_pointer_t!(Rc<T>, Rc::new);
smart_pointer_t!(Arc<T>, Arc::new);
smart_pointer_t!(RefCell<T>, RefCell::new);

/// Parses the meta-item, and in case of error preserves a copy of the input for
/// later analysis.
impl<T: FromMeta> FromMeta for ::std::result::Result<T, Meta> {
    fn from_meta(item: &Meta) -> Result<Self> {
        T::from_meta(item)
            .map(Ok)
            .or_else(|_| Ok(Err(item.clone())))
    }
}

/// Trait to convert from a path into an owned key for a map.
trait KeyFromPath: Sized {
    fn from_path(path: &syn::Path) -> Result<Self>;
    fn to_display(&self) -> Cow<'_, str>;
}

impl KeyFromPath for String {
    fn from_path(path: &syn::Path) -> Result<Self> {
        Ok(path_to_string(path))
    }

    fn to_display(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }
}

impl KeyFromPath for syn::Path {
    fn from_path(path: &syn::Path) -> Result<Self> {
        Ok(path.clone())
    }

    fn to_display(&self) -> Cow<'_, str> {
        Cow::Owned(path_to_string(self))
    }
}

impl KeyFromPath for syn::Ident {
    fn from_path(path: &syn::Path) -> Result<Self> {
        if path.segments.len() == 1
            && path.leading_colon.is_none()
            && path.segments[0].arguments.is_empty()
        {
            Ok(path.segments[0].ident.clone())
        } else {
            Err(Error::custom("Key must be an identifier").with_span(path))
        }
    }

    fn to_display(&self) -> Cow<'_, str> {
        Cow::Owned(self.to_string())
    }
}

macro_rules! map {
    (hash_map, $key:ty, $nested:ident) => {
        impl<V: FromMeta, S: BuildHasher + Default> FromMeta for HashMap<$key, V, S> {
            map!(
                HashMap::with_capacity_and_hasher($nested.len(), Default::default()),
                $key,
                $nested
            );
        }
    };

    (btree_map, $key:ty, $nested:ident) => {
        impl<V: FromMeta> FromMeta for BTreeMap<$key, V> {
            map!(BTreeMap::new(), $key, $nested);
        }
    };

    ($new:expr, $key:ty, $nested:ident) => {
        fn from_list($nested: &[NestedMeta]) -> Result<Self> {
            // Convert the nested meta items into a sequence of (path, value result) result tuples.
            // An outer Err means no (key, value) structured could be found, while an Err in the
            // second position of the tuple means that value was rejected by FromMeta.
            //
            // We defer key conversion into $key so that we don't lose span information in the case
            // of String keys; we'll need it for good duplicate key errors later.
            let pairs = $nested
                .iter()
                .map(|item| -> Result<(&syn::Path, Result<V>)> {
                    match *item {
                        NestedMeta::Meta(ref inner) => {
                            let path = inner.path();
                            Ok((
                                path,
                                FromMeta::from_meta(inner).map_err(|e| e.at_path(&path)),
                            ))
                        }
                        NestedMeta::Lit(_) => Err(Error::unsupported_format("expression")),
                    }
                });

            let mut errors = Error::accumulator();
            // We need to track seen keys separately from the final map, since a seen key with an
            // Err value won't go into the final map but should trigger a duplicate field error.
            //
            // This is a set of $key rather than Path to avoid the possibility that a key type
            // parses two paths of different values to the same key value.
            let mut seen_keys = HashSet::with_capacity($nested.len());

            // The map to return in the Ok case. Its size will always be exactly nested.len(),
            // since otherwise ≥1 field had a problem and the entire map is dropped immediately
            // when the function returns `Err`.
            let mut map = $new;

            for item in pairs {
                if let Some((path, value)) = errors.handle(item) {
                    let key: $key = match KeyFromPath::from_path(path) {
                        Ok(k) => k,
                        Err(e) => {
                            errors.push(e);

                            // Surface value errors even under invalid keys
                            errors.handle(value);

                            continue;
                        }
                    };

                    let already_seen = seen_keys.contains(&key);

                    if already_seen {
                        errors.push(Error::duplicate_field(&key.to_display()).with_span(path));
                    }

                    match value {
                        Ok(_) if already_seen => {}
                        Ok(val) => {
                            map.insert(key.clone(), val);
                        }
                        Err(e) => {
                            errors.push(e);
                        }
                    }

                    seen_keys.insert(key);
                }
            }

            errors.finish_with(map)
        }
    };
}

// This is done as a macro rather than a blanket impl to avoid breaking backwards compatibility
// with 0.12.x, while still sharing the same impl.
map!(hash_map, String, nested);
map!(hash_map, syn::Ident, nested);
map!(hash_map, syn::Path, nested);

map!(btree_map, String, nested);
map!(btree_map, syn::Ident, nested);

/// Tests for `FromMeta` implementations. Wherever the word `ignore` appears in test input,
/// it should not be considered by the parsing.
#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroU64};

    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::parse_quote;

    use crate::{Error, FromMeta, Result};

    /// parse a string as a syn::Meta instance.
    fn pm(tokens: TokenStream) -> ::std::result::Result<syn::Meta, String> {
        let attribute: syn::Attribute = parse_quote!(#[#tokens]);
        Ok(attribute.meta)
    }

    #[track_caller]
    fn fm<T: FromMeta>(tokens: TokenStream) -> T {
        FromMeta::from_meta(&pm(tokens).expect("Tests should pass well-formed input"))
            .expect("Tests should pass valid input")
    }

    #[test]
    fn unit_succeeds() {
        fm::<()>(quote!(ignore));
    }

    #[test]
    #[allow(clippy::bool_assert_comparison)]
    fn bool_succeeds() {
        // word format
        assert_eq!(fm::<bool>(quote!(ignore)), true);

        // bool literal
        assert_eq!(fm::<bool>(quote!(ignore = true)), true);
        assert_eq!(fm::<bool>(quote!(ignore = false)), false);

        // string literals
        assert_eq!(fm::<bool>(quote!(ignore = "true")), true);
        assert_eq!(fm::<bool>(quote!(ignore = "false")), false);
    }

    #[test]
    fn char_succeeds() {
        // char literal
        assert_eq!(fm::<char>(quote!(ignore = '😬')), '😬');

        // string literal
        assert_eq!(fm::<char>(quote!(ignore = "😬")), '😬');
    }

    #[test]
    fn string_succeeds() {
        // cooked form
        assert_eq!(&fm::<String>(quote!(ignore = "world")), "world");

        // raw form
        assert_eq!(&fm::<String>(quote!(ignore = r#"world"#)), "world");
    }

    #[test]
    fn pathbuf_succeeds() {
        assert_eq!(
            fm::<std::path::PathBuf>(quote!(ignore = r#"C:\"#)),
            std::path::PathBuf::from(r#"C:\"#)
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // we want exact equality
    fn number_succeeds() {
        assert_eq!(fm::<u8>(quote!(ignore = "2")), 2u8);
        assert_eq!(fm::<i16>(quote!(ignore = "-25")), -25i16);
        assert_eq!(fm::<f64>(quote!(ignore = "1.4e10")), 1.4e10);
    }

    #[should_panic(expected = "UnknownValue(\"0\")")]
    #[test]
    fn nonzero_number_fails() {
        fm::<NonZeroU64>(quote!(ignore = "0"));
    }

    #[test]
    fn nonzero_number_succeeds() {
        assert_eq!(
            fm::<NonZeroU32>(quote!(ignore = "2")),
            NonZeroU32::new(2).unwrap()
        );
    }

    #[test]
    fn int_without_quotes() {
        assert_eq!(fm::<u8>(quote!(ignore = 2)), 2u8);
        assert_eq!(fm::<u16>(quote!(ignore = 255)), 255u16);
        assert_eq!(fm::<u32>(quote!(ignore = 5000)), 5000u32);

        // Check that we aren't tripped up by incorrect suffixes
        assert_eq!(fm::<u32>(quote!(ignore = 5000i32)), 5000u32);
    }

    #[test]
    fn negative_int_without_quotes() {
        assert_eq!(fm::<i8>(quote!(ignore = -2)), -2i8);
        assert_eq!(fm::<i32>(quote!(ignore = -255)), -255i32);
    }

    #[test]
    #[allow(clippy::float_cmp)] // we want exact equality
    fn float_without_quotes() {
        assert_eq!(fm::<f32>(quote!(ignore = 2.)), 2.0f32);
        assert_eq!(fm::<f32>(quote!(ignore = 2.0)), 2.0f32);
        assert_eq!(fm::<f64>(quote!(ignore = 1.4e10)), 1.4e10f64);
    }

    #[test]
    fn too_large_int_produces_error() {
        assert!(fm::<Result<u8>>(quote!(ignore = 2000)).is_err());
    }

    #[test]
    fn meta_succeeds() {
        use syn::Meta;

        assert_eq!(
            fm::<Meta>(quote!(hello(world, today))),
            pm(quote!(hello(world, today))).unwrap()
        );
    }

    #[test]
    fn hash_map_succeeds() {
        use std::collections::HashMap;

        let comparison = {
            let mut c = HashMap::new();
            c.insert("hello".to_string(), true);
            c.insert("world".to_string(), false);
            c.insert("there".to_string(), true);
            c
        };

        assert_eq!(
            fm::<HashMap<String, bool>>(quote!(ignore(hello, world = false, there = "true"))),
            comparison
        );
    }

    /// Check that a `HashMap` cannot have duplicate keys, and that the generated error
    /// is assigned a span to correctly target the diagnostic message.
    #[test]
    fn hash_map_duplicate() {
        use std::collections::HashMap;

        let err: Result<HashMap<String, bool>> =
            FromMeta::from_meta(&pm(quote!(ignore(hello, hello = false))).unwrap());

        let err = err.expect_err("Duplicate keys in HashMap should error");

        assert!(err.has_span());
        assert_eq!(err.to_string(), Error::duplicate_field("hello").to_string());
    }

    #[test]
    fn hash_map_multiple_errors() {
        use std::collections::HashMap;

        let err = HashMap::<String, bool>::from_meta(
            &pm(quote!(ignore(hello, hello = 3, hello = false))).unwrap(),
        )
        .expect_err("Duplicates and bad values should error");

        assert_eq!(err.len(), 3);
        let errors = err.into_iter().collect::<Vec<_>>();
        assert!(errors[0].has_span());
        assert!(errors[1].has_span());
        assert!(errors[2].has_span());
    }

    #[test]
    fn hash_map_ident_succeeds() {
        use std::collections::HashMap;
        use syn::parse_quote;

        let comparison = {
            let mut c = HashMap::<syn::Ident, bool>::new();
            c.insert(parse_quote!(first), true);
            c.insert(parse_quote!(second), false);
            c
        };

        assert_eq!(
            fm::<HashMap<syn::Ident, bool>>(quote!(ignore(first, second = false))),
            comparison
        );
    }

    #[test]
    fn hash_map_ident_rejects_non_idents() {
        use std::collections::HashMap;

        let err: Result<HashMap<syn::Ident, bool>> =
            FromMeta::from_meta(&pm(quote!(ignore(first, the::second))).unwrap());

        err.unwrap_err();
    }

    #[test]
    fn hash_map_path_succeeds() {
        use std::collections::HashMap;
        use syn::parse_quote;

        let comparison = {
            let mut c = HashMap::<syn::Path, bool>::new();
            c.insert(parse_quote!(first), true);
            c.insert(parse_quote!(the::second), false);
            c
        };

        assert_eq!(
            fm::<HashMap<syn::Path, bool>>(quote!(ignore(first, the::second = false))),
            comparison
        );
    }

    #[test]
    fn btree_map_succeeds() {
        use std::collections::BTreeMap;

        let comparison = {
            let mut c = BTreeMap::new();
            c.insert("hello".to_string(), true);
            c.insert("world".to_string(), false);
            c.insert("there".to_string(), true);
            c
        };

        assert_eq!(
            fm::<BTreeMap<String, bool>>(quote!(ignore(hello, world = false, there = "true"))),
            comparison
        );
    }

    /// Check that a `HashMap` cannot have duplicate keys, and that the generated error
    /// is assigned a span to correctly target the diagnostic message.
    #[test]
    fn btree_map_duplicate() {
        use std::collections::BTreeMap;

        let err: Result<BTreeMap<String, bool>> =
            FromMeta::from_meta(&pm(quote!(ignore(hello, hello = false))).unwrap());

        let err = err.expect_err("Duplicate keys in BTreeMap should error");

        assert!(err.has_span());
        assert_eq!(err.to_string(), Error::duplicate_field("hello").to_string());
    }

    #[test]
    fn btree_map_multiple_errors() {
        use std::collections::BTreeMap;

        let err = BTreeMap::<String, bool>::from_meta(
            &pm(quote!(ignore(hello, hello = 3, hello = false))).unwrap(),
        )
        .expect_err("Duplicates and bad values should error");

        assert_eq!(err.len(), 3);
        let errors = err.into_iter().collect::<Vec<_>>();
        assert!(errors[0].has_span());
        assert!(errors[1].has_span());
        assert!(errors[2].has_span());
    }

    #[test]
    fn btree_map_ident_succeeds() {
        use std::collections::BTreeMap;
        use syn::parse_quote;

        let comparison = {
            let mut c = BTreeMap::<syn::Ident, bool>::new();
            c.insert(parse_quote!(first), true);
            c.insert(parse_quote!(second), false);
            c
        };

        assert_eq!(
            fm::<BTreeMap<syn::Ident, bool>>(quote!(ignore(first, second = false))),
            comparison
        );
    }

    #[test]
    fn btree_map_ident_rejects_non_idents() {
        use std::collections::BTreeMap;

        let err: Result<BTreeMap<syn::Ident, bool>> =
            FromMeta::from_meta(&pm(quote!(ignore(first, the::second))).unwrap());

        err.unwrap_err();
    }

    #[test]
    fn btree_map_expr_values_succeed() {
        use std::collections::BTreeMap;
        use syn::parse_quote;

        let comparison: BTreeMap<String, syn::Expr> = vec![
            ("hello", parse_quote!(2 + 2)),
            ("world", parse_quote!(x.foo())),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        assert_eq!(
            fm::<BTreeMap<String, syn::Expr>>(quote!(ignore(hello = 2 + 2, world = x.foo()))),
            comparison
        );
    }

    /// Tests that fallible parsing will always produce an outer `Ok` (from `fm`),
    /// and will accurately preserve the inner contents.
    #[test]
    fn darling_result_succeeds() {
        fm::<Result<()>>(quote!(ignore)).unwrap();
        fm::<Result<()>>(quote!(ignore(world))).unwrap_err();
    }

    /// Test punctuated
    #[test]
    fn test_punctuated() {
        fm::<syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>>(quote!(
            ignore = "a: u8, b: Type"
        ));
        fm::<syn::punctuated::Punctuated<syn::Expr, syn::token::Comma>>(quote!(ignore = "a, b, c"));
    }

    #[test]
    fn test_expr_array() {
        fm::<syn::ExprArray>(quote!(ignore = "[0x1, 0x2]"));
        fm::<syn::ExprArray>(quote!(ignore = "[\"Hello World\", \"Test Array\"]"));
    }

    #[test]
    fn test_expr() {
        fm::<syn::Expr>(quote!(ignore = "x + y"));
        fm::<syn::Expr>(quote!(ignore = "an_object.method_call()"));
        fm::<syn::Expr>(quote!(ignore = "{ a_statement(); in_a_block }"));
    }

    #[test]
    fn test_expr_without_quotes() {
        fm::<syn::Expr>(quote!(ignore = x + y));
        fm::<syn::Expr>(quote!(ignore = an_object.method_call()));
        fm::<syn::Expr>(quote!(
            ignore = {
                a_statement();
                in_a_block
            }
        ));
    }

    #[test]
    fn test_expr_path() {
        fm::<syn::ExprPath>(quote!(ignore = "std::mem::replace"));
        fm::<syn::ExprPath>(quote!(ignore = "x"));
        fm::<syn::ExprPath>(quote!(ignore = "example::<Test>"));
    }

    #[test]
    fn test_expr_path_without_quotes() {
        fm::<syn::ExprPath>(quote!(ignore = std::mem::replace));
        fm::<syn::ExprPath>(quote!(ignore = x));
        fm::<syn::ExprPath>(quote!(ignore = example::<Test>));
    }

    #[test]
    fn test_path_without_quotes() {
        fm::<syn::Path>(quote!(ignore = std::mem::replace));
        fm::<syn::Path>(quote!(ignore = x));
        fm::<syn::Path>(quote!(ignore = example::<Test>));
    }

    #[test]
    fn test_number_array() {
        assert_eq!(fm::<Vec<u8>>(quote!(ignore = [16, 0xff])), vec![0x10, 0xff]);
        assert_eq!(
            fm::<Vec<u16>>(quote!(ignore = "[32, 0xffff]")),
            vec![0x20, 0xffff]
        );
        assert_eq!(
            fm::<Vec<u32>>(quote!(ignore = "[48, 0xffffffff]")),
            vec![0x30, 0xffffffff]
        );
        assert_eq!(
            fm::<Vec<u64>>(quote!(ignore = "[64, 0xffffffffffffffff]")),
            vec![0x40, 0xffffffffffffffff]
        );
        assert_eq!(
            fm::<Vec<usize>>(quote!(ignore = "[80, 0xffffffff]")),
            vec![0x50, 0xffffffff]
        );
    }

    #[test]
    fn test_lit_array() {
        fm::<Vec<syn::LitStr>>(quote!(ignore = "[\"Hello World\", \"Test Array\"]"));
        fm::<Vec<syn::LitStr>>(quote!(ignore = ["Hello World", "Test Array"]));
        fm::<Vec<syn::LitChar>>(quote!(ignore = "['a', 'b', 'c']"));
        fm::<Vec<syn::LitBool>>(quote!(ignore = "[true]"));
        fm::<Vec<syn::LitStr>>(quote!(ignore = "[]"));
        fm::<Vec<syn::LitStr>>(quote!(ignore = []));
        fm::<Vec<syn::LitBool>>(quote!(ignore = [true, false]));
    }

    #[test]
    fn expr_range_without_quotes() {
        fm::<syn::ExprRange>(quote!(ignore = 0..5));
        fm::<syn::ExprRange>(quote!(ignore = 0..=5));
        fm::<syn::ExprRange>(quote!(ignore = ..5));
        fm::<syn::ExprRange>(quote!(ignore = ..(x + y)));
    }
}
