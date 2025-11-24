/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

// There are two improvements that we could make:
//
// 1. Use the existing name resolution code, so we don't end up duplicating that logic.
// 2. Extract a flow graph rather than doing a fixed point.
//
// But it does as things stand.

use std::cmp;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::mem;

use dupe::Dupe;
use maplit::hashset;
use starlark_syntax::syntax::ast::AssignP;
use starlark_syntax::syntax::ast::AstAssignIdent;
use starlark_syntax::syntax::ast::AstAssignTarget;
use starlark_syntax::syntax::ast::AstExpr;
use starlark_syntax::syntax::ast::AstIdent;
use starlark_syntax::syntax::ast::AstStmt;
use starlark_syntax::syntax::ast::AstTypeExpr;
use starlark_syntax::syntax::ast::Clause;
use starlark_syntax::syntax::ast::Expr;
use starlark_syntax::syntax::ast::ForClause;
use starlark_syntax::syntax::ast::ForP;
use starlark_syntax::syntax::ast::LoadArgP;
use starlark_syntax::syntax::ast::Stmt;
use starlark_syntax::syntax::module::AstModuleFields;
use thiserror::Error;

use crate::analysis::types::LintT;
use crate::analysis::types::LintWarning;
use crate::analysis::EvalSeverity;
use crate::codemap::CodeMap;
use crate::codemap::Span;
use crate::codemap::Spanned;
use crate::syntax::AstModule;

#[derive(Error, Debug)]
pub(crate) enum NameWarning {
    #[error("Unused `load` of `{0}`")]
    UnusedLoad(String),
    #[error("Unused assignment of `{0}`")]
    UnusedAssign(String),
    #[error("Unused argument `{0}`")]
    UnusedArgument(String),
    #[error("Use of unassigned variable `{0}`")]
    UsingUnassigned(String),
    #[error("Use of undefined variable `{0}`")]
    UsingUndefined(String),
    #[error("Use of potentially undefined variable `{0}`")]
    UsingMaybeUndefined(String),
}

impl LintWarning for NameWarning {
    fn severity(&self) -> EvalSeverity {
        match self {
            Self::UsingUnassigned(..) | Self::UsingMaybeUndefined(..) => EvalSeverity::Warning,
            _ => EvalSeverity::Disabled,
        }
    }

    fn short_name(&self) -> &'static str {
        match self {
            Self::UnusedLoad(..) => "unused-load",
            Self::UnusedAssign(..) => "unused-assign",
            Self::UnusedArgument(..) => "unused-argument",
            Self::UsingUnassigned(..) => "using-unassigned",
            Self::UsingUndefined(..) => "using-undefined",
            Self::UsingMaybeUndefined(..) => "using-maybe-undefined",
        }
    }
}

#[derive(Debug, Copy, Dupe, Clone, PartialEq, Eq)]
enum Kind {
    Load,
    Argument,
    Assign,
}

impl Kind {
    fn unused(self, name: String) -> NameWarning {
        match self {
            Kind::Load => NameWarning::UnusedLoad(name),
            Kind::Argument => NameWarning::UnusedArgument(name),
            Kind::Assign => NameWarning::UnusedAssign(name),
        }
    }
}

#[derive(Debug, Copy, Dupe, Clone, PartialEq, Eq)]
enum Assigned {
    Definitely,
    Maybe,
}

type AstStr<'a> = Spanned<&'a str>;

trait AstStrExt<'a> {
    fn new(span: Span, str: &'a str) -> Self;
    fn ident(x: &'a AstIdent) -> Self;
    fn assign_ident(x: &'a AstAssignIdent) -> Self;
}

impl<'a> AstStrExt<'a> for AstStr<'a> {
    fn new(span: Span, str: &'a str) -> Self {
        Spanned { span, node: str }
    }

    fn ident(x: &'a AstIdent) -> Self {
        Self::new(x.span, x.node.ident.as_str())
    }

    fn assign_ident(x: &'a AstAssignIdent) -> Self {
        Self::new(x.span, x.node.ident.as_str())
    }
}

/// When we see a control flow operator, how far does it apply?
#[derive(Copy, Dupe, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Abort {
    /// Abort the loop (e.g. `continue` or `break`)
    Loop,
    /// Abort the function (e.g. `return`)
    Function,
}

