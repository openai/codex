mod expr;
mod filter;
mod node;

use std::borrow::Cow;
use std::collections::hash_map::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::str;
use std::sync::Arc;

use parser::node::{Macro, Whitespace};
use parser::{
    CharLit, Expr, FloatKind, IntKind, MAX_RUST_KEYWORD_LEN, Num, RUST_KEYWORDS, StrLit, WithSpan,
};
use rustc_hash::FxBuildHasher;

use crate::ascii_str::{AsciiChar, AsciiStr};
use crate::heritage::{Context, Heritage};
use crate::html::write_escaped_str;
use crate::input::{Source, TemplateInput};
use crate::integration::{Buffer, impl_everything, write_header};
use crate::{CompileError, FileInfo};

pub(crate) fn template_to_string(
    buf: &mut Buffer,
    input: &TemplateInput<'_>,
    contexts: &HashMap<&Arc<Path>, Context<'_>, FxBuildHasher>,
    heritage: Option<&Heritage<'_, '_>>,
    tmpl_kind: TmplKind<'_>,
) -> Result<usize, CompileError> {
    let generator = Generator::new(
        input,
        contexts,
        heritage,
        MapChain::default(),
        input.block.is_some(),
        0,
    );
    let size_hint = match generator.impl_template(buf, tmpl_kind) {
        Err(mut err) if err.span.is_none() => {
            err.span = input.source_span;
            Err(err)
        }
        result => result,
    }?;

    if tmpl_kind == TmplKind::Struct {
        impl_everything(input.ast, buf);
    }
    Ok(size_hint)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TmplKind<'a> {
    /// [`askama::Template`]
    Struct,
    /// [`askama::helpers::EnumVariantTemplate`]
    Variant,
    /// Used in `blocks` implementation
    #[allow(unused)]
    Block(&'a str),
}

struct Generator<'a, 'h> {
    /// The template input state: original struct AST and attributes
    input: &'a TemplateInput<'a>,
    /// All contexts, keyed by the package-relative template path
    contexts: &'a HashMap<&'a Arc<Path>, Context<'a>, FxBuildHasher>,
    /// The heritage contains references to blocks and their ancestry
    heritage: Option<&'h Heritage<'a, 'h>>,
    /// Variables accessible directly from the current scope (not redirected to context)
    locals: MapChain<'a>,
    /// Suffix whitespace from the previous literal. Will be flushed to the
    /// output buffer unless suppressed by whitespace suppression on the next
    /// non-literal.
    next_ws: Option<&'a str>,
    /// Whitespace suppression from the previous non-literal. Will be used to
    /// determine whether to flush prefix whitespace from the next literal.
    skip_ws: Whitespace,
    /// If currently in a block, this will contain the name of a potential parent block
    super_block: Option<(&'a str, usize)>,
    /// Buffer for writable
    buf_writable: WritableBuffer<'a>,
    /// Used in blocks to check if we are inside a filter block.
    is_in_filter_block: usize,
    /// Set of called macros we are currently in. Used to prevent (indirect) recursions.
    seen_macros: Vec<(&'a Macro<'a>, Option<FileInfo<'a>>)>,
}

impl<'a, 'h> Generator<'a, 'h> {
    fn new(
        input: &'a TemplateInput<'a>,
        contexts: &'a HashMap<&'a Arc<Path>, Context<'a>, FxBuildHasher>,
        heritage: Option<&'h Heritage<'a, 'h>>,
        locals: MapChain<'a>,
        buf_writable_discard: bool,
        is_in_filter_block: usize,
    ) -> Self {
        Self {
            input,
            contexts,
            heritage,
            locals,
            next_ws: None,
            skip_ws: Whitespace::Preserve,
            super_block: None,
            buf_writable: WritableBuffer {
                discard: buf_writable_discard,
                ..Default::default()
            },
            is_in_filter_block,
            seen_macros: Vec::new(),
        }
    }

    // Implement `Template` for the given context struct.
    fn impl_template(
        mut self,
        buf: &mut Buffer,
        tmpl_kind: TmplKind<'a>,
    ) -> Result<usize, CompileError> {
        let ctx = &self.contexts[&self.input.path];

        let target = match tmpl_kind {
            TmplKind::Struct => "askama::Template",
            TmplKind::Variant => "askama::helpers::EnumVariantTemplate",
            TmplKind::Block(trait_name) => trait_name,
        };
        write_header(self.input.ast, buf, target);
        buf.write(
            "fn render_into_with_values<AskamaW>(\
                &self,\
                __askama_writer: &mut AskamaW,\
                __askama_values: &dyn askama::Values\
            ) -> askama::Result<()>\
            where \
                AskamaW: askama::helpers::core::fmt::Write + ?askama::helpers::core::marker::Sized\
            {\
                #[allow(unused_imports)]\
                use askama::{\
                    filters::{AutoEscape as _, WriteWritable as _},\
                    helpers::{ResultConverter as _, core::fmt::Write as _},\
                };",
        );

        if let Some(full_config_path) = &self.input.config.full_config_path {
            buf.write(format_args!(
                "const _: &[askama::helpers::core::primitive::u8] =\
                askama::helpers::core::include_bytes!({:?});",
                full_config_path.display()
            ));
        }

        // Make sure the compiler understands that the generated code depends on the template files.
        let mut paths = self
            .contexts
            .keys()
            .map(|path| -> &Path { path })
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            // Skip the fake path of templates defined in rust source.
            let path_is_valid = match self.input.source {
                Source::Path(_) => true,
                Source::Source(_) => path != &*self.input.path,
            };
            if path_is_valid {
                buf.write(format_args!(
                    "const _: &[askama::helpers::core::primitive::u8] =\
                        askama::helpers::core::include_bytes!({:#?});",
                    path.canonicalize().as_deref().unwrap_or(path),
                ));
            }
        }

        let size_hint = self.impl_template_inner(ctx, buf)?;

        buf.write("askama::Result::Ok(()) }");
        if tmpl_kind == TmplKind::Struct {
            buf.write(format_args!(
                "const SIZE_HINT: askama::helpers::core::primitive::usize = {size_hint}usize;",
            ));
        }

        buf.write('}');

        #[cfg(feature = "blocks")]
        for block in self.input.blocks {
            self.impl_block(buf, block)?;
        }

        Ok(size_hint)
    }

    #[cfg(feature = "blocks")]
    fn impl_block(
        &self,
        buf: &mut Buffer,
        block: &crate::input::Block,
    ) -> Result<(), CompileError> {
        // RATIONALE: `*self` must be the input type, implementation details should not leak:
        // - impl Self { fn as_block(self) } ->
        // - struct __Askama__Self__as__block__Wrapper { this: self } ->
        // - impl Template for __Askama__Self__as__block__Wrapper { fn render_into_with_values() } ->
        // - impl __Askama__Self__as__block for Self { render_into_with_values() }

        use quote::quote_spanned;
        use syn::{GenericParam, Ident, Lifetime, LifetimeParam, Token};

        let span = block.span;
        buf.write(
            "\
            #[allow(missing_docs, non_camel_case_types, non_snake_case, unreachable_pub)]\
            const _: () = {",
        );

        let ident = &self.input.ast.ident;

        let doc = format!(
            "A sub-template that renders only the block `{}` of [`{ident}`].",
            block.name
        );
        let method_name = format!("as_{}", block.name);
        let trait_name = format!("__Askama__{ident}__as__{}", block.name);
        let wrapper_name = format!("__Askama__{ident}__as__{}__Wrapper", block.name);
        let self_lt_name = format!("'__Askama__{ident}__as__{}__self", block.name);

        let method_id = Ident::new(&method_name, span);
        let trait_id = Ident::new(&trait_name, span);
        let wrapper_id = Ident::new(&wrapper_name, span);
        let self_lt = Lifetime::new(&self_lt_name, span);

        // generics of the input with an additional lifetime to capture `self`
        let mut wrapper_generics = self.input.ast.generics.clone();
        if wrapper_generics.lt_token.is_none() {
            wrapper_generics.lt_token = Some(Token![<](span));
            wrapper_generics.gt_token = Some(Token![>](span));
        }
        wrapper_generics.params.insert(
            0,
            GenericParam::Lifetime(LifetimeParam::new(self_lt.clone())),
        );

        let (impl_generics, ty_generics, where_clause) = self.input.ast.generics.split_for_impl();
        let (wrapper_impl_generics, wrapper_ty_generics, wrapper_where_clause) =
            wrapper_generics.split_for_impl();

        let input = TemplateInput {
            block: Some((&block.name, span)),
            blocks: &[],
            ..self.input.clone()
        };
        let size_hint = template_to_string(
            buf,
            &input,
            self.contexts,
            self.heritage,
            TmplKind::Block(&trait_name),
        )?;

        buf.write(quote_spanned! {
            span =>
            pub trait #trait_id {
                fn render_into_with_values<AskamaW>(
                    &self,
                    writer: &mut AskamaW,
                    values: &dyn askama::Values,
                ) -> askama::Result<()>
                where
                    AskamaW:
                        askama::helpers::core::fmt::Write + ?askama::helpers::core::marker::Sized;
            }

            impl #impl_generics #ident #ty_generics #where_clause {
                #[inline]
                #[doc = #doc]
                pub fn #method_id(&self) -> impl askama::Template + '_ {
                    #wrapper_id {
                        this: self,
                    }
                }
            }

            #[askama::helpers::core::prelude::rust_2021::derive(
                askama::helpers::core::prelude::rust_2021::Clone,
                askama::helpers::core::prelude::rust_2021::Copy
            )]
            pub struct #wrapper_id #wrapper_generics #wrapper_where_clause {
                this: &#self_lt #ident #ty_generics,
            }

            impl #wrapper_impl_generics askama::Template
            for #wrapper_id #wrapper_ty_generics #wrapper_where_clause {
                #[inline]
                fn render_into_with_values<AskamaW>(
                    &self,
                    writer: &mut AskamaW,
                    values: &dyn askama::Values
                ) -> askama::Result<()>
                where
                    AskamaW: askama::helpers::core::fmt::Write + ?askama::helpers::core::marker::Sized
                {
                    <_ as #trait_id>::render_into_with_values(self.this, writer, values)
                }

                const SIZE_HINT: askama::helpers::core::primitive::usize = #size_hint;
            }

            // cannot use `crate::integrations::impl_fast_writable()` w/o cloning the struct
            impl #wrapper_impl_generics askama::FastWritable
            for #wrapper_id #wrapper_ty_generics #wrapper_where_clause {
                #[inline]
                fn write_into<AskamaW>(
                    &self,
                    dest: &mut AskamaW,
                    values: &dyn askama::Values,
                ) -> askama::Result<()>
                where
                    AskamaW: askama::helpers::core::fmt::Write + ?askama::helpers::core::marker::Sized
                {
                    <_ as askama::Template>::render_into_with_values(self, dest, values)
                }
            }

            // cannot use `crate::integrations::impl_display()` w/o cloning the struct
            impl #wrapper_impl_generics askama::helpers::core::fmt::Display
            for #wrapper_id #wrapper_ty_generics #wrapper_where_clause {
                #[inline]
                fn fmt(
                    &self,
                    f: &mut askama::helpers::core::fmt::Formatter<'_>
                ) -> askama::helpers::core::fmt::Result {
                    <_ as askama::Template>::render_into(self, f)
                        .map_err(|_| askama::helpers::core::fmt::Error)
                }
            }
        });

        buf.write("};");
        Ok(())
    }

    fn is_var_defined(&self, var_name: &str) -> bool {
        self.locals.get(var_name).is_some() || self.input.fields.iter().any(|f| f == var_name)
    }
}

