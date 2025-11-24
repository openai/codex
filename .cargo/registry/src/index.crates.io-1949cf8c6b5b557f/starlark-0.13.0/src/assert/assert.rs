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

//! Utilities to test Starlark code execution.

// `if_then_panic` is only in newer clippy, delete this in future.
#![allow(unknown_lints)]
// We want to carefully control the panic message.
#![allow(clippy::if_then_panic)]

use std::collections::HashMap;

use dupe::Dupe;
use maplit::hashmap;
use once_cell::sync::Lazy;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::codemap::FileSpanRef;
use crate::environment::FrozenModule;
use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ReturnFileLoader;
use crate::stdlib::PrintHandler;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::none::NoneType;
use crate::values::structs::AllocStruct;
use crate::values::tuple::UnpackTuple;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::AllocValue;
use crate::values::Heap;
use crate::values::OwnedFrozenValue;
use crate::values::Value;

fn mk_environment() -> GlobalsBuilder {
    GlobalsBuilder::extended().with(test_functions)
}

static GLOBALS: Lazy<Globals> = Lazy::new(|| mk_environment().build());

static ASSERTS_STAR: Lazy<FrozenModule> = Lazy::new(|| {
    let g = GlobalsBuilder::new()
        .with_namespace("asserts", asserts_star)
        .build();
    let m = Module::new();
    m.frozen_heap().add_reference(g.heap());
    let asserts = g.get("asserts").unwrap();
    m.set("asserts", asserts);
    m.set(
        "freeze",
        asserts.get_attr("freeze", m.heap()).unwrap().unwrap(),
    );
    m.freeze().unwrap()
});

fn assert_equals<'v>(a: Value<'v>, b: Value<'v>) -> starlark::Result<NoneType> {
    if !a.equals(b)? {
        Err(anyhow::anyhow!("assert_eq: expected {}, got {}", a, b).into())
    } else {
        Ok(NoneType)
    }
}

fn assert_different<'v>(a: Value<'v>, b: Value<'v>) -> starlark::Result<NoneType> {
    if a.equals(b)? {
        Err(anyhow::anyhow!("assert_ne: but {} == {}", a, b).into())
    } else {
        Ok(NoneType)
    }
}

fn assert_less_than<'v>(a: Value<'v>, b: Value<'v>) -> starlark::Result<NoneType> {
    if a.compare(b)? != std::cmp::Ordering::Less {
        Err(anyhow::anyhow!("assert_lt: but {} >= {}", a, b).into())
    } else {
        Ok(NoneType)
    }
}

/// How often we garbage collection _should_ be transparent to the tests,
/// so we run each test in three configurations.
#[derive(Clone, Copy, Dupe, Debug)]
enum GcStrategy {
    Never,  // Disable GC
    Auto,   // Use the automatic heuristics (in practice, this does almost no GC)
    Always, // GC as aggressively as we can
}

/// Definitions to support assert.star as used by the Go test suite
#[starlark_module]
// Deliberately qualify the GlobalsBuild type to test that we can
fn asserts_star(builder: &mut crate::environment::GlobalsBuilder) {
    fn eq<'v>(a: Value<'v>, b: Value<'v>) -> starlark::Result<NoneType> {
        assert_equals(a, b)
    }

    fn ne<'v>(a: Value<'v>, b: Value<'v>) -> starlark::Result<NoneType> {
        assert_different(a, b)
    }

    fn lt<'v>(a: Value<'v>, b: Value<'v>) -> starlark::Result<NoneType> {
        assert_less_than(a, b)
    }

    fn contains<'v>(xs: Value<'v>, x: Value<'v>) -> starlark::Result<NoneType> {
        if !xs.is_in(x)? {
            Err(anyhow::anyhow!("assert.contains: expected {} to be in {}", x, xs).into())
        } else {
            Ok(NoneType)
        }
    }

    fn r#true(x: Value) -> starlark::Result<NoneType> {
        assert_equals(Value::new_bool(x.to_bool()), Value::new_bool(true))
    }

    // We don't allow this at runtime - just to be compatible with the Go Starlark test suite
    fn freeze<'v>(x: Value<'v>) -> anyhow::Result<Value<'v>> {
        Ok(x)
    }

    fn fails<'v>(
        f: Value<'v>,
        msg: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let _ = msg;
        match f.invoke_pos(&[], eval) {
            Err(_e) => Ok(NoneType), // We don't actually check the message
            Ok(_) => Err(anyhow::anyhow!("assert.fails: didn't fail")),
        }
    }
}

