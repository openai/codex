use std::borrow::Cow;
use std::collections::hash_map::{Entry, HashMap};
use std::fs::read_to_string;
use std::iter::FusedIterator;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, OnceLock};

use parser::node::Whitespace;
use parser::{Node, Parsed};
use proc_macro2::Span;
use rustc_hash::FxBuildHasher;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, ExprPath, Ident, Lit, LitBool, LitStr, Meta, Token};

use crate::config::{Config, SyntaxAndCache};
use crate::{CompileError, FileInfo, MsgValidEscapers, OnceMap};

#[derive(Clone)]
pub(crate) struct TemplateInput<'a> {
    pub(crate) ast: &'a syn::DeriveInput,
    pub(crate) enum_ast: Option<&'a syn::DeriveInput>,
    pub(crate) config: &'a Config,
    pub(crate) syntax: &'a SyntaxAndCache<'a>,
    pub(crate) source: &'a Source,
    pub(crate) source_span: Option<Span>,
    pub(crate) block: Option<(&'a str, Span)>,
    #[cfg(feature = "blocks")]
    pub(crate) blocks: &'a [Block],
    pub(crate) print: Print,
    pub(crate) escaper: &'a str,
    pub(crate) path: Arc<Path>,
    pub(crate) fields: Arc<[String]>,
}