#[cfg(target_pointer_width = "16")]
type TargetIsize = i16;
#[cfg(target_pointer_width = "32")]
type TargetIsize = i32;
#[cfg(target_pointer_width = "64")]
type TargetIsize = i64;

#[cfg(target_pointer_width = "16")]
type TargetUsize = u16;
#[cfg(target_pointer_width = "32")]
type TargetUsize = u32;
#[cfg(target_pointer_width = "64")]
type TargetUsize = u64;

#[cfg(not(any(
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64"
)))]
const _: () = {
    panic!("unknown cfg!(target_pointer_width)");
};

/// In here, we inspect in the expression if it is a literal, and if it is, whether it
/// can be escaped at compile time.
fn compile_time_escape<'a>(expr: &Expr<'a>, escaper: &str) -> Option<Writable<'a>> {
    // we only optimize for known escapers
    enum OutputKind {
        Html,
        Text,
    }

    // we only optimize for known escapers
    let output = match escaper.strip_prefix("askama::filters::")? {
        "Html" => OutputKind::Html,
        "Text" => OutputKind::Text,
        _ => return None,
    };

    // for now, we only escape strings, chars, numbers, and bools at compile time
    let value = match *expr {
        Expr::StrLit(StrLit {
            prefix: None,
            content,
        }) => {
            if content.find('\\').is_none() {
                // if the literal does not contain any backslashes, then it does not need unescaping
                Cow::Borrowed(content)
            } else {
                // the input could be string escaped if it contains any backslashes
                let input = format!(r#""{content}""#);
                let input = input.parse().ok()?;
                let input = syn::parse2::<syn::LitStr>(input).ok()?;
                Cow::Owned(input.value())
            }
        }
        Expr::CharLit(CharLit {
            prefix: None,
            content,
        }) => {
            if content.find('\\').is_none() {
                // if the literal does not contain any backslashes, then it does not need unescaping
                Cow::Borrowed(content)
            } else {
                // the input could be string escaped if it contains any backslashes
                let input = format!(r#"'{content}'"#);
                let input = input.parse().ok()?;
                let input = syn::parse2::<syn::LitChar>(input).ok()?;
                Cow::Owned(input.value().to_string())
            }
        }
        Expr::NumLit(_, value) => {
            enum NumKind {
                Int(Option<IntKind>),
                Float(Option<FloatKind>),
            }

            let (orig_value, kind) = match value {
                Num::Int(value, kind) => (value, NumKind::Int(kind)),
                Num::Float(value, kind) => (value, NumKind::Float(kind)),
            };
            let value = match orig_value.chars().any(|c| c == '_') {
                true => Cow::Owned(orig_value.chars().filter(|&c| c != '_').collect()),
                false => Cow::Borrowed(orig_value),
            };

            fn int<T: ToString, E>(
                from_str_radix: impl Fn(&str, u32) -> Result<T, E>,
                value: &str,
            ) -> Option<String> {
                Some(from_str_radix(value, 10).ok()?.to_string())
            }

            let value = match kind {
                NumKind::Int(Some(IntKind::I8)) => int(i8::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::I16)) => int(i16::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::I32)) => int(i32::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::I64)) => int(i64::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::I128)) => int(i128::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::Isize)) => int(TargetIsize::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U8)) => int(u8::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U16)) => int(u16::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U32)) => int(u32::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U64)) => int(u64::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U128)) => int(u128::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::Usize)) => int(TargetUsize::from_str_radix, &value)?,
                NumKind::Int(None) => match value.starts_with('-') {
                    true => int(i128::from_str_radix, &value)?,
                    false => int(u128::from_str_radix, &value)?,
                },
                NumKind::Float(Some(FloatKind::F32)) => value.parse::<f32>().ok()?.to_string(),
                NumKind::Float(Some(FloatKind::F64) | None) => {
                    value.parse::<f64>().ok()?.to_string()
                }
                // FIXME: implement once `f16` and `f128` are available
                NumKind::Float(Some(FloatKind::F16 | FloatKind::F128)) => return None,
            };
            match value == orig_value {
                true => Cow::Borrowed(orig_value),
                false => Cow::Owned(value),
            }
        }
        Expr::BoolLit(true) => Cow::Borrowed("true"),
        Expr::BoolLit(false) => Cow::Borrowed("false"),
        _ => return None,
    };

    // escape the un-string-escaped input using the selected escaper
    Some(Writable::Lit(match output {
        OutputKind::Text => value,
        OutputKind::Html => {
            let mut escaped = String::with_capacity(value.len() + 20);
            write_escaped_str(&mut escaped, &value).ok()?;
            match escaped == value {
                true => value,
                false => Cow::Owned(escaped),
            }
        }
    }))
}

