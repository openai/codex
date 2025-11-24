use std::borrow::Cow;
use std::collections::hash_map::{Entry, HashMap};
use std::fmt::{self, Write};
use std::mem;

use parser::node::{
    Call, Comment, Cond, CondTest, FilterBlock, If, Include, Let, Lit, Loop, Macro, Match,
    Whitespace, Ws,
};
use parser::{Expr, Filter, Node, Span, Target, WithSpan};
use rustc_hash::FxBuildHasher;

use super::{
    DisplayWrap, FILTER_SOURCE, Generator, LocalMeta, MapChain, compile_time_escape, is_copyable,
    normalize_identifier,
};
use crate::generator::Writable;
use crate::heritage::{Context, Heritage};
use crate::integration::Buffer;
use crate::{CompileError, FileInfo, fmt_left, fmt_right};

impl<'a> Generator<'a, '_> {
    pub(super) fn impl_template_inner(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
    ) -> Result<usize, CompileError> {
        buf.set_discard(self.buf_writable.discard);
        let size_hint = if let Some(heritage) = self.heritage {
            self.handle(heritage.root, heritage.root.nodes, buf, AstLevel::Top)
        } else {
            self.handle(ctx, ctx.nodes, buf, AstLevel::Top)
        }?;
        self.flush_ws(Ws(None, None));
        buf.set_discard(false);
        Ok(size_hint)
    }

    fn push_locals<T, F>(&mut self, callback: F) -> Result<T, CompileError>
    where
        F: FnOnce(&mut Self) -> Result<T, CompileError>,
    {
        self.locals.scopes.push(HashMap::default());
        let res = callback(self);
        self.locals.scopes.pop().unwrap();
        res
    }

    fn with_child<'b, T, F>(
        &mut self,
        heritage: Option<&'b Heritage<'a, 'b>>,
        callback: F,
    ) -> Result<T, CompileError>
    where
        F: FnOnce(&mut Generator<'a, 'b>) -> Result<T, CompileError>,
    {
        self.locals.scopes.push(HashMap::default());

        let buf_writable = mem::take(&mut self.buf_writable);
        let locals = mem::replace(&mut self.locals, MapChain::new_empty());

        let mut child = Generator::new(
            self.input,
            self.contexts,
            heritage,
            locals,
            self.buf_writable.discard,
            self.is_in_filter_block,
        );
        child.buf_writable = buf_writable;
        let res = callback(&mut child);
        Generator {
            locals: self.locals,
            buf_writable: self.buf_writable,
            ..
        } = child;

        self.locals.scopes.pop().unwrap();
        res
    }

    fn handle(
        &mut self,
        ctx: &Context<'a>,
        nodes: &'a [Node<'_>],
        buf: &mut Buffer,
        level: AstLevel,
    ) -> Result<usize, CompileError> {
        let mut size_hint = 0;
        for n in nodes {
            match *n {
                Node::Lit(ref lit) => {
                    self.write_lit(lit);
                }
                Node::Comment(ref comment) => {
                    self.write_comment(comment);
                }
                Node::Expr(ws, ref val) => {
                    self.write_expr(ws, val);
                }
                Node::Let(ref l) => {
                    self.write_let(ctx, buf, l)?;
                }
                Node::If(ref i) => {
                    size_hint += self.write_if(ctx, buf, i)?;
                }
                Node::Match(ref m) => {
                    size_hint += self.write_match(ctx, buf, m)?;
                }
                Node::Loop(ref loop_block) => {
                    size_hint += self.write_loop(ctx, buf, loop_block)?;
                }
                Node::BlockDef(ref b) => {
                    size_hint +=
                        self.write_block(ctx, buf, Some(b.name), Ws(b.ws1.0, b.ws2.1), b.span())?;
                }
                Node::Include(ref i) => {
                    size_hint += self.handle_include(ctx, buf, i)?;
                }
                Node::Call(ref call) => {
                    size_hint += self.write_call(ctx, buf, call)?;
                }
                Node::FilterBlock(ref filter) => {
                    size_hint += self.write_filter_block(ctx, buf, filter)?;
                }
                Node::Macro(ref m) => {
                    if level != AstLevel::Top {
                        return Err(ctx.generate_error(
                            "macro blocks only allowed at the top level",
                            m.span(),
                        ));
                    }
                    self.flush_ws(m.ws1);
                    self.prepare_ws(m.ws2);
                }
                Node::Raw(ref raw) => {
                    self.handle_ws(raw.ws1);
                    self.write_lit(&raw.lit);
                    self.handle_ws(raw.ws2);
                }
                Node::Import(ref i) => {
                    if level != AstLevel::Top {
                        return Err(ctx.generate_error(
                            "import blocks only allowed at the top level",
                            i.span(),
                        ));
                    }
                    self.handle_ws(i.ws);
                }
                Node::Extends(ref e) => {
                    if level != AstLevel::Top {
                        return Err(ctx.generate_error(
                            "extend blocks only allowed at the top level",
                            e.span(),
                        ));
                    }
                    // No whitespace handling: child template top-level is not used,
                    // except for the blocks defined in it.
                }
                Node::Break(ref ws) => {
                    self.handle_ws(**ws);
                    self.write_buf_writable(ctx, buf)?;
                    buf.write("break;");
                }
                Node::Continue(ref ws) => {
                    self.handle_ws(**ws);
                    self.write_buf_writable(ctx, buf)?;
                    buf.write("continue;");
                }
            }
        }

        if AstLevel::Top == level {
            // Handle any pending whitespace.
            if self.next_ws.is_some() {
                self.flush_ws(Ws(Some(self.skip_ws), None));
            }

            size_hint += self.write_buf_writable(ctx, buf)?;
        }
        Ok(size_hint)
    }

    fn evaluate_condition(
        &self,
        expr: WithSpan<'a, Expr<'a>>,
        only_contains_is_defined: &mut bool,
    ) -> (EvaluatedResult, WithSpan<'a, Expr<'a>>) {
        let (expr, span) = expr.deconstruct();

        match expr {
            Expr::NumLit(_, _)
            | Expr::StrLit(_)
            | Expr::CharLit(_)
            | Expr::Var(_)
            | Expr::Path(_)
            | Expr::Array(_)
            | Expr::Attr(_, _)
            | Expr::Index(_, _)
            | Expr::Filter(_)
            | Expr::Range(_, _, _)
            | Expr::Call { .. }
            | Expr::RustMacro(_, _)
            | Expr::Try(_)
            | Expr::Tuple(_)
            | Expr::NamedArgument(_, _)
            | Expr::FilterSource
            | Expr::As(_, _)
            | Expr::Concat(_)
            | Expr::LetCond(_)
            | Expr::ArgumentPlaceholder => {
                *only_contains_is_defined = false;
                (EvaluatedResult::Unknown, WithSpan::new(expr, span))
            }
            Expr::BoolLit(true) => (EvaluatedResult::AlwaysTrue, WithSpan::new(expr, span)),
            Expr::BoolLit(false) => (EvaluatedResult::AlwaysFalse, WithSpan::new(expr, span)),
            Expr::Unary("!", inner) => {
                let (result, expr) = self.evaluate_condition(*inner, only_contains_is_defined);
                match result {
                    EvaluatedResult::AlwaysTrue => (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(Expr::BoolLit(false), ""),
                    ),
                    EvaluatedResult::AlwaysFalse => (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(Expr::BoolLit(true), ""),
                    ),
                    EvaluatedResult::Unknown => (
                        EvaluatedResult::Unknown,
                        WithSpan::new(Expr::Unary("!", Box::new(expr)), span),
                    ),
                }
            }
            Expr::Unary(_, _) => (EvaluatedResult::Unknown, WithSpan::new(expr, span)),
            Expr::BinOp("&&", left, right) => {
                let (result_left, expr_left) =
                    self.evaluate_condition(*left, only_contains_is_defined);
                if result_left == EvaluatedResult::AlwaysFalse {
                    // The right side of the `&&` won't be evaluated, no need to go any further.
                    return (result_left, WithSpan::new(Expr::BoolLit(false), ""));
                }
                let (result_right, expr_right) =
                    self.evaluate_condition(*right, only_contains_is_defined);
                match (result_left, result_right) {
                    (EvaluatedResult::AlwaysTrue, EvaluatedResult::AlwaysTrue) => (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(Expr::BoolLit(true), ""),
                    ),
                    (_, EvaluatedResult::AlwaysFalse) => (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(
                            Expr::BinOp("&&", Box::new(expr_left), Box::new(expr_right)),
                            span,
                        ),
                    ),
                    (EvaluatedResult::AlwaysTrue, _) => (result_right, expr_right),
                    (_, EvaluatedResult::AlwaysTrue) => (result_left, expr_left),
                    _ => (
                        EvaluatedResult::Unknown,
                        WithSpan::new(
                            Expr::BinOp("&&", Box::new(expr_left), Box::new(expr_right)),
                            span,
                        ),
                    ),
                }
            }
            Expr::BinOp("||", left, right) => {
                let (result_left, expr_left) =
                    self.evaluate_condition(*left, only_contains_is_defined);
                if result_left == EvaluatedResult::AlwaysTrue {
                    // The right side of the `||` won't be evaluated, no need to go any further.
                    return (result_left, WithSpan::new(Expr::BoolLit(true), ""));
                }
                let (result_right, expr_right) =
                    self.evaluate_condition(*right, only_contains_is_defined);
                match (result_left, result_right) {
                    (EvaluatedResult::AlwaysFalse, EvaluatedResult::AlwaysFalse) => (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(Expr::BoolLit(false), ""),
                    ),
                    (_, EvaluatedResult::AlwaysTrue) => (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(
                            Expr::BinOp("||", Box::new(expr_left), Box::new(expr_right)),
                            span,
                        ),
                    ),
                    (EvaluatedResult::AlwaysFalse, _) => (result_right, expr_right),
                    (_, EvaluatedResult::AlwaysFalse) => (result_left, expr_left),
                    _ => (
                        EvaluatedResult::Unknown,
                        WithSpan::new(
                            Expr::BinOp("||", Box::new(expr_left), Box::new(expr_right)),
                            span,
                        ),
                    ),
                }
            }
            Expr::BinOp(_, _, _) => {
                *only_contains_is_defined = false;
                (EvaluatedResult::Unknown, WithSpan::new(expr, span))
            }
            Expr::Group(inner) => {
                let (result, expr) = self.evaluate_condition(*inner, only_contains_is_defined);
                (result, WithSpan::new(Expr::Group(Box::new(expr)), span))
            }
            Expr::IsDefined(left) => {
                // Variable is defined so we want to keep the condition.
                if self.is_var_defined(left) {
                    (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(Expr::BoolLit(true), ""),
                    )
                } else {
                    (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(Expr::BoolLit(false), ""),
                    )
                }
            }
            Expr::IsNotDefined(left) => {
                // Variable is defined so we don't want to keep the condition.
                if self.is_var_defined(left) {
                    (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(Expr::BoolLit(false), ""),
                    )
                } else {
                    (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(Expr::BoolLit(true), ""),
                    )
                }
            }
        }
    }

    fn write_if(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        if_: &'a If<'_>,
    ) -> Result<usize, CompileError> {
        let mut flushed = 0;
        let mut arm_sizes = Vec::new();
        let mut has_else = false;

        let conds = Conds::compute_branches(self, if_);

        if let Some(ws_before) = conds.ws_before {
            self.handle_ws(ws_before);
        }

        let mut iter = conds.conds.iter().enumerate().peekable();
        while let Some((pos, cond_info)) = iter.next() {
            let cond = cond_info.cond;

            if pos == 0 {
                self.handle_ws(cond.ws);
                flushed += self.write_buf_writable(ctx, buf)?;
            }

            self.push_locals(|this| {
                let mut arm_size = 0;

                if let Some(CondTest { target, expr, .. }) = &cond.cond {
                    let expr = cond_info.cond_expr.as_ref().unwrap_or(expr);

                    if pos == 0 {
                        if cond_info.generate_condition {
                            buf.write("if ");
                        }
                        // Otherwise it means it will be the only condition generated,
                        // so nothing to be added here.
                    } else if cond_info.generate_condition {
                        buf.write("} else if ");
                    } else {
                        buf.write("} else {");
                        has_else = true;
                    }

                    if let Some(target) = target {
                        let mut expr_buf = Buffer::new();
                        buf.write("let ");
                        // If this is a chain condition, then we need to declare the variable after the
                        // left expression has been handled but before the right expression is handled
                        // but this one should have access to the let-bound variable.
                        match &**expr {
                            Expr::BinOp(op @ ("||" | "&&"), ref left, ref right) => {
                                let display_wrap =
                                    this.visit_expr_first(ctx, &mut expr_buf, left)?;
                                this.visit_target(buf, true, true, target);
                                this.visit_expr_not_first(ctx, &mut expr_buf, left, display_wrap)?;
                                buf.write(format_args!("= &{expr_buf}"));
                                buf.write(format_args!(" {op} "));
                                this.visit_condition(ctx, buf, right)?;
                            }
                            _ => {
                                let display_wrap =
                                    this.visit_expr_first(ctx, &mut expr_buf, expr)?;
                                this.visit_target(buf, true, true, target);
                                this.visit_expr_not_first(ctx, &mut expr_buf, expr, display_wrap)?;
                                buf.write(format_args!("= &{expr_buf}"));
                            }
                        }
                        buf.write("{");
                    } else if cond_info.generate_condition {
                        this.visit_condition(ctx, buf, expr)?;
                        buf.write('{');
                    }
                } else if pos != 0 {
                    buf.write("} else {");
                    has_else = true;
                }

                if cond_info.generate_content {
                    arm_size += this.handle(ctx, &cond.nodes, buf, AstLevel::Nested)?;
                }
                arm_sizes.push(arm_size);

                if let Some((_, cond_info)) = iter.peek() {
                    let cond = cond_info.cond;

                    this.handle_ws(cond.ws);
                    flushed += this.write_buf_writable(ctx, buf)?;
                } else {
                    if let Some(ws_after) = conds.ws_after {
                        this.handle_ws(ws_after);
                    }
                    this.handle_ws(if_.ws);
                    flushed += this.write_buf_writable(ctx, buf)?;
                }
                Ok(0)
            })?;
        }

        if conds.nb_conds > 0 {
            buf.write('}');
        }

        if !has_else && !conds.conds.is_empty() {
            arm_sizes.push(0);
        }
        Ok(flushed + median(&mut arm_sizes))
    }

    #[allow(clippy::too_many_arguments)]
    fn write_match(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        m: &'a Match<'a>,
    ) -> Result<usize, CompileError> {
        let Match {
            ws1,
            ref expr,
            ref arms,
            ws2,
        } = *m;

        self.flush_ws(ws1);
        let flushed = self.write_buf_writable(ctx, buf)?;
        let mut arm_sizes = Vec::new();

        let expr_code = self.visit_expr_root(ctx, expr)?;
        buf.write(format_args!("match &{expr_code} {{"));

        let mut arm_size = 0;
        let mut iter = arms.iter().enumerate().peekable();
        while let Some((i, arm)) = iter.next() {
            if i == 0 {
                self.handle_ws(arm.ws);
            }

            self.push_locals(|this| {
                for (index, target) in arm.target.iter().enumerate() {
                    if index != 0 {
                        buf.write('|');
                    }
                    this.visit_target(buf, true, true, target);
                }
                buf.write(" => {");

                arm_size = this.handle(ctx, &arm.nodes, buf, AstLevel::Nested)?;

                if let Some((_, arm)) = iter.peek() {
                    this.handle_ws(arm.ws);
                    arm_sizes.push(arm_size + this.write_buf_writable(ctx, buf)?);

                    buf.write('}');
                } else {
                    this.handle_ws(ws2);
                    arm_sizes.push(arm_size + this.write_buf_writable(ctx, buf)?);
                    buf.write('}');
                }
                Ok(0)
            })?;
        }

        buf.write('}');

        Ok(flushed + median(&mut arm_sizes))
    }

    #[allow(clippy::too_many_arguments)]
    fn write_loop(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        loop_block: &'a WithSpan<'a, Loop<'_>>,
    ) -> Result<usize, CompileError> {
        self.handle_ws(loop_block.ws1);
        self.push_locals(|this| {
            let expr_code = this.visit_expr_root(ctx, &loop_block.iter)?;

            let has_else_nodes = !loop_block.else_nodes.is_empty();

            let flushed = this.write_buf_writable(ctx, buf)?;
            buf.write('{');
            if has_else_nodes {
                buf.write("let mut _did_loop = false;");
            }
            match &*loop_block.iter {
                Expr::Range(_, _, _) => buf.write(format_args!("let _iter = {expr_code};")),
                Expr::Array(..) => buf.write(format_args!("let _iter = {expr_code}.iter();")),
                // If `iter` is a call then we assume it's something that returns
                // an iterator. If not then the user can explicitly add the needed
                // call without issues.
                Expr::Call { .. } | Expr::Index(..) => {
                    buf.write(format_args!("let _iter = ({expr_code}).into_iter();"));
                }
                // If accessing `self` then it most likely needs to be
                // borrowed, to prevent an attempt of moving.
                _ if expr_code.starts_with("self.") => {
                    buf.write(format_args!("let _iter = (&{expr_code}).into_iter();"));
                }
                // If accessing a field then it most likely needs to be
                // borrowed, to prevent an attempt of moving.
                Expr::Attr(..) => {
                    buf.write(format_args!("let _iter = (&{expr_code}).into_iter();"));
                }
                // Otherwise, we borrow `iter` assuming that it implements `IntoIterator`.
                _ => buf.write(format_args!("let _iter = ({expr_code}).into_iter();")),
            }
            if let Some(cond) = &loop_block.cond {
                this.push_locals(|this| {
                    buf.write("let _iter = _iter.filter(|");
                    this.visit_target(buf, true, true, &loop_block.var);
                    buf.write("| -> bool {");
                    this.visit_expr(ctx, buf, cond)?;
                    buf.write("});");
                    Ok(0)
                })?;
            }

            let size_hint1 = this.push_locals(|this| {
                buf.write("for (");
                this.visit_target(buf, true, true, &loop_block.var);
                buf.write(", _loop_item) in askama::helpers::TemplateLoop::new(_iter) {");

                if has_else_nodes {
                    buf.write("_did_loop = true;");
                }
                let mut size_hint1 = this.handle(ctx, &loop_block.body, buf, AstLevel::Nested)?;
                this.handle_ws(loop_block.ws2);
                size_hint1 += this.write_buf_writable(ctx, buf)?;
                Ok(size_hint1)
            })?;
            buf.write('}');

            let size_hint2;
            if has_else_nodes {
                buf.write("if !_did_loop {");
                size_hint2 = this.push_locals(|this| {
                    let mut size_hint =
                        this.handle(ctx, &loop_block.else_nodes, buf, AstLevel::Nested)?;
                    this.handle_ws(loop_block.ws3);
                    size_hint += this.write_buf_writable(ctx, buf)?;
                    Ok(size_hint)
                })?;
                buf.write('}');
            } else {
                this.handle_ws(loop_block.ws3);
                size_hint2 = this.write_buf_writable(ctx, buf)?;
            }

            buf.write('}');
            Ok(flushed + ((size_hint1 * 3) + size_hint2) / 2)
        })
    }

    fn write_call(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        call: &'a WithSpan<'a, Call<'_>>,
    ) -> Result<usize, CompileError> {
        let Call {
            ws,
            scope,
            name,
            ref args,
        } = **call;
        if name == "super" {
            return self.write_block(ctx, buf, None, ws, call.span());
        }

        let (def, own_ctx) = if let Some(s) = scope {
            let path = ctx.imports.get(s).ok_or_else(|| {
                ctx.generate_error(format_args!("no import found for scope {s:?}"), call.span())
            })?;
            let mctx = self.contexts.get(path).ok_or_else(|| {
                ctx.generate_error(format_args!("context for {path:?} not found"), call.span())
            })?;
            let def = mctx.macros.get(name).ok_or_else(|| {
                ctx.generate_error(
                    format_args!("macro {name:?} not found in scope {s:?}"),
                    call.span(),
                )
            })?;
            (*def, mctx)
        } else {
            let def = ctx.macros.get(name).ok_or_else(|| {
                ctx.generate_error(format_args!("macro {name:?} not found"), call.span())
            })?;
            (*def, ctx)
        };

        if self.seen_macros.iter().any(|(s, _)| std::ptr::eq(*s, def)) {
            let mut message = "Found recursion in macro calls:".to_owned();
            for (m, f) in &self.seen_macros {
                if let Some(f) = f {
                    write!(message, "{f}").unwrap();
                } else {
                    write!(message, "\n`{}`", m.name.escape_debug()).unwrap();
                }
            }
            return Err(ctx.generate_error(message, call.span()));
        } else {
            self.seen_macros.push((def, ctx.file_info_of(call.span())));
        }

        self.flush_ws(ws); // Cannot handle_ws() here: whitespace from macro definition comes first
        let size_hint = self.push_locals(|this| {
            macro_call_ensure_arg_count(call, def, ctx)?;

            this.write_buf_writable(ctx, buf)?;
            buf.write('{');
            this.prepare_ws(def.ws1);

            let mut named_arguments: HashMap<&str, _, FxBuildHasher> = HashMap::default();
            // Since named arguments can only be passed last, we only need to check if the last argument
            // is a named one.
            if let Some(Expr::NamedArgument(_, _)) = args.last().map(|expr| &**expr) {
                // First we check that all named arguments actually exist in the called item.
                for (index, arg) in args.iter().enumerate().rev() {
                    let Expr::NamedArgument(arg_name, _) = &**arg else {
                        break;
                    };
                    if !def.args.iter().any(|(arg, _)| arg == arg_name) {
                        return Err(ctx.generate_error(
                            format_args!("no argument named `{arg_name}` in macro {name:?}"),
                            call.span(),
                        ));
                    }
                    named_arguments.insert(arg_name, (index, arg));
                }
            }

            let mut value = Buffer::new();

            // Handling both named and unnamed arguments requires to be careful of the named arguments
            // order. To do so, we iterate through the macro defined arguments and then check if we have
            // a named argument with this name:
            //
            // * If there is one, we add it and move to the next argument.
            // * If there isn't one, then we pick the next argument (we can do it without checking
            //   anything since named arguments are always last).
            let mut allow_positional = true;
            let mut used_named_args = vec![false; args.len()];
            for (index, (arg, default_value)) in def.args.iter().enumerate() {
                let expr = if let Some((index, expr)) = named_arguments.get(arg) {
                    used_named_args[*index] = true;
                    allow_positional = false;
                    expr
                } else {
                    match args.get(index) {
                        Some(arg_expr) if !matches!(**arg_expr, Expr::NamedArgument(_, _)) => {
                            // If there is already at least one named argument, then it's not allowed
                            // to use unnamed ones at this point anymore.
                            if !allow_positional {
                                return Err(ctx.generate_error(
                                    format_args!(
                                        "cannot have unnamed argument (`{arg}`) after named argument \
                                         in call to macro {name:?}"
                                    ),
                                    call.span(),
                                ));
                            }
                            arg_expr
                        }
                        Some(arg_expr) if used_named_args[index] => {
                            let Expr::NamedArgument(name, _) = **arg_expr else { unreachable!() };
                            return Err(ctx.generate_error(
                                format_args!("`{name}` is passed more than once"),
                                call.span(),
                            ));
                        }
                        _ => {
                            if let Some(default_value) = default_value {
                                default_value
                            } else {
                                return Err(ctx.generate_error(format_args!("missing `{arg}` argument"), call.span()));
                            }
                        }
                    }
                };
                match &**expr {
                    // If `expr` is already a form of variable then
                    // don't reintroduce a new variable. This is
                    // to avoid moving non-copyable values.
                    Expr::Var(name) if *name != "self" => {
                        let var = this.locals.resolve_or_self(name);
                        this.locals
                            .insert(Cow::Borrowed(arg), LocalMeta::with_ref(var));
                    }
                    Expr::Attr(obj, attr) => {
                        let mut attr_buf = Buffer::new();
                        this.visit_attr(ctx, &mut attr_buf, obj, attr)?;

                        let attr = attr_buf.into_string();
                        let var = this.locals.resolve(&attr).unwrap_or(attr);
                        this.locals
                            .insert(Cow::Borrowed(arg), LocalMeta::with_ref(var));
                    }
                    // Everything else still needs to become variables,
                    // to avoid having the same logic be executed
                    // multiple times, e.g. in the case of macro
                    // parameters being used multiple times.
                    _ => {
                        value.clear();
                        let (before, after) = if !is_copyable(expr) {
                            ("&(", ")")
                        } else {
                            ("", "")
                        };
                        value.write(this.visit_expr_root(ctx, expr)?);
                        // We need to normalize the arg to write it, thus we need to add it to
                        // locals in the normalized manner
                        let normalized_arg = normalize_identifier(arg);
                        buf.write(format_args!("let {} = {before}{value}{after};", normalized_arg));
                        this.locals.insert_with_default(Cow::Borrowed(normalized_arg));
                    }
                }
            }

            let mut size_hint = this.handle(own_ctx, &def.nodes, buf, AstLevel::Nested)?;

            this.flush_ws(def.ws2);
            size_hint += this.write_buf_writable(ctx, buf)?;
            buf.write('}');
            Ok(size_hint)
        })?;
        self.prepare_ws(ws);
        self.seen_macros.pop();
        Ok(size_hint)
    }

    fn write_filter_block(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        filter: &'a WithSpan<'a, FilterBlock<'_>>,
    ) -> Result<usize, CompileError> {
        self.write_buf_writable(ctx, buf)?;
        self.flush_ws(filter.ws1);
        self.is_in_filter_block += 1;
        self.write_buf_writable(ctx, buf)?;
        buf.write('{');

        // build `FmtCell` that contains the inner block
        buf.write(format_args!(
            "let {FILTER_SOURCE} = askama::helpers::FmtCell::new(\
                |__askama_writer: &mut askama::helpers::core::fmt::Formatter<'_>| -> askama::Result<()> {{"
        ));
        let size_hint = self.push_locals(|this| {
            this.prepare_ws(filter.ws1);
            let size_hint = this.handle(ctx, &filter.nodes, buf, AstLevel::Nested)?;
            this.flush_ws(filter.ws2);
            this.write_buf_writable(ctx, buf)?;
            Ok(size_hint)
        })?;
        buf.write(
            "\
                askama::Result::Ok(())\
            });",
        );

        // display the `FmtCell`
        let mut filter_buf = Buffer::new();
        let display_wrap = self.visit_filter(
            ctx,
            &mut filter_buf,
            filter.filters.name,
            &filter.filters.arguments,
            &filter.filters.generics,
            filter.span(),
        )?;
        let filter_buf = match display_wrap {
            DisplayWrap::Wrapped => fmt_left!("{filter_buf}"),
            DisplayWrap::Unwrapped => fmt_right!(
                "(&&askama::filters::AutoEscaper::new(&({filter_buf}), {})).askama_auto_escape()?",
                self.input.escaper,
            ),
        };
        buf.write(format_args!(
            "if askama::helpers::core::write!(__askama_writer, \"{{}}\", {filter_buf}).is_err() {{\
                return {FILTER_SOURCE}.take_err();\
            }}"
        ));

        buf.write('}');
        self.is_in_filter_block -= 1;
        self.prepare_ws(filter.ws2);
        Ok(size_hint)
    }

    fn handle_include(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        i: &'a WithSpan<'a, Include<'_>>,
    ) -> Result<usize, CompileError> {
        self.flush_ws(i.ws);
        self.write_buf_writable(ctx, buf)?;
        let file_info = ctx
            .path
            .map(|path| FileInfo::of(i.span(), path, ctx.parsed));
        let path = self
            .input
            .config
            .find_template(i.path, Some(&self.input.path), file_info)?;

        // We clone the context of the child in order to preserve their macros and imports.
        // But also add all the imports and macros from this template that don't override the
        // child's ones to preserve this template's context.
        let child_ctx = &mut self.contexts[&path].clone();
        for (name, mac) in &ctx.macros {
            child_ctx.macros.entry(name).or_insert(mac);
        }
        for (name, import) in &ctx.imports {
            child_ctx
                .imports
                .entry(name)
                .or_insert_with(|| import.clone());
        }

        // Create a new generator for the child, and call it like in `impl_template` as if it were
        // a full template, while preserving the context.
        let heritage = if !child_ctx.blocks.is_empty() || child_ctx.extends.is_some() {
            Some(Heritage::new(child_ctx, self.contexts))
        } else {
            None
        };

        let handle_ctx = match &heritage {
            Some(heritage) => heritage.root,
            None => child_ctx,
        };

        let size_hint = self.with_child(heritage.as_ref(), |child| {
            let mut size_hint = 0;
            size_hint += child.handle(handle_ctx, handle_ctx.nodes, buf, AstLevel::Top)?;
            size_hint += child.write_buf_writable(handle_ctx, buf)?;
            Ok(size_hint)
        })?;

        self.prepare_ws(i.ws);

        Ok(size_hint)
    }

    fn is_shadowing_variable(
        &self,
        ctx: &Context<'_>,
        var: &Target<'a>,
        l: Span<'_>,
    ) -> Result<bool, CompileError> {
        match var {
            Target::Name(name) => {
                let name = normalize_identifier(name);
                match self.locals.get(name) {
                    // declares a new variable
                    None => Ok(false),
                    // an initialized variable gets shadowed
                    Some(meta) if meta.initialized => Ok(true),
                    // initializes a variable that was introduced in a LetDecl before
                    _ => Ok(false),
                }
            }
            Target::Placeholder(_) => Ok(false),
            Target::Rest(var_name) => {
                if let Some(var_name) = **var_name {
                    match self.is_shadowing_variable(ctx, &Target::Name(var_name), l) {
                        Ok(false) => {}
                        outcome => return outcome,
                    }
                }
                Ok(false)
            }
            Target::Tuple(_, targets) => {
                for target in targets {
                    match self.is_shadowing_variable(ctx, target, l) {
                        Ok(false) => continue,
                        outcome => return outcome,
                    }
                }
                Ok(false)
            }
            Target::Struct(_, named_targets) => {
                for (_, target) in named_targets {
                    match self.is_shadowing_variable(ctx, target, l) {
                        Ok(false) => continue,
                        outcome => return outcome,
                    }
                }
                Ok(false)
            }
            _ => Err(ctx.generate_error(
                "literals are not allowed on the left-hand side of an assignment",
                l,
            )),
        }
    }

    fn write_let(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        l: &'a WithSpan<'a, Let<'_>>,
    ) -> Result<(), CompileError> {
        self.handle_ws(l.ws);

        let Some(val) = &l.val else {
            self.write_buf_writable(ctx, buf)?;
            buf.write("let ");
            self.visit_target(buf, false, true, &l.var);
            buf.write(';');
            return Ok(());
        };

        let mut expr_buf = Buffer::new();
        self.visit_expr(ctx, &mut expr_buf, val)?;

        let shadowed = self.is_shadowing_variable(ctx, &l.var, l.span())?;
        if shadowed {
            // Need to flush the buffer if the variable is being shadowed,
            // to ensure the old variable is used.
            self.write_buf_writable(ctx, buf)?;
        }
        if shadowed
            || !matches!(l.var, Target::Name(_))
            || matches!(&l.var, Target::Name(name) if self.locals.get(name).is_none())
        {
            buf.write("let ");
        }

        self.visit_target(buf, true, true, &l.var);
        // If it's not taking the ownership of a local variable or copyable, then we need to add
        // a reference.
        let (before, after) = if !matches!(**val, Expr::Try(..))
            && !matches!(**val, Expr::Var(name) if self.locals.get(name).is_some())
            && !is_copyable(val)
        {
            ("&(", ")")
        } else {
            ("", "")
        };
        buf.write(format_args!(" = {before}{expr_buf}{after};"));
        Ok(())
    }

    // If `name` is `Some`, this is a call to a block definition, and we have to find
    // the first block for that name from the ancestry chain. If name is `None`, this
    // is from a `super()` call, and we can get the name from `self.super_block`.
    fn write_block(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        name: Option<&'a str>,
        outer: Ws,
        node: Span<'_>,
    ) -> Result<usize, CompileError> {
        if self.is_in_filter_block > 0 {
            return Err(ctx.generate_error("cannot have a block inside a filter block", node));
        }
        // Flush preceding whitespace according to the outer WS spec
        self.flush_ws(outer);

        let cur = match (name, self.super_block) {
            // The top-level context contains a block definition
            (Some(cur_name), None) => (cur_name, 0),
            // A block definition contains a block definition of the same name
            (Some(cur_name), Some((prev_name, _))) if cur_name == prev_name => {
                return Err(ctx.generate_error(
                    format_args!("cannot define recursive blocks ({cur_name})"),
                    node,
                ));
            }
            // A block definition contains a definition of another block
            (Some(cur_name), Some((_, _))) => (cur_name, 0),
            // `super()` was called inside a block
            (None, Some((prev_name, gen))) => (prev_name, gen + 1),
            // `super()` is called from outside a block
            (None, None) => {
                return Err(ctx.generate_error("cannot call 'super()' outside block", node));
            }
        };

        self.write_buf_writable(ctx, buf)?;

        let block_fragment_write =
            self.input.block.map(|(block, _)| block) == name && self.buf_writable.discard;
        // Allow writing to the buffer if we're in the block fragment
        if block_fragment_write {
            self.buf_writable.discard = false;
        }
        let prev_buf_discard = buf.is_discard();
        buf.set_discard(self.buf_writable.discard);

        // Get the block definition from the heritage chain
        let heritage = self
            .heritage
            .ok_or_else(|| ctx.generate_error("no block ancestors available", node))?;
        let (child_ctx, def) = *heritage.blocks[cur.0].get(cur.1).ok_or_else(|| {
            ctx.generate_error(
                match name {
                    None => fmt_left!("no super() block found for block '{}'", cur.0),
                    Some(name) => fmt_right!(move "no block found for name '{name}'"),
                },
                node,
            )
        })?;

        // We clone the context of the child in order to preserve their macros and imports.
        // But also add all the imports and macros from this template that don't override the
        // child's ones to preserve this template's context.
        let mut child_ctx = child_ctx.clone();
        for (name, mac) in &ctx.macros {
            child_ctx.macros.entry(name).or_insert(mac);
        }
        for (name, import) in &ctx.imports {
            child_ctx
                .imports
                .entry(name)
                .or_insert_with(|| import.clone());
        }

        let size_hint = self.with_child(Some(heritage), |child| {
            // Handle inner whitespace suppression spec and process block nodes
            child.prepare_ws(def.ws1);

            child.super_block = Some(cur);
            let size_hint = child.handle(&child_ctx, &def.nodes, buf, AstLevel::Block)?;

            if !child.locals.is_current_empty() {
                // Need to flush the buffer before popping the variable stack
                child.write_buf_writable(ctx, buf)?;
            }

            child.flush_ws(def.ws2);
            Ok(size_hint)
        })?;

        // Restore original block context and set whitespace suppression for
        // succeeding whitespace according to the outer WS spec
        self.prepare_ws(outer);

        // If we are rendering a specific block and the discard changed, it means that we're done
        // with the block we want to render and that from this point, everything will be discarded.
        //
        // To get this block content rendered as well, we need to write to the buffer before then.
        if buf.is_discard() != prev_buf_discard {
            self.write_buf_writable(ctx, buf)?;
        }
        // Restore the original buffer discarding state
        if block_fragment_write {
            self.buf_writable.discard = true;
        }
        buf.set_discard(prev_buf_discard);

        Ok(size_hint)
    }

    fn write_expr(&mut self, ws: Ws, s: &'a WithSpan<'a, Expr<'a>>) {
        self.handle_ws(ws);
        let items = if let Expr::Concat(exprs) = &**s {
            exprs
        } else {
            std::slice::from_ref(s)
        };
        for s in items {
            self.buf_writable
                .push(compile_time_escape(s, self.input.escaper).unwrap_or(Writable::Expr(s)));
        }
    }

    // Write expression buffer and empty
    fn write_buf_writable(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
    ) -> Result<usize, CompileError> {
        let mut size_hint = 0;
        let items = mem::take(&mut self.buf_writable.buf);
        let mut it = items.iter().enumerate().peekable();

        while let Some((_, Writable::Lit(s))) = it.peek() {
            size_hint += buf.write_writer(s);
            it.next();
        }
        if it.peek().is_none() {
            return Ok(size_hint);
        }

        let mut targets = Buffer::new();
        let mut lines = Buffer::new();
        let mut expr_cache = HashMap::with_capacity(self.buf_writable.len());
        // the `last_line` contains any sequence of trailing simple `writer.write_str()` calls
        let mut trailing_simple_lines = Vec::new();

        buf.write("match (");
        while let Some((idx, s)) = it.next() {
            match s {
                Writable::Lit(s) => {
                    let mut items = vec![s];
                    while let Some((_, Writable::Lit(s))) = it.peek() {
                        items.push(s);
                        it.next();
                    }
                    if it.peek().is_some() {
                        for s in items {
                            size_hint += lines.write_writer(s);
                        }
                    } else {
                        trailing_simple_lines = items;
                        break;
                    }
                }
                Writable::Expr(s) => {
                    size_hint += 3;

                    let mut expr_buf = Buffer::new();
                    let expr = match self.visit_expr(ctx, &mut expr_buf, s)? {
                        DisplayWrap::Wrapped => expr_buf.into_string(),
                        DisplayWrap::Unwrapped => format!(
                            "(&&askama::filters::AutoEscaper::new(&({expr_buf}), {})).\
                                askama_auto_escape()?",
                            self.input.escaper,
                        ),
                    };
                    let idx = if is_cacheable(s) {
                        match expr_cache.entry(expr) {
                            Entry::Occupied(e) => *e.get(),
                            Entry::Vacant(e) => {
                                buf.write(format_args!("&({}),", e.key()));
                                targets.write(format_args!("expr{idx},"));
                                e.insert(idx);
                                idx
                            }
                        }
                    } else {
                        buf.write(format_args!("&({expr}),"));
                        targets.write(format_args!("expr{idx}, "));
                        idx
                    };
                    lines.write(format_args!(
                        "(&&&askama::filters::Writable(expr{idx})).\
                             askama_write(__askama_writer, __askama_values)?;",
                    ));
                }
            }
        }
        buf.write(format_args!(
            ") {{\
                ({targets}) => {{\
                    {lines}\
                }}\
            }}"
        ));

        for s in trailing_simple_lines {
            size_hint += buf.write_writer(s);
        }

        Ok(size_hint)
    }

    fn write_comment(&mut self, comment: &'a WithSpan<'a, Comment<'_>>) {
        self.handle_ws(comment.ws);
    }

    fn write_lit(&mut self, lit: &'a Lit<'_>) {
        assert!(self.next_ws.is_none());
        let Lit { lws, val, rws } = *lit;
        if !lws.is_empty() {
            match self.skip_ws {
                Whitespace::Suppress => {}
                _ if val.is_empty() => {
                    assert!(rws.is_empty());
                    self.next_ws = Some(lws);
                }
                Whitespace::Preserve => {
                    self.buf_writable.push(Writable::Lit(Cow::Borrowed(lws)));
                }
                Whitespace::Minimize => {
                    self.buf_writable.push(Writable::Lit(Cow::Borrowed(
                        match lws.contains('\n') {
                            true => "\n",
                            false => " ",
                        },
                    )));
                }
            }
        }

        if !val.is_empty() {
            self.skip_ws = Whitespace::Preserve;
            self.buf_writable.push(Writable::Lit(Cow::Borrowed(val)));
        }

        if !rws.is_empty() {
            self.next_ws = Some(rws);
        }
    }

    // Helper methods for dealing with whitespace nodes

    // Combines `flush_ws()` and `prepare_ws()` to handle both trailing whitespace from the
    // preceding literal and leading whitespace from the succeeding literal.
    fn handle_ws(&mut self, ws: Ws) {
        self.flush_ws(ws);
        self.prepare_ws(ws);
    }

    fn should_trim_ws(&self, ws: Option<Whitespace>) -> Whitespace {
        ws.unwrap_or(self.input.config.whitespace)
    }

    // If the previous literal left some trailing whitespace in `next_ws` and the
    // prefix whitespace suppressor from the given argument, flush that whitespace.
    // In either case, `next_ws` is reset to `None` (no trailing whitespace).
    fn flush_ws(&mut self, ws: Ws) {
        if self.next_ws.is_none() {
            return;
        }

        // If `whitespace` is set to `suppress`, we keep the whitespace characters only if there is
        // a `+` character.
        match self.should_trim_ws(ws.0) {
            Whitespace::Preserve => {
                let val = self.next_ws.unwrap();
                if !val.is_empty() {
                    self.buf_writable.push(Writable::Lit(Cow::Borrowed(val)));
                }
            }
            Whitespace::Minimize => {
                let val = self.next_ws.unwrap();
                if !val.is_empty() {
                    self.buf_writable.push(Writable::Lit(Cow::Borrowed(
                        match val.contains('\n') {
                            true => "\n",
                            false => " ",
                        },
                    )));
                }
            }
            Whitespace::Suppress => {}
        }
        self.next_ws = None;
    }

    // Sets `skip_ws` to match the suffix whitespace suppressor from the given
    // argument, to determine whether to suppress leading whitespace from the
    // next literal.
    fn prepare_ws(&mut self, ws: Ws) {
        self.skip_ws = self.should_trim_ws(ws.1);
    }
}

