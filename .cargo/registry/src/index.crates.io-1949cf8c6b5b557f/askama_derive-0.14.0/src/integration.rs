use std::fmt::{Arguments, Display, Write};

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{
    Data, DeriveInput, Fields, GenericParam, Generics, Ident, Lifetime, LifetimeParam, Token, Type,
    Variant, parse_quote,
};

use crate::generator::TmplKind;
use crate::input::{PartialTemplateArgs, TemplateArgs};
use crate::{CompileError, build_template_item};

/// Implement every integration for the given item
pub(crate) fn impl_everything(ast: &DeriveInput, buf: &mut Buffer) {
    impl_display(ast, buf);
    impl_fast_writable(ast, buf);
}

/// Writes header for the `impl` for `TraitFromPathName` or `Template` for the given item
pub(crate) fn write_header(ast: &DeriveInput, buf: &mut Buffer, target: impl Display) {
    let (impl_generics, orig_ty_generics, where_clause) = ast.generics.split_for_impl();

    let ident = &ast.ident;
    buf.write(format_args!(
        "impl {} {} for {} {{",
        quote!(#impl_generics),
        target,
        quote!(#ident #orig_ty_generics #where_clause),
    ));
}

/// Implement `Display` for the given item.
fn impl_display(ast: &DeriveInput, buf: &mut Buffer) {
    let ident = &ast.ident;
    buf.write(format_args!(
        "\
        /// Implement the [`format!()`][askama::helpers::std::format] trait for [`{}`]\n\
        ///\n\
        /// Please be aware of the rendering performance notice in the \
            [`Template`][askama::Template] trait.\n\
        ",
        quote!(#ident),
    ));
    write_header(ast, buf, "askama::helpers::core::fmt::Display");
    buf.write(
        "\
            #[inline]\
            fn fmt(\
                &self,\
                f: &mut askama::helpers::core::fmt::Formatter<'_>\
            ) -> askama::helpers::core::fmt::Result {\
                askama::Template::render_into(self, f)\
                    .map_err(|_| askama::helpers::core::fmt::Error)\
            }\
        }",
    );
}

/// Implement `FastWritable` for the given item.
fn impl_fast_writable(ast: &DeriveInput, buf: &mut Buffer) {
    write_header(ast, buf, "askama::FastWritable");
    buf.write(
        "\
            #[inline]\
            fn write_into<AskamaW>(\
                &self,\
                dest: &mut AskamaW,\
                values: &dyn askama::Values\
            ) -> askama::Result<()> \
            where \
                AskamaW: askama::helpers::core::fmt::Write + ?askama::helpers::core::marker::Sized,\
            {\
                askama::Template::render_into_with_values(self, dest, values)\
            }\
        }",
    );
}

#[derive(Debug)]
pub(crate) struct Buffer {
    // The buffer to generate the code into
    buf: String,
    discard: bool,
    last_was_write_str: bool,
}

impl Display for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.buf)
    }
}

impl Buffer {
    pub(crate) fn new() -> Self {
        Self {
            buf: String::new(),
            discard: false,
            last_was_write_str: false,
        }
    }

    pub(crate) fn into_string(self) -> String {
        self.buf
    }

    pub(crate) fn is_discard(&self) -> bool {
        self.discard
    }

    pub(crate) fn set_discard(&mut self, discard: bool) {
        self.discard = discard;
        self.last_was_write_str = false;
    }

    pub(crate) fn write(&mut self, src: impl BufferFmt) {
        if self.discard {
            return;
        }
        self.last_was_write_str = false;

        src.append_to(&mut self.buf);
    }

    pub(crate) fn write_separated_path(&mut self, path: &[&str]) {
        if self.discard {
            return;
        }
        self.last_was_write_str = false;

        for (idx, item) in path.iter().enumerate() {
            if idx > 0 {
                self.buf.push_str("::");
            }
            self.buf.push_str(item);
        }
    }

    pub(crate) fn write_escaped_str(&mut self, s: &str) {
        if self.discard {
            return;
        }
        self.last_was_write_str = false;

        self.buf.push('"');
        string_escape(&mut self.buf, s);
        self.buf.push('"');
    }