impl TemplateInput<'_> {
    /// Extract the template metadata from the `DeriveInput` structure. This
    /// mostly recovers the data for the `TemplateInput` fields from the
    /// `template()` attribute list fields.
    pub(crate) fn new<'n>(
        ast: &'n syn::DeriveInput,
        enum_ast: Option<&'n syn::DeriveInput>,
        config: &'n Config,
        args: &'n TemplateArgs,
    ) -> Result<TemplateInput<'n>, CompileError> {
        let TemplateArgs {
            source: (source, source_span),
            block,
            #[cfg(feature = "blocks")]
            blocks,
            print,
            escaping,
            ext,
            ext_span,
            syntax,
            ..
        } = args;

        // Validate the `source` and `ext` value together, since they are
        // related. In case `source` was used instead of `path`, the value
        // of `ext` is merged into a synthetic `path` value here.
        let path = match (&source, &ext) {
            (Source::Path(path), _) => config.find_template(path, None, None)?,
            (&Source::Source(_), Some(ext)) => {
                PathBuf::from(format!("{}.{}", ast.ident, ext)).into()
            }
            (&Source::Source(_), None) => {
                return Err(CompileError::no_file_info(
                    #[cfg(not(feature = "code-in-doc"))]
                    "must include `ext` attribute when using `source` attribute",
                    #[cfg(feature = "code-in-doc")]
                    "must include `ext` attribute when using `source` or `in_doc` attribute",
                    None,
                ));
            }
        };

        // Validate syntax
        let syntax = syntax.as_deref().map_or_else(
            || Ok(config.syntaxes.get(config.default_syntax).unwrap()),
            |s| {
                config.syntaxes.get(s).ok_or_else(|| {
                    CompileError::no_file_info(format_args!("syntax `{s}` is undefined"), None)
                })
            },
        )?;

        // Match extension against defined output formats

        let escaping = escaping
            .as_deref()
            .or_else(|| path.extension().and_then(|s| s.to_str()))
            .unwrap_or_default();

        let escaper = config
            .escapers
            .iter()
            .find_map(|(extensions, path)| {
                extensions
                    .contains(&Cow::Borrowed(escaping))
                    .then_some(path.as_ref())
            })
            .ok_or_else(|| {
                CompileError::no_file_info(
                    format_args!(
                        "no escaper defined for extension '{escaping}'. You can define an escaper \
                        in the config file (named `askama.toml` by default). {}",
                        MsgValidEscapers(&config.escapers),
                    ),
                    *ext_span,
                )
            })?;

        let empty_punctuated = Punctuated::new();
        let fields = match ast.data {
            syn::Data::Struct(ref struct_) => {
                if let syn::Fields::Named(ref fields) = &struct_.fields {
                    &fields.named
                } else {
                    &empty_punctuated
                }
            }
            syn::Data::Union(ref union_) => &union_.fields.named,
            syn::Data::Enum(_) => &empty_punctuated,
        }
        .iter()
        .map(|f| match &f.ident {
            Some(ident) => ident.to_string(),
            None => unreachable!("we checked that we are using a struct"),
        })
        .collect::<Vec<_>>();

        Ok(TemplateInput {
            ast,
            enum_ast,
            config,
            syntax,
            source,
            source_span: *source_span,
            block: block.as_ref().map(|(block, span)| (block.as_str(), *span)),
            #[cfg(feature = "blocks")]
            blocks: blocks.as_slice(),
            print: *print,
            escaper,
            path,
            fields: fields.into(),
        })
    }

    pub(crate) fn find_used_templates(
        &self,
        map: &mut HashMap<Arc<Path>, Arc<Parsed>, FxBuildHasher>,
    ) -> Result<(), CompileError> {
        let (source, source_path) = match &self.source {
            Source::Source(s) => (s.clone(), None),
            Source::Path(_) => (
                get_template_source(&self.path, None)?,
                Some(Arc::clone(&self.path)),
            ),
        };

        let mut dependency_graph = Vec::new();
        let mut check = vec![(Arc::clone(&self.path), source, source_path)];
        while let Some((path, source, source_path)) = check.pop() {
            let parsed = match self.syntax.parse(Arc::clone(&source), source_path) {
                Ok(parsed) => parsed,
                Err(err) => {
                    let msg = err
                        .message
                        .unwrap_or_else(|| "failed to parse template source".into());
                    let file_path = err
                        .file_path
                        .as_deref()
                        .unwrap_or(Path::new("<source attribute>"));
                    let file_info =
                        FileInfo::new(file_path, Some(&source), Some(&source[err.offset..]));
                    return Err(CompileError::new(msg, Some(file_info)));
                }
            };

            let mut top = true;
            let mut nested = vec![parsed.nodes()];
            while let Some(nodes) = nested.pop() {
                for n in nodes {
                    let mut add_to_check = |new_path: Arc<Path>| -> Result<(), CompileError> {
                        if let Entry::Vacant(e) = map.entry(new_path) {
                            // Add a dummy entry to `map` in order to prevent adding `path`
                            // multiple times to `check`.
                            let new_path = e.key();
                            let source = parsed.source();
                            let source = get_template_source(
                                new_path,
                                Some((
                                    &path,
                                    source,
                                    n.span().as_suffix_of(source).unwrap_or_default(),
                                )),
                            )?;
                            check.push((new_path.clone(), source, Some(new_path.clone())));
                            e.insert(Arc::default());
                        }
                        Ok(())
                    };

                    match n {
                        Node::Extends(extends) if top => {
                            let extends = self.config.find_template(
                                extends.path,
                                Some(&path),
                                Some(FileInfo::of(extends.span(), &path, &parsed)),
                            )?;
                            let dependency_path = (path.clone(), extends.clone());
                            if path == extends {
                                // We add the path into the graph to have a better looking error.
                                dependency_graph.push(dependency_path);
                                return cyclic_graph_error(&dependency_graph);
                            } else if dependency_graph.contains(&dependency_path) {
                                return cyclic_graph_error(&dependency_graph);
                            }
                            dependency_graph.push(dependency_path);
                            add_to_check(extends)?;
                        }
                        Node::Macro(m) if top => {
                            nested.push(&m.nodes);
                        }
                        Node::Import(import) if top => {
                            let import = self.config.find_template(
                                import.path,
                                Some(&path),
                                Some(FileInfo::of(import.span(), &path, &parsed)),
                            )?;
                            add_to_check(import)?;
                        }
                        Node::FilterBlock(f) => {
                            nested.push(&f.nodes);
                        }
                        Node::Include(include) => {
                            let include = self.config.find_template(
                                include.path,
                                Some(&path),
                                Some(FileInfo::of(include.span(), &path, &parsed)),
                            )?;
                            add_to_check(include)?;
                        }
                        Node::BlockDef(b) => {
                            nested.push(&b.nodes);
                        }
                        Node::If(i) => {
                            for cond in &i.branches {
                                nested.push(&cond.nodes);
                            }
                        }
                        Node::Loop(l) => {
                            nested.push(&l.body);
                            nested.push(&l.else_nodes);
                        }
                        Node::Match(m) => {
                            for arm in &m.arms {
                                nested.push(&arm.nodes);
                            }
                        }
                        Node::Lit(_)
                        | Node::Comment(_)
                        | Node::Expr(_, _)
                        | Node::Call(_)
                        | Node::Extends(_)
                        | Node::Let(_)
                        | Node::Import(_)
                        | Node::Macro(_)
                        | Node::Raw(_)
                        | Node::Continue(_)
                        | Node::Break(_) => {}
                    }
                }
                top = false;
            }
            map.insert(path, parsed);
        }
        Ok(())
    }
}