/// Is this function the global `fail` function.
/// Can technically be shadowed, but if you shadow it with something that doesn't fail
/// you're going to confuse users, so not too problematic.
fn is_fail(x: &AstExpr) -> bool {
    if let Expr::Call(x, _) = &**x {
        if let Expr::Identifier(x) = &***x {
            return x.ident.as_str() == "fail";
        }
    }
    false
}

/// A combination of the scope information (what is set) and state information as we
/// step through the function.
#[derive(Debug, Default)]
struct ScopeState<'a> {
    /// Those identifiers that weren't set at that point, so we went to the parent.
    /// If these are set later on, then it is a unassigned usage error.
    cant_set: Vec<AstStr<'a>>,
    /// Those identifiers that I couldn't find at the point I saw them.
    /// Since the child runs in a different order with respect to the parent, they might resolve later.
    unbound: Vec<AstStr<'a>>,
    /// Those definition sites that we have ended up using.
    used: HashSet<AstStr<'a>>,
    /// Those definitions that we have set.
    set: Vec<(AstStr<'a>, Kind)>,
    /// The last location/locations where I was set.
    /// The assigned is whether I am always set or not
    last_set: HashMap<&'a str, (Assigned, HashSet<Span>)>,
    /// Whether I can be reached
    abort: Option<Abort>,
}

/// The state we use when scanning the variables
struct State<'a> {
    codemap: &'a CodeMap,
    /// Those that are set in the global scope.
    /// If None then assume anything might be set at the global scope.
    globals: Option<&'a HashSet<String>>,
    /// These are the various scopes - one for the module, one for each def.
    scopes: Vec<ScopeState<'a>>,
    /// The current list of warnings.
    warnings: Vec<LintT<NameWarning>>,
    /// Thing things we have already warned about (no duplicates due to running loops twice)
    warned: HashSet<AstStr<'a>>,
    /// How many nested loops we are in
    loop_depth: usize,
}

impl<'a> State<'a> {
    fn add_warning(&mut self, ident: AstStr<'a>, ctor: impl Fn(String) -> NameWarning) {
        if self.warned.insert(ident) {
            self.warnings.push(LintT::new(
                self.codemap,
                ident.span,
                ctor(ident.node.to_owned()),
            ))
        }
    }

    // Scope stuff

    fn exit_scope(&mut self) {
        // The scope only collects things that could have been assigned, so should always be
        let scope = self.scopes.pop().unwrap();
        // Variables defined in the local scope
        let local: HashSet<&str> = scope.set.iter().map(|x| x.0.node).collect();

        // unset & set => we thought it was in the parent scope, but was actually undefined
        for x in &scope.cant_set {
            if local.contains(x.node) {
                self.add_warning(*x, NameWarning::UsingUnassigned);
            }
        }

        // some of our unbound variables were from our children
        // these might use any random variable at any point - impossible to know
        // so for those, just assume they are used
        let (unbound_defined, unbound_undefined) = scope
            .unbound
            .iter()
            .partition::<Vec<AstStr>, _>(|x| local.contains(x.node));
        let unbound_defined: HashSet<&str> = unbound_defined.iter().map(|x| x.node).collect();

        // set & !used => we set it but never used it in any branch
        // if we are in unbound,
        let top = self.scopes.is_empty();
        for (ident, kind) in scope.set {
            if !scope.used.contains(&ident) && !unbound_defined.contains(ident.node) {
                let underscore = ident.node.starts_with('_');
                let exported = top
                    && !underscore
                    && kind == Kind::Assign // Assume loads don't automatically export
                    && scope
                        .last_set
                        .get(ident.node)
                        .map_or(false, |x| x.1.contains(&ident.span));
                let ignored = !top && underscore;

                if !exported && !ignored {
                    self.add_warning(ident, |s| kind.unused(s));
                }
            }
        }

        // unbound & not defined => move to the parents unbound
        // unbound & no parent => undefined
        match self.scopes.last_mut() {
            None => {
                if let Some(globals) = self.globals {
                    for x in unbound_undefined {
                        if !globals.contains(x.node) {
                            self.add_warning(x, NameWarning::UsingUndefined)
                        }
                    }
                }
            }
            Some(parent) => {
                // these things were unbound, but perhaps in a child, and perhaps we defined it later
                // as we can use a variable in a child before we define it, since the child is a def,
                // so runs later.
                for x in unbound_undefined {
                    if !local.contains(x.node) {
                        parent.unbound.push(x);
                    }
                }
            }
        }
    }