#[derive(Clone, Default)]
struct LocalMeta {
    refs: Option<String>,
    initialized: bool,
}

impl LocalMeta {
    fn initialized() -> Self {
        Self {
            refs: None,
            initialized: true,
        }
    }

    fn with_ref(refs: String) -> Self {
        Self {
            refs: Some(refs),
            initialized: true,
        }
    }
}

struct MapChain<'a> {
    scopes: Vec<HashMap<Cow<'a, str>, LocalMeta, FxBuildHasher>>,
}

impl<'a> MapChain<'a> {
    fn new_empty() -> Self {
        Self { scopes: vec![] }
    }

    /// Iterates the scopes in reverse and returns `Some(LocalMeta)`
    /// from the first scope where `key` exists.
    fn get<'b>(&'b self, key: &str) -> Option<&'b LocalMeta> {
        self.scopes.iter().rev().find_map(|set| set.get(key))
    }

    fn is_current_empty(&self) -> bool {
        self.scopes.last().unwrap().is_empty()
    }

    fn insert(&mut self, key: Cow<'a, str>, val: LocalMeta) {
        self.scopes.last_mut().unwrap().insert(key, val);

        // Note that if `insert` returns `Some` then it implies
        // an identifier is reused. For e.g. `{% macro f(a, a) %}`
        // and `{% let (a, a) = ... %}` then this results in a
        // generated template, which when compiled fails with the
        // compile error "identifier `a` used more than once".
    }

    fn insert_with_default(&mut self, key: Cow<'a, str>) {
        self.insert(key, LocalMeta::default());
    }

    fn resolve(&self, name: &str) -> Option<String> {
        let name = normalize_identifier(name);
        self.get(&Cow::Borrowed(name)).map(|meta| match &meta.refs {
            Some(expr) => expr.clone(),
            None => name.to_string(),
        })
    }

    fn resolve_or_self(&self, name: &str) -> String {
        let name = normalize_identifier(name);
        self.resolve(name).unwrap_or_else(|| format!("self.{name}"))
    }
}