pub(crate) enum AnyTemplateArgs {
    Struct(TemplateArgs),
    Enum {
        enum_args: Option<PartialTemplateArgs>,
        vars_args: Vec<Option<PartialTemplateArgs>>,
        has_default_impl: bool,
    },
}

impl AnyTemplateArgs {
    pub(crate) fn new(ast: &syn::DeriveInput) -> Result<Self, CompileError> {
        let syn::Data::Enum(enum_data) = &ast.data else {
            return Ok(Self::Struct(TemplateArgs::new(ast)?));
        };

        let enum_args = PartialTemplateArgs::new(ast, &ast.attrs, false)?;
        let vars_args = enum_data
            .variants
            .iter()
            .map(|variant| PartialTemplateArgs::new(ast, &variant.attrs, true))
            .collect::<Result<Vec<_>, _>>()?;
        if vars_args.is_empty() {
            return Ok(Self::Struct(TemplateArgs::from_partial(ast, enum_args)?));
        }

        let mut needs_default_impl = vars_args.len();
        let enum_source = enum_args.as_ref().and_then(|v| v.source.as_ref());
        for (variant, var_args) in enum_data.variants.iter().zip(&vars_args) {
            if var_args
                .as_ref()
                .and_then(|v| v.source.as_ref())
                .or(enum_source)
                .is_none()
            {
                return Err(CompileError::new_with_span(
                    #[cfg(not(feature = "code-in-doc"))]
                    "either all `enum` variants need a `path` or `source` argument, \
                    or the `enum` itself needs a default implementation",
                    #[cfg(feature = "code-in-doc")]
                    "either all `enum` variants need a `path`, `source` or `in_doc` argument, \
                    or the `enum` itself needs a default implementation",
                    None,
                    Some(variant.ident.span()),
                ));
            } else if !var_args.is_none() {
                needs_default_impl -= 1;
            }
        }

        Ok(Self::Enum {
            enum_args,
            vars_args,
            has_default_impl: needs_default_impl > 0,
        })
    }

    pub(crate) fn take_crate_name(&mut self) -> Option<ExprPath> {
        match self {
            AnyTemplateArgs::Struct(template_args) => template_args.crate_name.take(),
            AnyTemplateArgs::Enum { enum_args, .. } => {
                if let Some(PartialTemplateArgs { crate_name, .. }) = enum_args {
                    crate_name.take()
                } else {
                    None
                }
            }
        }
    }
}

#[cfg(feature = "blocks")]
pub(crate) struct Block {
    pub(crate) name: String,
    pub(crate) span: Span,
}

pub(crate) struct TemplateArgs {
    pub(crate) source: (Source, Option<Span>),
    block: Option<(String, Span)>,
    #[cfg(feature = "blocks")]
    blocks: Vec<Block>,
    print: Print,
    escaping: Option<String>,
    ext: Option<String>,
    ext_span: Option<Span>,
    syntax: Option<String>,
    config: Option<String>,
    crate_name: Option<ExprPath>,
    pub(crate) whitespace: Option<Whitespace>,
    pub(crate) template_span: Option<Span>,
    pub(crate) config_span: Option<Span>,
}

impl TemplateArgs {
    pub(crate) fn new(ast: &syn::DeriveInput) -> Result<Self, CompileError> {
        Self::from_partial(ast, PartialTemplateArgs::new(ast, &ast.attrs, false)?)
    }