struct CondInfo<'a> {
    cond: &'a WithSpan<'a, Cond<'a>>,
    cond_expr: Option<WithSpan<'a, Expr<'a>>>,
    generate_condition: bool,
    generate_content: bool,
}

struct Conds<'a> {
    conds: Vec<CondInfo<'a>>,
    ws_before: Option<Ws>,
    ws_after: Option<Ws>,
    nb_conds: usize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum EvaluatedResult {
    AlwaysTrue,
    AlwaysFalse,
    Unknown,
}

impl<'a> Conds<'a> {
    fn compute_branches(generator: &Generator<'a, '_>, i: &'a If<'a>) -> Self {
        let mut conds = Vec::with_capacity(i.branches.len());
        let mut ws_before = None;
        let mut ws_after = None;
        let mut nb_conds = 0;
        let mut stop_loop = false;

        for cond in &i.branches {
            if stop_loop {
                ws_after = Some(cond.ws);
                break;
            }
            if let Some(CondTest {
                expr,
                contains_bool_lit_or_is_defined,
                ..
            }) = &cond.cond
            {
                let mut only_contains_is_defined = true;

                let (evaluated_result, cond_expr) = if *contains_bool_lit_or_is_defined {
                    let (evaluated_result, expr) =
                        generator.evaluate_condition(expr.clone(), &mut only_contains_is_defined);
                    (evaluated_result, Some(expr))
                } else {
                    (EvaluatedResult::Unknown, None)
                };

                match evaluated_result {
                    // We generate the condition in case some calls are changing a variable, but
                    // no need to generate the condition body since it will never be called.
                    //
                    // However, if the condition only contains "is (not) defined" checks, then we
                    // can completely skip it.
                    EvaluatedResult::AlwaysFalse => {
                        if only_contains_is_defined {
                            if conds.is_empty() && ws_before.is_none() {
                                // If this is the first `if` and it's skipped, we definitely don't
                                // want its whitespace control to be lost.
                                ws_before = Some(cond.ws);
                            }
                            continue;
                        }
                        nb_conds += 1;
                        conds.push(CondInfo {
                            cond,
                            cond_expr,
                            generate_condition: true,
                            generate_content: false,
                        });
                    }
                    // This case is more interesting: it means that we will always enter this
                    // condition, meaning that any following should not be generated. Another
                    // thing to take into account: if there are no if branches before this one,
                    // no need to generate an `else`.
                    EvaluatedResult::AlwaysTrue => {
                        let generate_condition = !only_contains_is_defined;
                        if generate_condition {
                            nb_conds += 1;
                        }
                        conds.push(CondInfo {
                            cond,
                            cond_expr,
                            generate_condition,
                            generate_content: true,
                        });
                        // Since it's always true, we can stop here.
                        stop_loop = true;
                    }
                    EvaluatedResult::Unknown => {
                        nb_conds += 1;
                        conds.push(CondInfo {
                            cond,
                            cond_expr,
                            generate_condition: true,
                            generate_content: true,
                        });
                    }
                }
            } else {
                let generate_condition = !conds.is_empty();
                if generate_condition {
                    nb_conds += 1;
                }
                conds.push(CondInfo {
                    cond,
                    cond_expr: None,
                    generate_condition,
                    generate_content: true,
                });
            }
        }
        Self {
            conds,
            ws_before,
            ws_after,
            nb_conds,
        }
    }
}