#[starlark_module]
pub(crate) fn test_functions(builder: &mut GlobalsBuilder) {
    // Used by one of the test methods in Go
    const fibonacci: Vec<i32> = vec![0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89];

    // Approximate version of a method used by the Go test suite
    fn hasfields<'v>() -> anyhow::Result<impl AllocValue<'v>> {
        Ok(AllocStruct::EMPTY)
    }

    fn assert_eq<'v>(a: Value<'v>, b: Value<'v>) -> starlark::Result<NoneType> {
        assert_equals(a, b)
    }

    fn assert_ne<'v>(a: Value<'v>, b: Value<'v>) -> starlark::Result<NoneType> {
        assert_different(a, b)
    }

    fn assert_lt<'v>(a: Value<'v>, b: Value<'v>) -> starlark::Result<NoneType> {
        assert_less_than(a, b)
    }

    fn assert_true(a: Value) -> anyhow::Result<NoneType> {
        if !a.to_bool() {
            Err(anyhow::anyhow!("assertion failed"))
        } else {
            Ok(NoneType)
        }
    }

    fn assert_false(a: Value) -> anyhow::Result<NoneType> {
        if a.to_bool() {
            Err(anyhow::anyhow!("assertion failed"))
        } else {
            Ok(NoneType)
        }
    }

    // This is only safe to call at the top-level of a Starlark module
    fn garbage_collect(eval: &mut Evaluator) -> anyhow::Result<NoneType> {
        eval.trigger_gc();
        Ok(NoneType)
    }

    fn assert_type<'v>(v: Value<'v>, ty: Value<'v>, heap: &'v Heap) -> starlark::Result<NoneType> {
        TypeCompiled::new(ty, heap)?.check_type(v, Some("v"))?;
        Ok(NoneType)
    }

    /// Function which consumes arguments and that's it.
    ///
    /// This function is unknown to optimizer, so it can be used in optimizer tests.
    fn noop<'v>(
        #[starlark(args)] args: UnpackTuple<Value<'v>>,
        #[starlark(kwargs)] kwargs: Value<'v>,
    ) -> anyhow::Result<Value<'v>> {
        let _ = kwargs;
        Ok(args.items.into_iter().next().unwrap_or(Value::new_none()))
    }
}

/// Environment in which to run assertion tests.
pub struct Assert<'a> {
    dialect: Dialect,
    modules: HashMap<String, FrozenModule>,
    globals: Globals,
    gc_strategy: Option<GcStrategy>,
    setup_eval: Box<dyn Fn(&mut Evaluator)>,
    // Ideally `print_handler` should be set up in `setup_eval`
    // but if you know how to do it, show me how.
    print_handler: Option<&'a (dyn PrintHandler + 'a)>,
    static_typechecking: bool,
}

/// Construction and state management.
impl<'a> Assert<'a> {
    /// Create a new assert object, which will by default use
    /// extended dialect and all library extensions,
    /// plus some additional global functions like `assert_eq`.
    /// The usual pattern is to create a `mut` `Assert`, modify some properties
    /// and then execute some tests.
    pub fn new() -> Self {
        Self {
            dialect: Dialect::AllOptionsInternal,
            modules: hashmap!["asserts.star".to_owned() => Lazy::force(&ASSERTS_STAR).dupe()],
            globals: Lazy::force(&GLOBALS).dupe(),
            gc_strategy: None,
            setup_eval: Box::new(|_| ()),
            print_handler: None,
            static_typechecking: true,
        }
    }

    /// Disable garbage collection on the tests.
    pub fn disable_gc(&mut self) {
        self.gc_strategy = Some(GcStrategy::Never)
    }

    /// Configure a callback which is used to setup evaluator before each evaluation.
    pub fn setup_eval(&mut self, setup: impl Fn(&mut Evaluator) + 'static) {
        self.setup_eval = Box::new(setup);
    }