    fn enter_scope(&mut self) {
        self.scopes.push(ScopeState::default());
    }

    // Nest stuff

    fn set_abort(&mut self, abort: Abort) {
        self.scopes.last_mut().unwrap().abort = Some(abort);
    }

    fn loops(&mut self, inner: impl Fn(&mut State<'a>)) {
        self.loop_depth += 1;
        // We run the loop twice since it might set a variable that is only used in subsequent iterations of the loop.
        // That means n nested loops are now O(2^n), but if n > 5, just give up and only iterate once
        // (a few too many warnings, bounded runtime).
        for _ in 0..(if self.loop_depth > 5 { 1 } else { 2 }) {
            self.branch(&inner, |_| ());
            let scope = self.scopes.last_mut().unwrap();
            if scope.abort == Some(Abort::Loop) {
                scope.abort = None;
            }
        }
        self.loop_depth -= 1;
    }

    fn branch(&mut self, opt1: impl Fn(&mut State<'a>), opt2: impl Fn(&mut State<'a>)) {
        let original_abort = self.scopes.last().unwrap().abort;
        let mut other = self.scopes.last().unwrap().last_set.clone();
        opt1(self);
        let other_abort = self.scopes.last().unwrap().abort;
        mem::swap(&mut other, &mut self.scopes.last_mut().unwrap().last_set);
        self.scopes.last_mut().unwrap().abort = original_abort;
        opt2(self);
        let current_abort = self.scopes.last().unwrap().abort;

        // now need to merge `current` with `other`
        self.scopes.last_mut().unwrap().abort = cmp::min(other_abort, current_abort);
        let current = &mut self.scopes.last_mut().unwrap().last_set;

        let join_assigned = |c, o| {
            match (current_abort.is_some(), other_abort.is_some()) {
                (true, true) => Assigned::Definitely, // Probably irrelevant
                (true, false) => o,
                (false, true) => c,
                (false, false) => {
                    if c == Assigned::Maybe {
                        Assigned::Maybe
                    } else {
                        o
                    }
                }
            }
        };

        let mut keys: HashSet<&str> = current.keys().copied().collect();
        keys.extend(other.keys().copied());
        for k in keys {
            match current.entry(k) {
                Entry::Vacant(e) => {
                    if let Some(x) = other.remove(k) {
                        e.insert((join_assigned(Assigned::Maybe, x.0), x.1));
                    }
                }
                Entry::Occupied(mut e) => match other.remove(k) {
                    None => {
                        e.get_mut().0 = join_assigned(e.get().0, Assigned::Maybe);
                    }
                    Some((assigned, spans)) => {
                        e.get_mut().0 = join_assigned(e.get().0, assigned);
                        for span in spans {
                            e.get_mut().1.insert(span);
                        }
                    }
                },
            }
        }
    }

    // Actual operations

    fn use_ident(&mut self, ident: AstStr<'a>) {
        for (depth, scope) in self.scopes.iter_mut().rev().enumerate() {
            match scope.last_set.get(ident.node) {
                None => {
                    if depth == 0 {
                        scope.cant_set.push(ident)
                    }
                }
                Some((assigned, spans)) => {
                    for span in spans {
                        scope.used.insert(AstStr::new(*span, ident.node));
                    }
                    if *assigned == Assigned::Maybe {
                        self.add_warning(ident, NameWarning::UsingMaybeUndefined)
                    }
                    return;
                }
            }
        }
        self.scopes.last_mut().unwrap().unbound.push(ident);
    }

    fn set_ident(&mut self, ident: &'a AstAssignIdent, kind: Kind) {
        let ident = AstStr::assign_ident(ident);
        let scope = self.scopes.last_mut().unwrap();
        scope.set.push((ident, kind));
        scope
            .last_set
            .insert(ident.node, (Assigned::Definitely, hashset![ident.span]));
    }

    // Traverse the syntax tree

    fn assign(&mut self, assign: &'a AstAssignTarget) {
        assign.visit_expr(|x| self.expr(x));
        assign.visit_lvalue(|x| self.set_ident(x, Kind::Assign))
    }

    fn comprehension(
        &mut self,
        res1: &'a AstExpr,
        res2: Option<&'a AstExpr>,
        for_: &'a ForClause,
        clauses: &'a [Clause],
    ) {
        self.expr(&for_.over);
        self.enter_scope();
        // This isn't quite right, as we assume the comprehensions are always evaluated (e.g. no zero arrays)
        // and the assign is always hit, which isn't true.
        // But it is a close enough approximation, and comprehensions tend not to have variable assignment issues.
        self.assign(&for_.var);
        for clause in clauses {
            match clause {
                Clause::For(ForClause { var, over }) => {
                    self.expr(over);
                    self.assign(var);
                }
                Clause::If(x) => self.expr(x),
            }
        }
        self.expr(res1);
        self.expr_opt(res2);
        self.exit_scope();
    }

    fn expr(&mut self, expr: &'a AstExpr) {
        match &**expr {
            Expr::Identifier(ident) => self.use_ident(AstStr::ident(ident)),
            Expr::Lambda(x) => {
                for p in &x.params {
                    p.node.visit_expr(|e| self.expr(e));
                }
                self.enter_scope();
                for p in &x.params {
                    if let Some(pname) = p.node.ident() {
                        self.set_ident(pname, Kind::Argument);
                    }
                }
                self.expr(&x.body);
                self.exit_scope();
            }
            Expr::ListComprehension(a, b, c) => self.comprehension(a, None, b, c),
            Expr::DictComprehension(a, b, c) => self.comprehension(&a.0, Some(&a.1), b, c),
            _ => expr.visit_expr(|x| self.expr(x)),
        }
    }

    fn expr_opt(&mut self, expr: Option<&'a AstExpr>) {
        if let Some(x) = expr {
            self.expr(x)
        }
    }

    fn typ_opt(&mut self, ty: Option<&'a AstTypeExpr>) {
        if let Some(x) = ty {
            self.typ(x)
        }
    }

    fn typ(&mut self, ty: &'a AstTypeExpr) {
        self.expr(&ty.expr)
    }

    fn assign_as_expr(&mut self, assign: &'a AstAssignTarget) {
        assign.visit_expr(|x| self.expr(x));
        assign.visit_lvalue(|x| self.use_ident(AstStr::assign_ident(x)));
    }

    fn stmt(&mut self, stmt: &'a AstStmt) {
        match &**stmt {
            Stmt::Expression(x) => {
                self.expr(x);
                if is_fail(x) {
                    self.set_abort(Abort::Function);
                }
            }
            Stmt::Return(x) => {
                self.expr_opt(x.as_ref());
                self.set_abort(Abort::Function);
            }
            Stmt::Assign(AssignP { lhs, ty, rhs }) => {
                self.typ_opt(ty.as_ref());
                self.expr(rhs);
                self.assign(lhs);
            }
            Stmt::AssignModify(lhs, _, rhs) => {
                self.expr(rhs);
                self.assign_as_expr(lhs);
                self.assign(lhs);
            }
            Stmt::Statements(xs) => {
                for x in xs {
                    self.stmt(x)
                }
            }
            Stmt::If(cond, t) => {
                self.expr(cond);
                self.branch(|me| me.stmt(t), |_| ());
            }
            Stmt::IfElse(cond, t_f) => {
                self.expr(cond);
                self.branch(|me| me.stmt(&t_f.0), |me| me.stmt(&t_f.1));
            }
            Stmt::For(ForP { var, over, body }) => {
                self.expr(over);
                // Note this isn't 100% correct, as a for loop may set something the next iteration consumes
                self.loops(|me| {
                    me.assign(var);
                    me.stmt(body);
                });
            }
            Stmt::Def(x) => {
                for p in &x.params {
                    p.node.visit_expr(|e| self.expr(e));
                }
                self.typ_opt(x.return_type.as_deref());
                self.set_ident(&x.name, Kind::Assign);
                self.enter_scope();
                for p in &x.params {
                    if let Some(pname) = p.node.ident() {
                        self.set_ident(pname, Kind::Argument);
                    }
                }
                self.stmt(&x.body);
                self.exit_scope();
            }
            // These were handled by collecting the scopes
            Stmt::Load(x) => {
                for LoadArgP { local, .. } in &x.args {
                    self.set_ident(local, Kind::Load)
                }
            }
            // These control flow operators can be ignored - either the code after is fine (no problem)
            // or in error (in which case you have useless code after flow control)
            Stmt::Break | Stmt::Continue => {
                self.set_abort(Abort::Loop);
            }
            Stmt::Pass => {}
        }
    }

    fn module(&mut self, module: &'a AstModule) {
        self.enter_scope();
        self.stmt(module.statement());
        self.exit_scope();
    }
}

pub(crate) fn lint(
    module: &AstModule,
    globals: Option<&HashSet<String>>,
) -> Vec<LintT<NameWarning>> {
    let mut state = State {
        codemap: module.codemap(),
        globals,
        scopes: Vec::new(),
        warnings: Vec::new(),
        warned: HashSet::new(),
        loop_depth: 0,
    };
    state.module(module);
    state.warnings
}

#[cfg(test)]
mod tests {
    use starlark_syntax::slice_vec_ext::SliceExt;