impl Default for MapChain<'_> {
    fn default() -> Self {
        Self {
            scopes: vec![HashMap::default()],
        }
    }
}

/// Returns `true` if enough assumptions can be made,
/// to determine that `self` is copyable.
fn is_copyable(expr: &Expr<'_>) -> bool {
    is_copyable_within_op(expr, false)
}

fn is_copyable_within_op(expr: &Expr<'_>, within_op: bool) -> bool {
    match expr {
        Expr::BoolLit(_)
        | Expr::NumLit(_, _)
        | Expr::StrLit(_)
        | Expr::CharLit(_)
        | Expr::BinOp(_, _, _) => true,
        Expr::Unary(.., expr) => is_copyable_within_op(expr, true),
        Expr::Range(..) => true,
        // The result of a call likely doesn't need to be borrowed,
        // as in that case the call is more likely to return a
        // reference in the first place then.
        Expr::Call { .. } | Expr::Path(..) | Expr::Filter(..) | Expr::RustMacro(..) => true,
        // If the `expr` is within a `Unary` or `BinOp` then
        // an assumption can be made that the operand is copy.
        // If not, then the value is moved and adding `.clone()`
        // will solve that issue. However, if the operand is
        // implicitly borrowed, then it's likely not even possible
        // to get the template to compile.
        _ => within_op && is_attr_self(expr),
    }
}