    /// Configure the handler for `print` function.
    pub fn set_print_handler(&mut self, handler: &'a (dyn PrintHandler + 'a)) {
        self.print_handler = Some(handler);
    }

    /// Disable static typechecking for test. It is off by default in `Evaluator`,
    /// but on by default in `Assert`.
    pub fn disable_static_typechecking(&mut self) {
        self.static_typechecking = false;
    }

    fn with_gc<A>(&self, f: impl Fn(GcStrategy) -> A) -> A {
        match self.gc_strategy {
            None => {
                // We want to run with Auto first, and use that as the result, because that's the default
                let res = f(GcStrategy::Auto);
                f(GcStrategy::Never);
                f(GcStrategy::Always);
                res
            }
            Some(x) => f(x),
        }
    }

    fn execute<'v>(
        &self,
        path: &str,
        program: &str,
        module: &'v Module,
        gc: GcStrategy,
    ) -> crate::Result<Value<'v>> {
        let mut modules = HashMap::with_capacity(self.modules.len());
        for (k, v) in &self.modules {
            modules.insert(k.as_str(), v);
        }
        let loader = ReturnFileLoader { modules: &modules };
        let ast = AstModule::parse(path, program.to_owned(), &self.dialect)?;
        let gc_always = |_span: FileSpanRef, eval: &mut Evaluator| {
            eval.trigger_gc();
        };
        let mut eval = Evaluator::new(module);
        eval.enable_static_typechecking(self.static_typechecking);
        (self.setup_eval)(&mut eval);
        if let Some(print_handler) = self.print_handler {
            eval.set_print_handler(print_handler);
        }

        match gc {
            GcStrategy::Never => eval.disable_gc(),
            GcStrategy::Auto => {}
            GcStrategy::Always => eval.before_stmt_fn(&gc_always),
        }
        eval.set_loader(&loader);
        eval.eval_module(ast, &self.globals).map_err(Into::into)
    }

    fn execute_fail<'v>(
        &self,
        func: &str,
        program: &str,
        module: &'v Module,
        gc: GcStrategy,
    ) -> crate::Error {
        match self.execute("assert.bzl", program, module, gc) {
            Ok(v) => panic!(
                "starlark::assert::{}, didn't fail!\nCode:\n{}\nResult:\n{}\n",
                func, program, v
            ),
            Err(e) => e,
        }
    }

    fn execute_unwrap<'v>(
        &self,
        func: &str,
        path: &str,
        program: &str,
        module: &'v Module,
        gc: GcStrategy,
    ) -> Value<'v> {
        match self.execute(path, program, module, gc) {
            Ok(v) => v,
            Err(err) => {
                err.eprint();
                panic!(
                    "starlark::assert::{}, failed to execute!\nCode:\n{}\nGot error: {}",
                    func, program, err
                );
            }
        }
    }

    fn execute_unwrap_true<'v>(
        &self,
        func: &str,
        program: &str,
        module: &'v Module,
        gc: GcStrategy,
    ) {
        let v = self.execute_unwrap(func, "assert.bzl", program, module, gc);
        match v.unpack_bool() {
            Some(true) => {}
            Some(false) => panic!("starlark::assert::{}, got false!\nCode:\n{}", func, program),
            None => panic!(
                "starlark::assert::{}, not a bool!\nCode:\n{}\nResult\n{}",
                func, program, v
            ),
        }
    }

    fn execute_unwrap_false<'v>(
        &self,
        func: &str,
        program: &str,
        module: &'v Module,
        gc: GcStrategy,
    ) {
        let v = self.execute_unwrap(func, "assert.bzl", program, module, gc);
        match v.unpack_bool() {
            Some(false) => {}
            Some(true) => panic!("starlark::assert::{}, got true!\nCode:\n{}", func, program),
            None => panic!(
                "starlark::assert::{}, not a bool!\nCode:\n{}\nResult\n{}",
                func, program, v
            ),
        }
    }

    /// Set the [`Dialect`] that future tests will use.
    pub fn dialect(&mut self, x: &Dialect) {
        self.dialect = x.clone();
    }

    /// Set specific fields in the [`Dialect`] that future tests will use.
    pub fn dialect_set(&mut self, f: impl FnOnce(&mut Dialect)) {
        f(&mut self.dialect)
    }

    /// Add a [`FrozenModule`] to the environment that future tests can access via
    /// `load`. To construct the [`FrozenModule`] automatically use [`module`](Assert::module).
    pub fn module_add(&mut self, name: &str, module: FrozenModule) {
        self.modules.insert(name.to_owned(), module);
    }

    /// Add a module to the environment that future tests can access.
    ///
    /// ```
    /// # use starlark::assert::Assert;
    /// let mut a = Assert::new();
    /// a.module("hello.star", "hello = 'world'");
    /// a.is_true("load('hello.star', 'hello'); hello == 'world'");
    /// ```
    pub fn module(&mut self, name: &str, program: &str) -> FrozenModule {
        let module = self
            .with_gc(|gc| {
                let module = Module::new();
                self.execute_unwrap("module", &format!("{}.bzl", name), program, &module, gc);
                module.freeze()
            })
            .expect("error freezing module");
        self.module_add(name, module.dupe());
        module
    }

    /// Set the [`Globals`] that future tests have access to.
    pub fn globals(&mut self, x: Globals) {
        self.globals = x;
    }

    /// Modify the [`Globals`] that future tests have access to.
    /// Note that this method will start from the default environment for [`Assert`],
    /// ignoring any previous [`globals`](Assert::globals) or [`globals_add`](Assert::globals_add) calls.
    pub fn globals_add(&mut self, f: impl FnOnce(&mut GlobalsBuilder)) {
        self.globals(mk_environment().with(f).build())
    }

    fn fails_with_name(&self, func: &str, program: &str, msgs: &[&str]) -> crate::Error {
        self.with_gc(|gc| {
            let module_env = Module::new();
            let original = self.execute_fail(func, program, &module_env, gc);
            // We really want to check the error message, but if in our doc tests we do:
            // fail("bad") # error: magic
            // Then when we print the source code, magic is contained in the error message.
            // Therefore, find the internals.
            let inner = original.without_diagnostic();
            let err_msg = format!("{:#}", inner);
            for msg in msgs {
                if !err_msg.contains(msg) {
                    original.eprint();
                    panic!(
                    "starlark::assert::{}, failed with the wrong message!\nCode:\n{}\nError:\n{:#}\nMissing:\n{}\nExpected:\n{:?}",
                    func, program, inner, msg, msgs
                )
                }
            }
            drop(inner);
            original
        })
    }
}