    use super::*;
    use crate::syntax::Dialect;

    impl NameWarning {
        fn about(&self) -> &String {
            match self {
                NameWarning::UnusedLoad(x) => x,
                NameWarning::UnusedAssign(x) => x,
                NameWarning::UnusedArgument(x) => x,
                NameWarning::UsingUnassigned(x) => x,
                NameWarning::UsingUndefined(x) => x,
                NameWarning::UsingMaybeUndefined(x) => x,
            }
        }
    }

    fn module(x: &str) -> AstModule {
        AstModule::parse("X", x.to_owned(), &Dialect::AllOptionsInternal).unwrap()
    }

    #[test]
    fn test_lint_unused() {
        // unused kind arg is fine if it starts with _
        // allowed to ignore things which are

        let m = module(
            r#"
load("test", "no1", "a")
b = a
_c = 18
_no2 = 17
d = _c
_e = 9
_no4 = 1
def magic(no3, f, _allowed):
    _ignore = f
    _no4 = f # shadows out _no4, which is thus unused
    no5 = 10
    (a, _) = f()
    (a2, _ignored) = f()
    f.go(_e, a, a2, _no4)
def uses_h():
    _h
_h = []
def dots(f, g):
    (d_a, d_b, d_c, d_d, d_e, d_f, d_g, d_h, d_i, d_j) = (20, 21, 22, 23, 24, 25, 26, 27, 28, 29)
    # Make sure that expressions on dotted accesses are accounted for.
    # Ignore that the integers referenced aren't functions. They're just names.
    f.foo[d_a].bar
    f.foo[d_b].bar[d_c]
    f.baz(g(["ignore", d_d]))
    f.foobar.baz(d_e, [d_f], d_g([d_h]))
    d_i(d_j).blah
    pass
def _no6(): pass
def foo():
    array = [1,2,3]
    x = 1
    array[x] = 9
    return array
"#,
        );
        let res = lint(&m, None);
        let mut res = res.map(|x| x.problem.about());
        res.sort();
        assert_eq!(res, &["_no2", "_no4", "_no6", "no1", "no3", "no5"]);
    }

