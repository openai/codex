use std::borrow::Cow;

use parser::node::CondTest;
use parser::{Attr, CharLit, CharPrefix, Expr, Filter, Span, StrLit, Target, TyGenerics, WithSpan};
use quote::quote;

use super::{
    DisplayWrap, FILTER_SOURCE, Generator, LocalMeta, Writable, compile_time_escape, is_copyable,
    normalize_identifier,
};
use crate::CompileError;
use crate::heritage::Context;
use crate::integration::Buffer;

impl<'a> Generator<'a, '_> {
    pub(crate) fn visit_expr_root(
        &mut self,
        ctx: &Context<'_>,
        expr: &WithSpan<'a, Expr<'a>>,
    ) -> Result<String, CompileError> {
        let mut buf = Buffer::new();
        self.visit_expr(ctx, &mut buf, expr)?;
        Ok(buf.into_string())
    }

    pub(super) fn visit_expr(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'a, Expr<'a>>,
    ) -> Result<DisplayWrap, CompileError> {
        Ok(match **expr {
            Expr::BoolLit(s) => self.visit_bool_lit(buf, s),
            Expr::NumLit(s, _) => self.visit_num_lit(buf, s),
            Expr::StrLit(ref s) => self.visit_str_lit(buf, s),
            Expr::CharLit(ref s) => self.visit_char_lit(buf, s),
            Expr::Var(s) => self.visit_var(buf, s),
            Expr::Path(ref path) => self.visit_path(buf, path),
            Expr::Array(ref elements) => self.visit_array(ctx, buf, elements)?,
            Expr::Attr(ref obj, ref attr) => self.visit_attr(ctx, buf, obj, attr)?,
            Expr::Index(ref obj, ref key) => self.visit_index(ctx, buf, obj, key)?,
            Expr::Filter(Filter {
                name,
                ref arguments,
                ref generics,
            }) => self.visit_filter(ctx, buf, name, arguments, generics, expr.span())?,
            Expr::Unary(op, ref inner) => self.visit_unary(ctx, buf, op, inner)?,
            Expr::BinOp(op, ref left, ref right) => self.visit_binop(ctx, buf, op, left, right)?,
            Expr::Range(op, ref left, ref right) => {
                self.visit_range(ctx, buf, op, left.as_deref(), right.as_deref())?
            }
            Expr::Group(ref inner) => self.visit_group(ctx, buf, inner)?,
            Expr::Call {
                ref path,
                ref args,
                ref generics,
            } => self.visit_call(ctx, buf, path, args, generics)?,
            Expr::RustMacro(ref path, args) => self.visit_rust_macro(buf, path, args),
            Expr::Try(ref expr) => self.visit_try(ctx, buf, expr)?,
            Expr::Tuple(ref exprs) => self.visit_tuple(ctx, buf, exprs)?,
            Expr::NamedArgument(_, ref expr) => self.visit_named_argument(ctx, buf, expr)?,
            Expr::FilterSource => self.visit_filter_source(buf),
            Expr::IsDefined(var_name) => self.visit_is_defined(buf, true, var_name)?,
            Expr::IsNotDefined(var_name) => self.visit_is_defined(buf, false, var_name)?,
            Expr::As(ref expr, target) => self.visit_as(ctx, buf, expr, target)?,
            Expr::Concat(ref exprs) => self.visit_concat(ctx, buf, exprs)?,
            Expr::LetCond(ref cond) => self.visit_let_cond(ctx, buf, cond)?,
            Expr::ArgumentPlaceholder => DisplayWrap::Unwrapped,
        })
    }

    /// This method and `visit_expr_not_first` are needed because in case we have
    /// `{% if let Some(x) = x && x == "a" %}`, if we first start to visit `Some(x)`, then we end
    /// up with `if let Some(x) = x && x == "a"`, however if we first visit the expr, we end up with
    /// `if let Some(x) = self.x && self.x == "a"`. It's all a big "variable declaration" mess.
    ///
    /// So instead, we first visit the expression, but only the first "level" to ensure we won't
    /// go after the `&&` and badly generate the rest of the expression.
    pub(super) fn visit_expr_first(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'a, Expr<'a>>,
    ) -> Result<DisplayWrap, CompileError> {
        match **expr {
            Expr::BinOp(op @ ("||" | "&&"), ref left, _) => {
                let ret = self.visit_expr(ctx, buf, left)?;
                buf.write(format_args!(" {op} "));
                return Ok(ret);
            }
            Expr::Unary(op, ref inner) => {
                buf.write(op);
                return self.visit_expr_first(ctx, buf, inner);
            }
            _ => {}
        }
        self.visit_expr(ctx, buf, expr)
    }

    pub(super) fn visit_expr_not_first(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'a, Expr<'a>>,
        prev_display_wrap: DisplayWrap,
    ) -> Result<DisplayWrap, CompileError> {
        match **expr {
            Expr::BinOp("||" | "&&", _, ref right) => {
                self.visit_condition(ctx, buf, right)?;
                Ok(DisplayWrap::Unwrapped)
            }
            Expr::Unary(_, ref inner) => {
                self.visit_expr_not_first(ctx, buf, inner, prev_display_wrap)
            }
            _ => Ok(prev_display_wrap),
        }
    }

    pub(super) fn visit_condition(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'a, Expr<'a>>,
    ) -> Result<(), CompileError> {
        match &**expr {
            Expr::BoolLit(_) | Expr::IsDefined(_) | Expr::IsNotDefined(_) => {
                self.visit_expr(ctx, buf, expr)?;
            }
            Expr::Unary("!", expr) => {
                buf.write('!');
                self.visit_condition(ctx, buf, expr)?;
            }
            Expr::BinOp(op @ ("&&" | "||"), left, right) => {
                self.visit_condition(ctx, buf, left)?;
                buf.write(format_args!(" {op} "));
                self.visit_condition(ctx, buf, right)?;
            }
            Expr::Group(expr) => {
                buf.write('(');
                self.visit_condition(ctx, buf, expr)?;
                buf.write(')');
            }
            Expr::LetCond(cond) => {
                self.visit_let_cond(ctx, buf, cond)?;
            }
            _ => {
                buf.write("askama::helpers::as_bool(&(");
                self.visit_expr(ctx, buf, expr)?;
                buf.write("))");
            }
        }
        Ok(())
    }

    fn visit_is_defined(
        &mut self,
        buf: &mut Buffer,
        is_defined: bool,
        left: &str,
    ) -> Result<DisplayWrap, CompileError> {
        match (is_defined, self.is_var_defined(left)) {
            (true, true) | (false, false) => buf.write("true"),
            _ => buf.write("false"),
        }
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_as(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'a, Expr<'a>>,
        target: &str,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write("askama::helpers::get_primitive_value(&(");
        self.visit_expr(ctx, buf, expr)?;
        buf.write(format_args!(
            ")) as askama::helpers::core::primitive::{target}"
        ));
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_concat(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        exprs: &[WithSpan<'a, Expr<'a>>],
    ) -> Result<DisplayWrap, CompileError> {
        match exprs {
            [] => unreachable!(),
            [expr] => self.visit_expr(ctx, buf, expr),
            exprs => {
                let (l, r) = exprs.split_at(exprs.len().div_ceil(2));
                buf.write("askama::helpers::Concat(&(");
                self.visit_concat(ctx, buf, l)?;
                buf.write("), &(");
                self.visit_concat(ctx, buf, r)?;
                buf.write("))");
                Ok(DisplayWrap::Unwrapped)
            }
        }
    }

    fn visit_let_cond(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        cond: &WithSpan<'a, CondTest<'a>>,
    ) -> Result<DisplayWrap, CompileError> {
        let mut expr_buf = Buffer::new();
        let display_wrap = self.visit_expr_first(ctx, &mut expr_buf, &cond.expr)?;
        buf.write(" let ");
        if let Some(ref target) = cond.target {
            self.visit_target(buf, true, true, target);
        }
        buf.write(format_args!("= &{expr_buf}"));
        self.visit_expr_not_first(ctx, buf, &cond.expr, display_wrap)
    }

    fn visit_try(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'a, Expr<'a>>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write("match (");
        self.visit_expr(ctx, buf, expr)?;
        buf.write(
            ") { res => (&&askama::helpers::ErrorMarker::of(&res)).askama_conv_result(res)? }",
        );
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_rust_macro(&mut self, buf: &mut Buffer, path: &[&str], args: &str) -> DisplayWrap {
        self.visit_path(buf, path);
        buf.write("!(");
        buf.write(args);
        buf.write(')');

        DisplayWrap::Unwrapped
    }

    pub(super) fn visit_value(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        args: &[WithSpan<'a, Expr<'a>>],
        generics: &[WithSpan<'a, TyGenerics<'a>>],
        node: Span<'_>,
        kind: &str,
    ) -> Result<DisplayWrap, CompileError> {
        let [key] = args else {
            return Err(ctx.generate_error(
                format_args!("{kind} only takes one argument, found {}", args.len()),
                node,
            ));
        };
        let [gen] = generics else {
            return Err(ctx.generate_error(
                format_args!("{kind} expects one generic, found {}", generics.len()),
                node,
            ));
        };
        buf.write("askama::helpers::get_value");
        buf.write("::<");
        self.visit_ty_generic(buf, gen);
        buf.write('>');
        buf.write("(&__askama_values, &(");
        self.visit_arg(ctx, buf, key)?;
        buf.write("))");
        Ok(DisplayWrap::Unwrapped)
    }

    pub(super) fn visit_args(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        args: &[WithSpan<'a, Expr<'a>>],
    ) -> Result<(), CompileError> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                buf.write(',');
            }
            self.visit_arg(ctx, buf, arg)?;
        }
        Ok(())
    }

    pub(super) fn visit_arg(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        arg: &WithSpan<'a, Expr<'a>>,
    ) -> Result<(), CompileError> {
        self.visit_arg_inner(ctx, buf, arg, false)
    }

    fn visit_arg_inner(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        arg: &WithSpan<'a, Expr<'a>>,
        // This parameter is needed because even though Expr::Unary is not copyable, we might still
        // be able to skip a few levels.
        need_borrow: bool,
    ) -> Result<(), CompileError> {
        if let Expr::Unary(expr @ ("*" | "&"), ref arg) = **arg {
            buf.write(expr);
            return self.visit_arg_inner(ctx, buf, arg, true);
        }
        let borrow = need_borrow || !is_copyable(arg);
        if borrow {
            buf.write("&(");
        }
        match **arg {
            Expr::Call { ref path, .. } if !matches!(***path, Expr::Path(_)) => {
                buf.write('{');
                self.visit_expr(ctx, buf, arg)?;
                buf.write('}');
            }
            _ => {
                self.visit_expr(ctx, buf, arg)?;
            }
        }
        if borrow {
            buf.write(')');
        }
        Ok(())
    }

    pub(super) fn visit_auto_escaped_arg(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        arg: &WithSpan<'a, Expr<'a>>,
    ) -> Result<(), CompileError> {
        if let Some(Writable::Lit(arg)) = compile_time_escape(arg, self.input.escaper) {
            if !arg.is_empty() {
                buf.write("askama::filters::Safe(");
                buf.write_escaped_str(&arg);
                buf.write(')');
            } else {
                buf.write("askama::helpers::Empty");
            }
        } else {
            buf.write("(&&askama::filters::AutoEscaper::new(");
            self.visit_arg(ctx, buf, arg)?;
            buf.write(format_args!(
                ", {})).askama_auto_escape()?",
                self.input.escaper
            ));
        }
        Ok(())
    }

    pub(crate) fn visit_attr(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        obj: &WithSpan<'a, Expr<'a>>,
        attr: &Attr<'a>,
    ) -> Result<DisplayWrap, CompileError> {
        if let Expr::Var(name) = **obj {
            if name == "loop" {
                if attr.name == "index" {
                    buf.write("(_loop_item.index + 1)");
                    return Ok(DisplayWrap::Unwrapped);
                } else if attr.name == "index0" {
                    buf.write("_loop_item.index");
                    return Ok(DisplayWrap::Unwrapped);
                } else if attr.name == "first" {
                    buf.write("_loop_item.first");
                    return Ok(DisplayWrap::Unwrapped);
                } else if attr.name == "last" {
                    buf.write("_loop_item.last");
                    return Ok(DisplayWrap::Unwrapped);
                } else {
                    return Err(ctx.generate_error("unknown loop variable", obj.span()));
                }
            }
        }
        self.visit_expr(ctx, buf, obj)?;
        buf.write(format_args!(".{}", normalize_identifier(attr.name)));
        self.visit_call_generics(buf, &attr.generics);
        Ok(DisplayWrap::Unwrapped)
    }

    pub(super) fn visit_call_generics(
        &mut self,
        buf: &mut Buffer,
        generics: &[WithSpan<'a, TyGenerics<'a>>],
    ) {
        if generics.is_empty() {
            return;
        }
        buf.write("::");
        self.visit_ty_generics(buf, generics);
    }

    fn visit_ty_generics(&mut self, buf: &mut Buffer, generics: &[WithSpan<'a, TyGenerics<'a>>]) {
        if generics.is_empty() {
            return;
        }
        buf.write('<');
        for generic in generics {
            self.visit_ty_generic(buf, generic);
            buf.write(',');
        }
        buf.write('>');
    }

    pub(super) fn visit_ty_generic(
        &mut self,
        buf: &mut Buffer,
        generic: &WithSpan<'a, TyGenerics<'a>>,
    ) {
        let TyGenerics { refs, path, args } = &**generic;
        for _ in 0..*refs {
            buf.write('&');
        }
        self.visit_path(buf, path);
        self.visit_ty_generics(buf, args);
    }

    fn visit_index(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        obj: &WithSpan<'a, Expr<'a>>,
        key: &WithSpan<'a, Expr<'a>>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write('&');
        self.visit_expr(ctx, buf, obj)?;
        buf.write('[');
        self.visit_expr(ctx, buf, key)?;
        buf.write(']');
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_call(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        left: &WithSpan<'a, Expr<'a>>,
        args: &[WithSpan<'a, Expr<'a>>],
        generics: &[WithSpan<'a, TyGenerics<'a>>],
    ) -> Result<DisplayWrap, CompileError> {
        match &**left {
            Expr::Attr(sub_left, Attr { name, .. }) if ***sub_left == Expr::Var("loop") => {
                match *name {
                    "cycle" => {
                        if let [generic, ..] = generics {
                            return Err(ctx.generate_error(
                                "loop.cycle(…) doesn't use generics",
                                generic.span(),
                            ));
                        }
                        match args {
                            [arg] => {
                                if matches!(**arg, Expr::Array(ref arr) if arr.is_empty()) {
                                    return Err(ctx.generate_error(
                                        "loop.cycle(…) cannot use an empty array",
                                        arg.span(),
                                    ));
                                }
                                buf.write(
                                    "\
                                ({\
                                    let _cycle = &(",
                                );
                                self.visit_expr(ctx, buf, arg)?;
                                buf.write(
                                "\
                                    );\
                                    let _len = _cycle.len();\
                                    if _len == 0 {\
                                        return askama::helpers::core::result::Result::Err(askama::Error::Fmt);\
                                    }\
                                    _cycle[_loop_item.index % _len]\
                                })",
                            );
                            }
                            _ => {
                                return Err(ctx.generate_error(
                                    "loop.cycle(…) cannot use an empty array",
                                    left.span(),
                                ));
                            }
                        }
                    }
                    s => {
                        return Err(ctx.generate_error(
                            format_args!("unknown loop method: {s:?}"),
                            left.span(),
                        ));
                    }
                }
            }
            // We special-case "askama::get_value".
            Expr::Path(path) if path == &["askama", "get_value"] => {
                self.visit_value(
                    ctx,
                    buf,
                    args,
                    generics,
                    left.span(),
                    "`get_value` function",
                )?;
            }
            sub_left => {
                match sub_left {
                    Expr::Var(name) => match self.locals.resolve(name) {
                        Some(resolved) => buf.write(resolved),
                        None => buf.write(format_args!("self.{}", normalize_identifier(name))),
                    },
                    _ => {
                        self.visit_expr(ctx, buf, left)?;
                    }
                }
                self.visit_call_generics(buf, generics);
                buf.write('(');
                self.visit_args(ctx, buf, args)?;
                buf.write(')');
            }
        }
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_unary(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        op: &str,
        inner: &WithSpan<'a, Expr<'a>>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write(op);
        self.visit_expr(ctx, buf, inner)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_range(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        op: &str,
        left: Option<&WithSpan<'a, Expr<'a>>>,
        right: Option<&WithSpan<'a, Expr<'a>>>,
    ) -> Result<DisplayWrap, CompileError> {
        if let Some(left) = left {
            self.visit_expr(ctx, buf, left)?;
        }
        buf.write(op);
        if let Some(right) = right {
            self.visit_expr(ctx, buf, right)?;
        }
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_binop(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        op: &str,
        left: &WithSpan<'a, Expr<'a>>,
        right: &WithSpan<'a, Expr<'a>>,
    ) -> Result<DisplayWrap, CompileError> {
        self.visit_expr(ctx, buf, left)?;
        buf.write(format_args!(" {op} "));
        self.visit_expr(ctx, buf, right)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_group(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        inner: &WithSpan<'a, Expr<'a>>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write('(');
        self.visit_expr(ctx, buf, inner)?;
        buf.write(')');
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_tuple(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        exprs: &[WithSpan<'a, Expr<'a>>],
    ) -> Result<DisplayWrap, CompileError> {
        buf.write('(');
        for (index, expr) in exprs.iter().enumerate() {
            if index > 0 {
                buf.write(' ');
            }
            self.visit_expr(ctx, buf, expr)?;
            buf.write(',');
        }
        buf.write(')');
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_named_argument(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'a, Expr<'a>>,
    ) -> Result<DisplayWrap, CompileError> {
        self.visit_expr(ctx, buf, expr)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_array(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        elements: &[WithSpan<'a, Expr<'a>>],
    ) -> Result<DisplayWrap, CompileError> {
        buf.write('[');
        for (i, el) in elements.iter().enumerate() {
            if i > 0 {
                buf.write(',');
            }
            self.visit_expr(ctx, buf, el)?;
        }
        buf.write(']');
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_path(&mut self, buf: &mut Buffer, path: &[&str]) -> DisplayWrap {
        for (i, part) in path.iter().copied().enumerate() {
            if i > 0 {
                buf.write("::");
            } else if let Some(enum_ast) = self.input.enum_ast {
                if part == "Self" {
                    let this = &enum_ast.ident;
                    let (_, generics, _) = enum_ast.generics.split_for_impl();
                    let generics = generics.as_turbofish();
                    buf.write(quote!(#this #generics));
                    continue;
                }
            }
            buf.write(part);
        }
        DisplayWrap::Unwrapped
    }

    fn visit_var(&mut self, buf: &mut Buffer, s: &str) -> DisplayWrap {
        if s == "self" {
            buf.write(s);
            return DisplayWrap::Unwrapped;
        }

        buf.write(normalize_identifier(&self.locals.resolve_or_self(s)));
        DisplayWrap::Unwrapped
    }

    fn visit_filter_source(&mut self, buf: &mut Buffer) -> DisplayWrap {
        // We can assume that the body of the `{% filter %}` was already escaped.
        // And if it's not, then this was done intentionally.
        buf.write(format_args!("askama::filters::Safe(&{FILTER_SOURCE})"));
        DisplayWrap::Wrapped
    }

    fn visit_bool_lit(&mut self, buf: &mut Buffer, s: bool) -> DisplayWrap {
        if s {
            buf.write("true");
        } else {
            buf.write("false");
        }
        DisplayWrap::Unwrapped
    }

    pub(super) fn visit_str_lit(&mut self, buf: &mut Buffer, s: &StrLit<'_>) -> DisplayWrap {
        if let Some(prefix) = s.prefix {
            buf.write(prefix.to_char());
        }
        buf.write(format_args!("\"{}\"", s.content));
        DisplayWrap::Unwrapped
    }

    fn visit_char_lit(&mut self, buf: &mut Buffer, c: &CharLit<'_>) -> DisplayWrap {
        if c.prefix == Some(CharPrefix::Binary) {
            buf.write('b');
        }
        buf.write(format_args!("'{}'", c.content));
        DisplayWrap::Unwrapped
    }

    fn visit_num_lit(&mut self, buf: &mut Buffer, s: &str) -> DisplayWrap {
        buf.write(s);
        DisplayWrap::Unwrapped
    }

    pub(super) fn visit_target(
        &mut self,
        buf: &mut Buffer,
        initialized: bool,
        first_level: bool,
        target: &Target<'a>,
    ) {
        match target {
            Target::Placeholder(_) => buf.write('_'),
            Target::Rest(s) => {
                if let Some(var_name) = &**s {
                    self.locals
                        .insert(Cow::Borrowed(var_name), LocalMeta::initialized());
                    buf.write(var_name);
                    buf.write(" @ ");
                }
                buf.write("..");
            }
            Target::Name(name) => {
                let name = normalize_identifier(name);
                match initialized {
                    true => self
                        .locals
                        .insert(Cow::Borrowed(name), LocalMeta::initialized()),
                    false => self.locals.insert_with_default(Cow::Borrowed(name)),
                }
                buf.write(name);
            }
            Target::OrChain(targets) => match targets.first() {
                None => buf.write('_'),
                Some(first_target) => {
                    self.visit_target(buf, initialized, first_level, first_target);
                    for target in &targets[1..] {
                        buf.write('|');
                        self.visit_target(buf, initialized, first_level, target);
                    }
                }
            },
            Target::Tuple(path, targets) => {
                buf.write_separated_path(path);
                buf.write('(');
                for target in targets {
                    self.visit_target(buf, initialized, false, target);
                    buf.write(',');
                }
                buf.write(')');
            }
            Target::Array(path, targets) => {
                buf.write_separated_path(path);
                buf.write('[');
                for target in targets {
                    self.visit_target(buf, initialized, false, target);
                    buf.write(',');
                }
                buf.write(']');
            }
            Target::Struct(path, targets) => {
                buf.write_separated_path(path);
                buf.write('{');
                for (name, target) in targets {
                    if let Target::Rest(_) = target {
                        buf.write("..");
                        continue;
                    }

                    buf.write(normalize_identifier(name));
                    buf.write(": ");
                    self.visit_target(buf, initialized, false, target);
                    buf.write(',');
                }
                buf.write('}');
            }
            Target::Path(path) => {
                self.visit_path(buf, path);
                buf.write("{}");
            }
            Target::StrLit(s) => {
                if first_level {
                    buf.write('&');
                }
                self.visit_str_lit(buf, s);
            }
            Target::NumLit(s, _) => {
                if first_level {
                    buf.write('&');
                }
                self.visit_num_lit(buf, s);
            }
            Target::CharLit(s) => {
                if first_level {
                    buf.write('&');
                }
                self.visit_char_lit(buf, s);
            }
            Target::BoolLit(s) => {
                if first_level {
                    buf.write('&');
                }
                buf.write(s);
            }
        }
    }
}