    pub(crate) fn from_partial(
        ast: &syn::DeriveInput,
        args: Option<PartialTemplateArgs>,
    ) -> Result<Self, CompileError> {
        let Some(args) = args else {
            return Err(CompileError::new_with_span(
                "no attribute `template` found",
                None,
                Some(ast.ident.span()),
            ));
        };
        Ok(Self {
            source: match args.source {
                Some(PartialTemplateArgsSource::Path(s)) => {
                    (Source::Path(s.value().into()), Some(s.span()))
                }
                Some(PartialTemplateArgsSource::Source(s)) => {
                    (Source::Source(s.value().into()), Some(s.span()))
                }
                #[cfg(feature = "code-in-doc")]
                Some(PartialTemplateArgsSource::InDoc(span, source)) => (source, Some(span)),
                None => {
                    return Err(CompileError::no_file_info(
                        #[cfg(not(feature = "code-in-doc"))]
                        "specify one template argument `path` or `source`",
                        #[cfg(feature = "code-in-doc")]
                        "specify one template argument `path`, `source` or `in_doc`",
                        Some(args.template.span()),
                    ));
                }
            },
            block: args.block.map(|value| (value.value(), value.span())),
            #[cfg(feature = "blocks")]
            blocks: args
                .blocks
                .unwrap_or_default()
                .into_iter()
                .map(|value| Block {
                    name: value.value(),
                    span: value.span(),
                })
                .collect(),
            print: args.print.unwrap_or_default(),
            escaping: args.escape.map(|value| value.value()),
            ext: args.ext.as_ref().map(|value| value.value()),
            ext_span: args.ext.as_ref().map(|value| value.span()),
            syntax: args.syntax.map(|value| value.value()),
            config: args.config.as_ref().map(|value| value.value()),
            crate_name: args.crate_name,
            whitespace: args.whitespace,
            template_span: Some(args.template.span()),
            config_span: args.config.as_ref().map(|value| value.span()),
        })
    }

    pub(crate) fn fallback() -> Self {
        Self {
            source: (Source::Source("".into()), None),
            block: None,
            #[cfg(feature = "blocks")]
            blocks: vec![],
            print: Print::default(),
            escaping: None,
            ext: Some("txt".to_string()),
            ext_span: None,
            syntax: None,
            config: None,
            crate_name: None,
            whitespace: None,
            template_span: None,
            config_span: None,
        }
    }

    pub(crate) fn config_path(&self) -> Option<&str> {
        self.config.as_deref()
    }
}

/// Try to find the source in the comment, in a `askama` code block.
///
/// This is only done if no path or source was given in the `#[template]` attribute.
#[cfg(feature = "code-in-doc")]
fn source_from_docs(
    span: Span,
    docs: &[&Attribute],
    ast: &syn::DeriveInput,
) -> Result<(Source, Option<Span>), CompileError> {
    let (source_span, source) = collect_comment_blocks(span, docs, ast)?;
    let source = strip_common_ws_prefix(source);
    let source = collect_askama_code_blocks(span, ast, source)?;
    Ok((source, source_span))
}

#[cfg(feature = "code-in-doc")]
fn collect_comment_blocks(
    span: Span,
    docs: &[&Attribute],
    ast: &syn::DeriveInput,
) -> Result<(Option<Span>, String), CompileError> {
    let mut source_span: Option<Span> = None;
    let mut assign_span = |kv: &syn::MetaNameValue| {
        // FIXME: uncomment once <https://github.com/rust-lang/rust/issues/54725> is stable
        // let new_span = kv.path.span();
        // source_span = Some(match source_span {
        //     Some(cur_span) => cur_span.join(new_span).unwrap_or(cur_span),
        //     None => new_span,
        // });

        if source_span.is_none() {
            source_span = Some(kv.path.span());
        }
    };

    let mut source = String::new();
    for a in docs {
        // is a comment?
        let Meta::NameValue(kv) = &a.meta else {
            continue;
        };
        if !kv.path.is_ident("doc") {
            continue;
        }

        // is an understood comment, e.g. not `#[doc = inline_str(…)]`
        let mut value = &kv.value;
        let value = loop {
            match value {
                Expr::Lit(lit) => break lit,
                Expr::Group(group) => value = &group.expr,
                _ => continue,
            }
        };
        let Lit::Str(value) = &value.lit else {
            continue;
        };

        assign_span(kv);
        source.push_str(value.value().as_str());
        source.push('\n');
    }
    if source.is_empty() {
        return Err(no_askama_code_block(span, ast));
    }

    Ok((source_span, source))
}