    #[test]
    fn test_lint_duplicate_assign() {
        let m = module(
            r#"
load("test", "no1", "a")
no1 = 12
b = a
no2 = 8
no2 = 9
c = 8
c += 1
def foo():
    no3 = 1
    no3 = 1
    _ignore = a
    _ignore = a
    if foo2:
        d = 1
    else:
        d = 2
    _use = (no3, d)
no2 = 10
_no4 = 1
_no4 = 2
export = _no4

def capture_e(): print(e)
e = 8
capture_e()
e = 10
capture_e()
"#,
        );
        let res = lint(&m, None);
        let mut res = res.map(|x| x.problem.about());
        res.sort();
        assert_eq!(res, &["_no4", "no1", "no2", "no2", "no3"]);
    }

    #[test]
    fn test_lint_unassigned() {
        let m = module(
            r#"
a = no1
no1 = 9
test = no3()
def foo():
    b = no2 + undefined_variable
    for x in xs:
        no2 = 18 + x + b
        _use = no2
def no3(): pass
def uses_h():
    _h
_h = []
"#,
        );
        let res = lint(&m, None);
        let mut res = res.map(|x| x.problem.about());
        res.sort();
        assert_eq!(res, &["no1", "no2", "no3"]);
    }

    #[test]
    fn test_lint_undefined() {
        let globals = HashSet::from(["True".to_owned(), "fail".to_owned()]);

        let m = module(
            r#"
load("test", imported = "more")
a = True + imported + no1
def foo():
    fail("test") + no2(t = 3)
def bar(ctx):
    ctx.attrs.dep[fail.index].default_outputs[0]
    ctx[0].default_outputs
"#,
        );
        let res = lint(&m, Some(&globals));
        let mut res = res.map(|x| x.problem.about());
        res.sort();
        assert_eq!(res, &["no1", "no2"])
    }