    pub(crate) fn write_writer(&mut self, s: &str) -> usize {
        const OPEN: &str = r#"__askama_writer.write_str(""#;
        const CLOSE: &str = r#"")?;"#;

        if !s.is_empty() && !self.discard {
            if !self.last_was_write_str {
                self.last_was_write_str = true;
                self.buf.push_str(OPEN);
            } else {
                // strip trailing `")?;`, leaving an unterminated string
                self.buf.truncate(self.buf.len() - CLOSE.len());
            }
            string_escape(&mut self.buf, s);
            self.buf.push_str(CLOSE);
        }
        s.len()
    }

    pub(crate) fn clear(&mut self) {
        self.buf.clear();
        self.last_was_write_str = false;
    }

    pub(crate) fn get_mark(&mut self) -> usize {
        self.buf.len()
    }

    pub(crate) fn marked_text(&self, mark: usize) -> &str {
        &self.buf[..mark]
    }
}

pub(crate) trait BufferFmt {
    fn append_to(&self, buf: &mut String);
}

impl<T: BufferFmt + ?Sized> BufferFmt for &T {
    fn append_to(&self, buf: &mut String) {
        T::append_to(self, buf);
    }
}

impl BufferFmt for char {
    fn append_to(&self, buf: &mut String) {
        buf.push(*self);
    }
}

impl BufferFmt for str {
    fn append_to(&self, buf: &mut String) {
        buf.push_str(self);
    }
}

impl BufferFmt for String {
    fn append_to(&self, buf: &mut String) {
        buf.push_str(self);
    }
}

impl BufferFmt for Arguments<'_> {
    fn append_to(&self, buf: &mut String) {
        buf.write_fmt(*self).unwrap();
    }
}

impl BufferFmt for TokenStream {
    fn append_to(&self, buf: &mut String) {
        write!(buf, "{self}").unwrap();
    }
}

/// Similar to `write!(dest, "{src:?}")`, but only escapes the strictly needed characters,
/// and without the surrounding `"…"` quotation marks.
fn string_escape(dest: &mut String, src: &str) {
    // SAFETY: we will only push valid str slices
    let dest = unsafe { dest.as_mut_vec() };
    let src = src.as_bytes();
    let mut last = 0;

    // According to <https://doc.rust-lang.org/reference/tokens.html#string-literals>, every
    // character is valid except `" \ IsolatedCR`. We don't test if the `\r` is isolated or not,
    // but always escape it.
    for x in memchr::memchr3_iter(b'\\', b'"', b'\r', src) {
        dest.extend(&src[last..x]);
        dest.extend(match src[x] {
            b'\\' => br"\\",
            b'\"' => br#"\""#,
            _ => br"\r",
        });
        last = x + 1;
    }
    dest.extend(&src[last..]);
}