/// Execution tests.
impl<'a> Assert<'a> {
    /// A program that must fail with an error message that contains a specific
    /// string. Remember that the purpose of `fail` is to ensure you get
    /// the right error, not to fully specify the error - usually only one or
    /// two words will be sufficient to ensure that.
    ///
    /// ```
    /// # use starlark::assert::Assert;
    /// Assert::new().fail("fail('hello')", "ello");
    /// ```
    pub fn fail(&self, program: &str, msg: &str) -> crate::Error {
        self.fails_with_name("fail", program, &[msg])
    }

    /// A program that must fail with an error message that contains a specific
    /// set of strings. Remember that the purpose of `fail` is to ensure you get
    /// the right error, not to fully specify the error - usually only one or
    /// two words will be sufficient to ensure that. The words do not have to be
    /// in order.
    ///
    /// ```
    /// # use starlark::assert::Assert;
    /// Assert::new().fails("fail('hello')", &["fail", "ello"]);
    /// ```
    pub fn fails(&self, program: &str, msgs: &[&str]) -> crate::Error {
        self.fails_with_name("fails", program, msgs)
    }

    /// A program that must execute successfully without an exception. Often uses
    /// assert_eq. Returns the resulting value.
    ///
    /// ```
    /// # use starlark::assert::Assert;
    /// Assert::new().pass("assert_eq(1, 1)");
    /// ```
    pub fn pass(&self, program: &str) -> OwnedFrozenValue {
        self.with_gc(|gc| {
            let env = Module::new();
            let res = self.execute_unwrap("pass", "assert.bzl", program, &env, gc);
            env.set("_", res);
            env.freeze()
                .expect("error freezing module")
                .get("_")
                .unwrap()
        })
    }

    /// A program that must execute successfully without an exception. Returns the frozen module
    /// that `program` was evaluated in.
    pub fn pass_module(&self, program: &str) -> FrozenModule {
        self.with_gc(|gc| {
            let env = Module::new();
            self.execute_unwrap("pass", "assert.bzl", program, &env, gc);
            env.freeze().expect("error freezing module")
        })
    }