    #[test]
    fn test_early_fail() {
        let m = module(
            r#"
def foo(x):
    if x == 1:
        ok1 = 2
    elif x == 3:
        ok1 = 7
    else:
        fail("bad")
    return ok1

def bar(xs):
    for x in xs:
        if x == 1:
            ok2 = 2
        else:
            continue
        print(ok2)
"#,
        );
        let res = lint(&m, None);
        let res = res.map(|x| x.problem.about());
        assert_eq!(res, Vec::<&String>::new())
    }

    #[test]
    fn test_assign_for_next() {
        let m = module(
            r#"
def foo(xs):
    counter = 0
    reached = None
    for x in xs:
        if counter == 7:
            reached = x
        counter += 1
    return reached
"#,
        );
        let res = lint(&m, None);
        let res = res.map(|x| x.problem.about());
        assert_eq!(res, Vec::<&String>::new())
    }

    #[test]
    fn test_flow_control() {
        let m = module(
            r#"
def foo(b, xs):
    if b:
        no1 = xs
    for x in no1:
        return x
    for no2 in xs:
        print(no2)
    print(no2)
    if b:
        yes = xs
    else:
        yes = []
    return yes
"#,
        );
        let res = lint(&m, None);
        let res = res.map(|x| x.problem.about());
        assert_eq!(res, &["no1", "no2"])
    }

    #[test]
    fn test_lambda_capture() {
        let m = module(
            r#"
def foo():
    lam = lambda: print(x)
    x = 1
    lam()
    x = 2
    lam()
"#,
        );
        let res = lint(&m, None);
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn test_global_defined_later() {
        let m = module(
            r#"
def foo():
    _bar()
def _bar():
    pass
"#,
        );
        let res = lint(&m, Some(&HashSet::new()));
        assert_eq!(res.len(), 0);
    }
}