pub(crate) fn build_template_enum(
    buf: &mut Buffer,
    enum_ast: &DeriveInput,
    mut enum_args: Option<PartialTemplateArgs>,
    vars_args: Vec<Option<PartialTemplateArgs>>,
    has_default_impl: bool,
) -> Result<usize, CompileError> {
    let Data::Enum(enum_data) = &enum_ast.data else {
        unreachable!();
    };

    impl_everything(enum_ast, buf);

    let enum_id = &enum_ast.ident;
    let enum_span = enum_id.span();
    let lifetime = Lifetime::new(&format!("'__Askama_{enum_id}"), enum_span);

    let mut generics = enum_ast.generics.clone();
    if generics.lt_token.is_none() {
        generics.lt_token = Some(Token![<](enum_span));
    }
    if generics.gt_token.is_none() {
        generics.gt_token = Some(Token![>](enum_span));
    }
    generics
        .params
        .insert(0, GenericParam::Lifetime(LifetimeParam::new(lifetime)));

    let mut biggest_size_hint = 0;
    let mut render_into_arms = TokenStream::new();
    let mut size_hint_arms = TokenStream::new();
    for (var, var_args) in enum_data.variants.iter().zip(vars_args) {
        let Some(mut var_args) = var_args else {
            continue;
        };

        let var_ast = type_for_enum_variant(enum_ast, &generics, var);
        buf.write(quote!(#var_ast));

        // not inherited: template, meta_docs, block, print
        if let Some(enum_args) = &mut enum_args {
            set_default(&mut var_args, enum_args, |v| &mut v.source);
            set_default(&mut var_args, enum_args, |v| &mut v.escape);
            set_default(&mut var_args, enum_args, |v| &mut v.ext);
            set_default(&mut var_args, enum_args, |v| &mut v.syntax);
            set_default(&mut var_args, enum_args, |v| &mut v.config);
            set_default(&mut var_args, enum_args, |v| &mut v.whitespace);
        }
        let size_hint = biggest_size_hint.max(build_template_item(
            buf,
            &var_ast,
            Some(enum_ast),
            &TemplateArgs::from_partial(&var_ast, Some(var_args))?,
            TmplKind::Variant,
        )?);
        biggest_size_hint = biggest_size_hint.max(size_hint);

        variant_as_arm(
            &var_ast,
            var,
            size_hint,
            &mut render_into_arms,
            &mut size_hint_arms,
        );
    }
    if has_default_impl {
        let size_hint = build_template_item(
            buf,
            enum_ast,
            None,
            &TemplateArgs::from_partial(enum_ast, enum_args)?,
            TmplKind::Variant,
        )?;
        biggest_size_hint = biggest_size_hint.max(size_hint);

        render_into_arms.extend(quote! {
            ref __askama_arg => {
                <_ as askama::helpers::EnumVariantTemplate>::render_into_with_values(
                    __askama_arg,
                    __askama_writer,
                    __askama_values,
                )
            }
        });
        size_hint_arms.extend(quote! {
            _ => {
                #size_hint
            }
        });
    }

    write_header(enum_ast, buf, "askama::Template");
    buf.write(format_args!(
        "\
        fn render_into_with_values<AskamaW>(\
            &self,\
            __askama_writer: &mut AskamaW,\
            __askama_values: &dyn askama::Values,\
        ) -> askama::Result<()>\
        where \
            AskamaW: askama::helpers::core::fmt::Write + ?askama::helpers::core::marker::Sized\
        {{\
            match *self {{\
                {render_into_arms}\
            }}\
        }}",
    ));

    #[cfg(feature = "alloc")]
    buf.write(format_args!(
        "\
        fn render_with_values(\
            &self,\
            __askama_values: &dyn askama::Values,\
        ) -> askama::Result<askama::helpers::alloc::string::String> {{\
            let size_hint = match self {{\
                {size_hint_arms}\
            }};\
            let mut buf = askama::helpers::alloc::string::String::new();\
            let _ = buf.try_reserve(size_hint);\
            self.render_into_with_values(&mut buf, __askama_values)?;\
            askama::Result::Ok(buf)\
        }}",
    ));

    buf.write(format_args!(
        "\
        const SIZE_HINT: askama::helpers::core::primitive::usize = {biggest_size_hint}usize;\
        }}",
    ));
    Ok(biggest_size_hint)
}

fn set_default<S, T, A>(dest: &mut S, parent: &mut S, mut access: A)
where
    T: Clone,
    A: FnMut(&mut S) -> &mut Option<T>,
{
    let dest = access(dest);
    if dest.is_none() {
        if let Some(parent) = access(parent) {
            *dest = Some(parent.clone());
        }
    }
}

/// Generates a `struct` to contain the data of an enum variant
fn type_for_enum_variant(
    enum_ast: &DeriveInput,
    enum_generics: &Generics,
    var: &Variant,
) -> DeriveInput {
    let enum_id = &enum_ast.ident;
    let (_, ty_generics, _) = enum_ast.generics.split_for_impl();
    let lt = enum_generics.params.first().unwrap();

    let id = &var.ident;
    let span = id.span();
    let id = Ident::new(&format!("__Askama__{enum_id}__{id}"), span);

    let phantom: Type = parse_quote! {
        askama::helpers::core::marker::PhantomData < &#lt #enum_id #ty_generics >
    };
    let fields = match &var.fields {
        Fields::Named(fields) => {
            let mut fields = fields.clone();
            for f in fields.named.iter_mut() {
                let ty = &f.ty;
                f.ty = parse_quote!(&#lt #ty);
            }
            let id = Ident::new(&format!("__Askama__{enum_id}__phantom"), span);
            fields.named.push(parse_quote!(#id: #phantom));
            Fields::Named(fields)
        }
        Fields::Unnamed(fields) => {
            let mut fields = fields.clone();
            for f in fields.unnamed.iter_mut() {
                let ty = &f.ty;
                f.ty = parse_quote!(&#lt #ty);
            }
            fields.unnamed.push(parse_quote!(#phantom));
            Fields::Unnamed(fields)
        }
        Fields::Unit => Fields::Unnamed(parse_quote!((#phantom))),
    };
    let semicolon = match &var.fields {
        Fields::Named(_) => None,
        _ => Some(Token![;](span)),
    };

    parse_quote! {
        #[askama::helpers::core::prelude::rust_2021::derive(
            askama::helpers::core::prelude::rust_2021::Clone,
            askama::helpers::core::prelude::rust_2021::Copy,
            askama::helpers::core::prelude::rust_2021::Debug
        )]
        #[allow(dead_code, non_camel_case_types, non_snake_case)]
        struct #id #enum_generics #fields #semicolon
    }
}

/// Generates a `match` arm for an `enum` variant, that calls `<_ as EnumVariantTemplate>::render_into()`
/// for that type and data
fn variant_as_arm(
    var_ast: &DeriveInput,
    var: &Variant,
    size_hint: usize,
    render_into_arms: &mut TokenStream,
    size_hint_arms: &mut TokenStream,
) {
    let var_id = &var_ast.ident;
    let ident = &var.ident;
    let span = ident.span();

    let generics = var_ast.generics.clone();
    let (_, ty_generics, _) = generics.split_for_impl();
    let ty_generics: TokenStream = ty_generics
        .as_turbofish()
        .to_token_stream()
        .into_iter()
        .enumerate()
        .map(|(idx, token)| match idx {
            // 0 1 2 3 4   =>   : : < ' __Askama_Foo
            4 => TokenTree::Ident(Ident::new("_", span)),
            _ => token,
        })
        .collect();

    let Data::Struct(ast_data) = &var_ast.data else {
        unreachable!();
    };
    let mut src = TokenStream::new();
    let mut this = TokenStream::new();
    match &var.fields {
        Fields::Named(fields) => {
            for (idx, field) in fields.named.iter().enumerate() {
                let arg = Ident::new(&format!("__askama_arg_{idx}"), field.span());
                let id = field.ident.as_ref().unwrap();
                src.extend(quote!(#id: ref #arg,));
                this.extend(quote!(#id: #arg,));
            }

            let phantom = match &ast_data.fields {
                Fields::Named(fields) => fields
                    .named
                    .iter()
                    .next_back()
                    .unwrap()
                    .ident
                    .as_ref()
                    .unwrap(),
                Fields::Unnamed(_) | Fields::Unit => unreachable!(),
            };
            this.extend(quote!(#phantom: askama::helpers::core::marker::PhantomData {},));
        }

        Fields::Unnamed(fields) => {
            for (idx, field) in fields.unnamed.iter().enumerate() {
                let span = field.ident.span();
                let arg = Ident::new(&format!("__askama_arg_{idx}"), span);
                let idx = syn::LitInt::new(&format!("{idx}"), span);
                src.extend(quote!(#idx: ref #arg,));
                this.extend(quote!(#idx: #arg,));
            }
            let idx = syn::LitInt::new(&format!("{}", fields.unnamed.len()), span);
            this.extend(quote!(#idx: askama::helpers::core::marker::PhantomData {},));
        }

        Fields::Unit => {
            this.extend(quote!(0: askama::helpers::core::marker::PhantomData {},));
        }
    };
    render_into_arms.extend(quote! {
        Self :: #ident { #src } => {
            <_ as askama::helpers::EnumVariantTemplate>::render_into_with_values(
                & #var_id #ty_generics { #this },
                __askama_writer,
                __askama_values,
            )
        }
    });
    size_hint_arms.extend(quote! {
        Self :: #ident { .. } => {
            #size_hint
        }
    });
}