#[cfg(feature = "code-in-doc")]
fn no_askama_code_block(span: Span, ast: &syn::DeriveInput) -> CompileError {
    let kind = match &ast.data {
        syn::Data::Struct(_) => "struct",
        syn::Data::Enum(_) => "enum",
        // actually unreachable: `union`s are rejected by `TemplateArgs::new()`
        syn::Data::Union(_) => "union",
    };
    CompileError::no_file_info(
        format_args!(
            "when using `in_doc` with the value `true`, the {kind}'s documentation needs a \
             `askama` code block"
        ),
        Some(span),
    )
}

#[cfg(feature = "code-in-doc")]
fn strip_common_ws_prefix(source: String) -> String {
    let mut common_prefix_iter = source
        .lines()
        .filter_map(|s| Some(&s[..s.find(|c: char| !c.is_ascii_whitespace())?]));
    let mut common_prefix = common_prefix_iter.next().unwrap_or_default();
    for p in common_prefix_iter {
        if common_prefix.is_empty() {
            break;
        }
        let ((pos, _), _) = common_prefix
            .char_indices()
            .zip(p.char_indices())
            .take_while(|(l, r)| l == r)
            .last()
            .unwrap_or_default();
        common_prefix = &common_prefix[..pos];
    }
    if common_prefix.is_empty() {
        return source;
    }

    source
        .lines()
        .flat_map(|s| [s.get(common_prefix.len()..).unwrap_or_default(), "\n"])
        .collect()
}

#[cfg(feature = "code-in-doc")]
fn collect_askama_code_blocks(
    span: Span,
    ast: &syn::DeriveInput,
    source: String,
) -> Result<Source, CompileError> {
    use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

    let mut tmpl_source = String::new();
    let mut in_askama_code = false;
    let mut had_askama_code = false;
    for e in Parser::new(&source) {
        match (in_askama_code, e) {
            (false, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(s)))) => {
                if s.split(",")
                    .any(|s| JINJA_EXTENSIONS.contains(&s.trim_ascii()))
                {
                    in_askama_code = true;
                    had_askama_code = true;
                }
            }
            (true, Event::End(TagEnd::CodeBlock)) => in_askama_code = false,
            (true, Event::Text(text)) => tmpl_source.push_str(&text),
            _ => {}
        }
    }
    if !had_askama_code {
        return Err(no_askama_code_block(span, ast));
    }

    if tmpl_source.ends_with('\n') {
        tmpl_source.pop();
    }
    Ok(Source::Source(tmpl_source.into()))
}

struct ResultIter<I, E>(Result<I, Option<E>>);

impl<I: IntoIterator, E> From<Result<I, E>> for ResultIter<I::IntoIter, E> {
    fn from(value: Result<I, E>) -> Self {
        Self(match value {
            Ok(i) => Ok(i.into_iter()),
            Err(e) => Err(Some(e)),
        })
    }
}

impl<I: Iterator, E> Iterator for ResultIter<I, E> {
    type Item = Result<I::Item, E>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            Ok(iter) => Some(Ok(iter.next()?)),
            Err(err) => Some(Err(err.take()?)),
        }
    }
}

impl<I: FusedIterator, E> FusedIterator for ResultIter<I, E> {}

#[derive(Debug, Clone, Hash, PartialEq)]
pub(crate) enum Source {
    Path(Arc<str>),
    Source(Arc<str>),
}

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub(crate) enum Print {
    All,
    Ast,
    Code,
    None,
}

impl Default for Print {
    fn default() -> Self {
        Self::None
    }
}

impl FromStr for Print {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "ast" => Ok(Self::Ast),
            "code" => Ok(Self::Code),
            "none" => Ok(Self::None),
            _ => Err(format!("invalid value for `print` option: {s}")),
        }
    }
}