/// Returns `true` if this is an `Attr` where the `obj` is `"self"`.
fn is_attr_self(mut expr: &Expr<'_>) -> bool {
    loop {
        match expr {
            Expr::Attr(obj, _) if matches!(***obj, Expr::Var("self")) => return true,
            Expr::Attr(obj, _) if matches!(***obj, Expr::Attr(..)) => expr = obj,
            _ => return false,
        }
    }
}

const FILTER_SOURCE: &str = "__askama_filter_block";

#[derive(Clone, Copy, Debug)]
enum DisplayWrap {
    Wrapped,
    Unwrapped,
}

#[derive(Default, Debug)]
struct WritableBuffer<'a> {
    buf: Vec<Writable<'a>>,
    discard: bool,
}

impl<'a> WritableBuffer<'a> {
    fn push(&mut self, writable: Writable<'a>) {
        if !self.discard {
            self.buf.push(writable);
        }
    }
}

impl<'a> Deref for WritableBuffer<'a> {
    type Target = [Writable<'a>];

    fn deref(&self) -> &Self::Target {
        &self.buf[..]
    }
}

#[derive(Debug)]
enum Writable<'a> {
    Lit(Cow<'a, str>),
    Expr(&'a WithSpan<'a, Expr<'a>>),
}

/// Identifiers to be replaced with raw identifiers, so as to avoid
/// collisions between template syntax and Rust's syntax. In particular
/// [Rust keywords](https://doc.rust-lang.org/reference/keywords.html)
/// should be replaced, since they're not reserved words in Askama
/// syntax but have a high probability of causing problems in the
/// generated code.
///
/// This list excludes the Rust keywords *self*, *Self*, and *super*
/// because they are not allowed to be raw identifiers, and *loop*
/// because it's used something like a keyword in the template
/// language.
fn normalize_identifier(ident: &str) -> &str {
    // This table works for as long as the replacement string is the original string
    // prepended with "r#". The strings get right-padded to the same length with b'_'.
    // While the code does not need it, please keep the list sorted when adding new
    // keywords.

    if ident.len() > MAX_RUST_KEYWORD_LEN {
        return ident;
    }
    let kws = RUST_KEYWORDS[ident.len()];

    let mut padded_ident = [0; MAX_RUST_KEYWORD_LEN];
    padded_ident[..ident.len()].copy_from_slice(ident.as_bytes());

    // Since the individual buckets are quite short, a linear search is faster than a binary search.
    for probe in kws {
        if padded_ident == *AsciiChar::slice_as_bytes(probe[2..].try_into().unwrap()) {
            return AsciiStr::from_slice(&probe[..ident.len() + 2]);
        }
    }
    ident
}