fn median(sizes: &mut [usize]) -> usize {
    if sizes.is_empty() {
        return 0;
    }
    sizes.sort_unstable();
    if sizes.len() % 2 == 1 {
        sizes[sizes.len() / 2]
    } else {
        (sizes[sizes.len() / 2 - 1] + sizes[sizes.len() / 2]) / 2
    }
}

fn macro_call_ensure_arg_count(
    call: &WithSpan<'_, Call<'_>>,
    def: &Macro<'_>,
    ctx: &Context<'_>,
) -> Result<(), CompileError> {
    if call.args.len() > def.args.len() {
        return Err(ctx.generate_error(
            format_args!(
                "macro `{}` expected {} argument{}, found {}",
                def.name,
                def.args.len(),
                if def.args.len() > 1 { "s" } else { "" },
                call.args.len(),
            ),
            call.span(),
        ));
    }

    // First we list of arguments position, then we remove every argument with a value.
    let mut args: Vec<_> = def.args.iter().map(|&(name, _)| Some(name)).collect();
    for (pos, arg) in call.args.iter().enumerate() {
        let pos = match **arg {
            Expr::NamedArgument(name, ..) => {
                def.args.iter().position(|(arg_name, _)| *arg_name == name)
            }
            _ => Some(pos),
        };
        if let Some(pos) = pos {
            if mem::take(&mut args[pos]).is_none() {
                // This argument was already passed, so error.
                return Err(ctx.generate_error(
                    format_args!(
                        "argument `{}` was passed more than once when calling macro `{}`",
                        def.args[pos].0, def.name,
                    ),
                    call.span(),
                ));
            }
        }
    }

    // Now we can check off arguments with a default value, too.
    for (pos, (_, dflt)) in def.args.iter().enumerate() {
        if dflt.is_some() {
            args[pos] = None;
        }
    }

    // Now that we have a needed information, we can print an error message (if needed).
    struct FmtMissing<'a, I> {
        count: usize,
        missing: I,
        name: &'a str,
    }

    impl<'a, I: Iterator<Item = &'a str> + Clone> fmt::Display for FmtMissing<'a, I> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if self.count == 1 {
                let a = self.missing.clone().next().unwrap();
                write!(
                    f,
                    "missing argument when calling macro `{}`: `{a}`",
                    self.name
                )
            } else {
                write!(f, "missing arguments when calling macro `{}`: ", self.name)?;
                for (idx, a) in self.missing.clone().enumerate() {
                    if idx == self.count - 1 {
                        write!(f, " and ")?;
                    } else if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "`{a}`")?;
                }
                Ok(())
            }
        }
    }

    let missing = args.iter().filter_map(Option::as_deref);
    let fmt_missing = FmtMissing {
        count: missing.clone().count(),
        missing,
        name: def.name,
    };
    if fmt_missing.count == 0 {
        Ok(())
    } else {
        Err(ctx.generate_error(fmt_missing, call.span()))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AstLevel {
    Top,
    Block,
    Nested,
}

/// Returns `true` if the outcome of this expression may be used multiple times in the same
/// `write!()` call, without evaluating the expression again, i.e. the expression should be
/// side-effect free.
fn is_cacheable(expr: &WithSpan<'_, Expr<'_>>) -> bool {
    match &**expr {
        // Literals are the definition of pure:
        Expr::BoolLit(_) => true,
        Expr::NumLit(_, _) => true,
        Expr::StrLit(_) => true,
        Expr::CharLit(_) => true,
        // fmt::Display should have no effects:
        Expr::Var(_) => true,
        Expr::Path(_) => true,
        // Check recursively:
        Expr::Array(args) => args.iter().all(is_cacheable),
        Expr::Attr(lhs, _) => is_cacheable(lhs),
        Expr::Index(lhs, rhs) => is_cacheable(lhs) && is_cacheable(rhs),
        Expr::Filter(Filter { arguments, .. }) => arguments.iter().all(is_cacheable),
        Expr::Unary(_, arg) => is_cacheable(arg),
        Expr::BinOp(_, lhs, rhs) => is_cacheable(lhs) && is_cacheable(rhs),
        Expr::IsDefined(_) | Expr::IsNotDefined(_) => true,
        Expr::Range(_, lhs, rhs) => {
            lhs.as_ref().is_none_or(|v| is_cacheable(v))
                && rhs.as_ref().is_none_or(|v| is_cacheable(v))
        }
        Expr::Group(arg) => is_cacheable(arg),
        Expr::Tuple(args) => args.iter().all(is_cacheable),
        Expr::NamedArgument(_, expr) => is_cacheable(expr),
        Expr::As(expr, _) => is_cacheable(expr),
        Expr::Try(expr) => is_cacheable(expr),
        Expr::Concat(args) => args.iter().all(is_cacheable),
        // Doesn't make sense in this context.
        Expr::LetCond(_) => false,
        // We have too little information to tell if the expression is pure:
        Expr::Call { .. } => false,
        Expr::RustMacro(_, _) => false,
        // Should never be encountered:
        Expr::FilterSource => unreachable!("FilterSource in expression?"),
        Expr::ArgumentPlaceholder => unreachable!("ExpressionPlaceholder in expression?"),
    }
}