fn cyclic_graph_error(dependency_graph: &[(Arc<Path>, Arc<Path>)]) -> Result<(), CompileError> {
    Err(CompileError::no_file_info(
        format_args!(
            "cyclic dependency in graph {:#?}",
            dependency_graph
                .iter()
                .map(|e| format!("{:#?} --> {:#?}", e.0, e.1))
                .collect::<Vec<String>>()
        ),
        None,
    ))
}

pub(crate) fn get_template_source(
    tpl_path: &Arc<Path>,
    import_from: Option<(&Arc<Path>, &str, &str)>,
) -> Result<Arc<str>, CompileError> {
    static CACHE: OnceLock<OnceMap<Arc<Path>, Arc<str>>> = OnceLock::new();

    CACHE.get_or_init(OnceMap::default).get_or_try_insert(
        tpl_path,
        |tpl_path| match read_to_string(tpl_path) {
            Ok(mut source) => {
                if source.ends_with('\n') {
                    let _ = source.pop();
                }
                Ok((Arc::clone(tpl_path), Arc::from(source)))
            }
            Err(err) => Err(CompileError::new(
                format_args!(
                    "unable to open template file '{}': {err}",
                    tpl_path.to_str().unwrap(),
                ),
                import_from.map(|(node_file, file_source, node_source)| {
                    FileInfo::new(node_file, Some(file_source), Some(node_source))
                }),
            )),
        },
        Arc::clone,
    )
}

pub(crate) struct PartialTemplateArgs {
    pub(crate) template: Ident,
    pub(crate) source: Option<PartialTemplateArgsSource>,
    pub(crate) block: Option<LitStr>,
    pub(crate) print: Option<Print>,
    pub(crate) escape: Option<LitStr>,
    pub(crate) ext: Option<LitStr>,
    pub(crate) syntax: Option<LitStr>,
    pub(crate) config: Option<LitStr>,
    pub(crate) whitespace: Option<Whitespace>,
    pub(crate) crate_name: Option<ExprPath>,
    #[cfg(feature = "blocks")]
    pub(crate) blocks: Option<Vec<LitStr>>,
}

#[derive(Clone)]
pub(crate) enum PartialTemplateArgsSource {
    Path(LitStr),
    Source(LitStr),
    #[cfg(feature = "code-in-doc")]
    InDoc(Span, Source),
}

impl PartialTemplateArgsSource {
    pub(crate) fn span(&self) -> Span {
        match self {
            Self::Path(s) => s.span(),
            Self::Source(s) => s.span(),
            #[cfg(feature = "code-in-doc")]
            Self::InDoc(s, _) => s.span(),
        }
    }
}