    /// A program that must evaluate to `True`.
    ///
    /// ```
    /// # use starlark::assert::Assert;
    /// Assert::new().is_true(
    ///     r#"
    /// x = 1 + 1
    /// x == 2
    /// "#,
    /// );
    /// ```
    pub fn is_true(&self, program: &str) {
        self.with_gc(|gc| {
            let env = Module::new();
            self.execute_unwrap_true("is_true", program, &env, gc);
        })
    }

    /// A program that must evaluate to `False`.
    pub fn is_false(&self, program: &str) {
        self.with_gc(|gc| {
            let env = Module::new();
            self.execute_unwrap_false("is_false", program, &env, gc);
        })
    }

    /// Many lines, each of which must individually evaluate to `True` (or be blank lines).
    ///
    /// ```
    /// # use starlark::assert::Assert;
    /// Assert::new().all_true(
    ///     r#"
    /// 1 == 1
    ///
    /// 2 == 1 + 1
    /// "#,
    /// );
    /// ```
    pub fn all_true(&self, program: &str) {
        self.with_gc(|gc| {
            for s in program.lines() {
                if s == "" {
                    continue;
                }
                let env = Module::new();
                self.execute_unwrap_true("all_true", s, &env, gc);
            }
        })
    }

    /// Two programs that must evaluate to the same (non-error) result.
    ///
    /// ```
    /// # use starlark::assert::Assert;
    /// Assert::new().eq("1 + 1", "2");
    /// ```
    pub fn eq(&self, lhs: &str, rhs: &str) {
        self.with_gc(|gc| {
            let lhs_m = Module::new();
            let rhs_m = Module::new();
            let lhs_v = self.execute_unwrap("eq", "lhs.bzl", lhs, &lhs_m, gc);
            let rhs_v = self.execute_unwrap("eq", "rhs.bzl", rhs, &rhs_m, gc);
            if lhs_v != rhs_v {
                panic!(
                "starlark::assert::eq, values differ!\nCode 1:\n{}\nCode 2:\n{}\nValue 1:\n{}\nValue 2\n{}",
                lhs, rhs, lhs_v, rhs_v
            );
            }
        })
    }
}

/// See [`Assert::eq`].
pub fn eq(lhs: &str, rhs: &str) {
    Assert::new().eq(lhs, rhs)
}

/// See [`Assert::fail`].
pub fn fail(program: &str, msg: &str) -> crate::Error {
    Assert::new().fail(program, msg)
}

#[cfg(test)]
pub(crate) fn fail_golden(path: &str, program: &str) -> crate::Error {
    let program = program.trim();
    let e = fails(program, &[]);
    let output = format!("Program:\n\n{program}\n\nError:\n\n{e:?}\n");
    starlark_syntax::golden_test_template::golden_test_template(path, &output);
    e
}

#[cfg(test)]
pub(crate) fn fail_skip_typecheck(program: &str, msg: &str) -> crate::Error {
    let mut a = Assert::new();
    a.disable_static_typechecking();
    a.fail(program, msg)
}

/// See [`Assert::fails`].
pub fn fails(program: &str, msgs: &[&str]) -> crate::Error {
    Assert::new().fails(program, msgs)
}

#[cfg(test)]
pub(crate) fn fails_skip_typecheck(program: &str, msgs: &[&str]) -> crate::Error {
    let mut a = Assert::new();
    a.disable_static_typechecking();
    a.fails(program, msgs)
}

/// See [`Assert::is_true`].
pub fn is_true(program: &str) {
    Assert::new().is_true(program)
}

/// See [`Assert::is_false`].
pub fn is_false(program: &str) {
    Assert::new().is_false(program)
}

#[cfg(test)]
pub(crate) fn is_true_skip_typecheck(program: &str) {
    let mut a = Assert::new();
    a.disable_static_typechecking();
    a.is_true(program)
}

/// See [`Assert::all_true`].
pub fn all_true(expressions: &str) {
    let mut a = Assert::new();
    // TODO(nga): fix and enable.
    a.disable_static_typechecking();
    a.all_true(expressions)
}

/// See [`Assert::pass`].
pub fn pass(program: &str) -> OwnedFrozenValue {
    Assert::new().pass(program)
}

/// See [`Assert::pass_module`].
pub fn pass_module(program: &str) -> FrozenModule {
    Assert::new().pass_module(program)
}