// implement PartialTemplateArgs::new()
const _: () = {
    impl PartialTemplateArgs {
        pub(crate) fn new(
            ast: &syn::DeriveInput,
            attrs: &[Attribute],
            is_enum_variant: bool,
        ) -> Result<Option<Self>, CompileError> {
            new(ast, attrs, is_enum_variant)
        }
    }

    #[inline]
    fn new(
        ast: &syn::DeriveInput,
        attrs: &[Attribute],
        is_enum_variant: bool,
    ) -> Result<Option<PartialTemplateArgs>, CompileError> {
        // FIXME: implement once <https://github.com/rust-lang/rfcs/pull/3715> is stable
        if let syn::Data::Union(data) = &ast.data {
            return Err(CompileError::new_with_span(
                "askama templates are not supported for `union` types, only `struct` and `enum`",
                None,
                Some(data.union_token.span),
            ));
        }

        #[cfg(feature = "code-in-doc")]
        let mut meta_docs = vec![];

        let mut this = PartialTemplateArgs {
            template: Ident::new("template", Span::call_site()),
            source: None,
            block: None,
            print: None,
            escape: None,
            ext: None,
            syntax: None,
            config: None,
            whitespace: None,
            crate_name: None,
            #[cfg(feature = "blocks")]
            blocks: None,
        };
        let mut has_data = false;

        for attr in attrs {
            let Some(ident) = attr.path().get_ident() else {
                continue;
            };
            if ident == "template" {
                this.template = ident.clone();
                has_data = true;
            } else {
                #[cfg(feature = "code-in-doc")]
                if ident == "doc" {
                    meta_docs.push(attr);
                }
                continue;
            }

            let args = attr
                .parse_args_with(<Punctuated<Meta, Token![,]>>::parse_terminated)
                .map_err(|e| {
                    CompileError::no_file_info(
                        format_args!("unable to parse template arguments: {e}"),
                        Some(attr.path().span()),
                    )
                })?;
            for arg in args {
                let pair = match arg {
                    Meta::NameValue(pair) => pair,
                    v => {
                        return Err(CompileError::no_file_info(
                            "unsupported attribute argument",
                            Some(v.span()),
                        ));
                    }
                };
                let ident = match pair.path.get_ident() {
                    Some(ident) => ident,
                    None => unreachable!("not possible in syn::Meta::NameValue(…)"),
                };

                if ident == "askama" {
                    if is_enum_variant {
                        return Err(CompileError::no_file_info(
                            "template attribute `askama` can only be used on the `enum`, \
                            not its variants",
                            Some(ident.span()),
                        ));
                    }
                    ensure_only_once(ident, &mut this.crate_name)?;
                    this.crate_name = Some(get_exprpath(ident, pair.value)?);
                    continue;
                } else if ident == "blocks" {
                    if !cfg!(feature = "blocks") {
                        return Err(CompileError::no_file_info(
                            "enable feature `blocks` to use `blocks` argument",
                            Some(ident.span()),
                        ));
                    } else if is_enum_variant {
                        return Err(CompileError::no_file_info(
                            "template attribute `blocks` can only be used on the `enum`, \
                            not its variants",
                            Some(ident.span()),
                        ));
                    }
                    #[cfg(feature = "blocks")]
                    {
                        ensure_only_once(ident, &mut this.blocks)?;
                        this.blocks = Some(
                            get_exprarray(ident, pair.value)?
                                .elems
                                .into_iter()
                                .map(|value| get_strlit(ident, get_lit(ident, value)?))
                                .collect::<Result<_, _>>()?,
                        );
                        continue;
                    }
                }

                let value = get_lit(ident, pair.value)?;

                if ident == "path" {
                    ensure_source_only_once(ident, &this.source)?;
                    this.source = Some(PartialTemplateArgsSource::Path(get_strlit(ident, value)?));
                } else if ident == "source" {
                    ensure_source_only_once(ident, &this.source)?;
                    this.source =
                        Some(PartialTemplateArgsSource::Source(get_strlit(ident, value)?));
                } else if ident == "in_doc" {
                    let value = get_boollit(ident, value)?;
                    if !value.value() {
                        continue;
                    }
                    ensure_source_only_once(ident, &this.source)?;

                    #[cfg(not(feature = "code-in-doc"))]
                    {
                        return Err(CompileError::no_file_info(
                            "enable feature `code-in-doc` to use `in_doc` argument",
                            Some(ident.span()),
                        ));
                    }
                    #[cfg(feature = "code-in-doc")]
                    {
                        this.source = Some(PartialTemplateArgsSource::InDoc(
                            value.span(),
                            Source::Path("".into()),
                        ));
                    }
                } else if ident == "block" {
                    set_strlit_pair(ident, value, &mut this.block)?;
                } else if ident == "print" {
                    set_parseable_string(ident, value, &mut this.print)?;
                } else if ident == "escape" {
                    set_strlit_pair(ident, value, &mut this.escape)?;
                } else if ident == "ext" {
                    set_strlit_pair(ident, value, &mut this.ext)?;
                } else if ident == "syntax" {
                    set_strlit_pair(ident, value, &mut this.syntax)?;
                } else if ident == "config" {
                    set_strlit_pair(ident, value, &mut this.config)?;
                } else if ident == "whitespace" {
                    set_parseable_string(ident, value, &mut this.whitespace)?;
                } else {
                    return Err(CompileError::no_file_info(
                        format_args!("unsupported template attribute `{ident}` found"),
                        Some(ident.span()),
                    ));
                }
            }
        }
        if !has_data {
            return Ok(None);
        }

        #[cfg(feature = "code-in-doc")]
        if let Some(PartialTemplateArgsSource::InDoc(lit_span, _)) = this.source {
            let (source, doc_span) = source_from_docs(lit_span, &meta_docs, ast)?;
            this.source = Some(PartialTemplateArgsSource::InDoc(
                doc_span.unwrap_or(lit_span),
                source,
            ));
        }

        Ok(Some(this))
    }

    fn set_strlit_pair(
        name: &Ident,
        value: ExprLit,
        dest: &mut Option<LitStr>,
    ) -> Result<(), CompileError> {
        ensure_only_once(name, dest)?;
        *dest = Some(get_strlit(name, value)?);
        Ok(())
    }

    fn set_parseable_string<T: FromStr<Err: ToString>>(
        name: &Ident,
        value: ExprLit,
        dest: &mut Option<T>,
    ) -> Result<(), CompileError> {
        ensure_only_once(name, dest)?;
        let str_value = get_strlit(name, value)?;
        *dest = Some(
            str_value
                .value()
                .parse()
                .map_err(|msg| CompileError::no_file_info(msg, Some(str_value.span())))?,
        );
        Ok(())
    }

    fn ensure_only_once<T>(name: &Ident, dest: &mut Option<T>) -> Result<(), CompileError> {
        if dest.is_none() {
            Ok(())
        } else {
            Err(CompileError::no_file_info(
                format_args!("template attribute `{name}` already set"),
                Some(name.span()),
            ))
        }
    }

    fn get_lit(name: &Ident, mut expr: Expr) -> Result<ExprLit, CompileError> {
        loop {
            match expr {
                Expr::Lit(lit) => return Ok(lit),
                Expr::Group(group) => expr = *group.expr,
                v => {
                    return Err(CompileError::no_file_info(
                        format_args!("template attribute `{name}` expects a literal"),
                        Some(v.span()),
                    ));
                }
            }
        }
    }

    fn get_strlit(name: &Ident, value: ExprLit) -> Result<LitStr, CompileError> {
        if let Lit::Str(s) = value.lit {
            Ok(s)
        } else {
            Err(CompileError::no_file_info(
                format_args!("template attribute `{name}` expects a string literal"),
                Some(value.lit.span()),
            ))
        }
    }

    fn get_boollit(name: &Ident, value: ExprLit) -> Result<LitBool, CompileError> {
        if let Lit::Bool(s) = value.lit {
            Ok(s)
        } else {
            Err(CompileError::no_file_info(
                format_args!("template attribute `{name}` expects a boolean value"),
                Some(value.lit.span()),
            ))
        }
    }

    fn get_exprpath(name: &Ident, mut expr: Expr) -> Result<ExprPath, CompileError> {
        loop {
            match expr {
                Expr::Path(path) => return Ok(path),
                Expr::Group(group) => expr = *group.expr,
                v => {
                    return Err(CompileError::no_file_info(
                        format_args!("template attribute `{name}` expects a path or identifier"),
                        Some(v.span()),
                    ));
                }
            }
        }
    }

    #[cfg(feature = "blocks")]
    fn get_exprarray(name: &Ident, mut expr: Expr) -> Result<syn::ExprArray, CompileError> {
        loop {
            match expr {
                Expr::Array(array) => return Ok(array),
                Expr::Group(group) => expr = *group.expr,
                v => {
                    return Err(CompileError::no_file_info(
                        format_args!("template attribute `{name}` expects an array"),
                        Some(v.span()),
                    ));
                }
            }
        }
    }

    fn ensure_source_only_once(
        name: &Ident,
        source: &Option<PartialTemplateArgsSource>,
    ) -> Result<(), CompileError> {
        if source.is_some() {
            return Err(CompileError::no_file_info(
                #[cfg(feature = "code-in-doc")]
                "must specify `source`, `path` or `is_doc` exactly once",
                #[cfg(not(feature = "code-in-doc"))]
                "must specify `source` or `path` exactly once",
                Some(name.span()),
            ));
        }
        Ok(())
    }
};

#[cfg(feature = "code-in-doc")]
const JINJA_EXTENSIONS: &[&str] = &["askama", "j2", "jinja", "jinja2", "rinja"];

#[test]
fn get_source() {
    let path = Config::new("", None, None, None, None)
        .and_then(|config| config.find_template("b.html", None, None))
        .unwrap();
    assert_eq!(get_template_source(&path, None).unwrap(), "bar".into());
}
