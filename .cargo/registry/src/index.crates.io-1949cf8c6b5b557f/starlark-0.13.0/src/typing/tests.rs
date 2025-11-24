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

use std::collections::HashMap;
use std::fmt::Write;

use dupe::Dupe;
use starlark_derive::starlark_module;
use starlark_map::small_map::SmallMap;
use starlark_syntax::golden_test_template::golden_test_template;

use crate as starlark;
use crate::assert::Assert;
use crate::environment::FrozenModule;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::runtime::file_loader::ReturnOwnedFileLoader;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::tests::util::trim_rust_backtrace;
use crate::typing::callable_param::ParamIsRequired;
use crate::typing::interface::Interface;
use crate::typing::AstModuleTypecheck;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::util::ArcStr;
use crate::values::none::NoneType;
use crate::values::typing::StarlarkCallable;
use crate::values::typing::StarlarkCallableParamSpec;
use crate::values::typing::StarlarkIter;
use crate::values::Value;
use crate::values::ValueOfUnchecked;

mod call;
mod callable;
mod list;
mod special_function;
mod tuple;
mod types;

#[derive(Default)]
struct TypeCheck {
    expect_types: Vec<String>,
    loads: HashMap<String, (Interface, FrozenModule)>,
}

struct NamedXy;

impl StarlarkCallableParamSpec for NamedXy {
    fn params() -> ParamSpec {
        ParamSpec::new_named_only([
            (ArcStr::new_static("x"), ParamIsRequired::Yes, Ty::string()),
            (ArcStr::new_static("y"), ParamIsRequired::Yes, Ty::int()),
        ])
        .unwrap()
    }
}

#[starlark_module]
fn register_typecheck_globals(globals: &mut GlobalsBuilder) {
    fn accepts_iterable<'v>(
        #[starlark(require = pos)] xs: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
    ) -> anyhow::Result<NoneType> {
        let _ignore = xs;
        Ok(NoneType)
    }

    fn accepts_typed_kwargs(
        #[starlark(kwargs)] x: SmallMap<String, u32>,
    ) -> anyhow::Result<NoneType> {
        let _ignore = x;
        Ok(NoneType)
    }

    fn accepts_callable_named_xy<'v>(
        #[starlark(require = pos)] f: StarlarkCallable<'v, NamedXy, NoneType>,
    ) -> anyhow::Result<NoneType> {
        let _ignore = f;
        Ok(NoneType)
    }
}

impl TypeCheck {
    fn new() -> Self {
        Self::default()
    }

    fn ty(mut self, name: &str) -> Self {
        self.expect_types.push(name.to_owned());
        self
    }

    fn load(mut self, file: &str, interface: Interface, module: FrozenModule) -> Self {
        self.loads.insert(file.to_owned(), (interface, module));
        self
    }

    fn mk_file_loader(&self) -> ReturnOwnedFileLoader {
        let modules = self
            .loads
            .iter()
            .map(|(name, (_, module))| (name.clone(), module.dupe()))
            .collect();
        ReturnOwnedFileLoader { modules }
    }

    fn check(&self, test_name: &str, code: &str) -> (Interface, FrozenModule) {
        let globals = GlobalsBuilder::extended()
            .with(register_typecheck_globals)
            .build();
        let ast =
            AstModule::parse("filename", code.to_owned(), &Dialect::AllOptionsInternal).unwrap();
        let (errors, typemap, interface, approximations) = ast.clone().typecheck(
            &globals,
            &self
                .loads
                .iter()
                .map(|(name, (intf, _))| (name.clone(), intf.clone()))
                .collect(),
        );

        let mut output = String::new();
        writeln!(output, "Code:").unwrap();
        writeln!(output, "{}", code.trim()).unwrap();
        if errors.is_empty() {
            writeln!(output).unwrap();
            writeln!(output, "No errors.").unwrap();
        } else {
            for error in &errors {
                writeln!(output).unwrap();
                writeln!(output, "Error:").unwrap();
                // Note we are using `:#` here instead of `:?` because
                // `:?` includes rust backtrace.
                // The issue: https://github.com/dtolnay/anyhow/issues/300
                writeln!(output, "{}", format!("{:#}", error).trim_end()).unwrap();
            }
        }

        if !approximations.is_empty() {
            writeln!(output).unwrap();
            writeln!(output, "Approximations:").unwrap();
            for appox in approximations {
                writeln!(output, "{}", appox).unwrap();
            }
        }

        if !self.expect_types.is_empty() {
            writeln!(output).unwrap();
            writeln!(output, "Types:").unwrap();
            for k in &self.expect_types {
                let types = typemap.find_bindings_by_name(k);
                match types.as_slice() {
                    [ty] => writeln!(output, "{}: {}", k, ty).unwrap(),
                    [] => panic!("Type not found for {}", k),
                    [_, _, ..] => panic!("Multiple types found for {}", k),
                }
            }
        }

        let loader = self.mk_file_loader();
        let module = {
            writeln!(output).unwrap();
            writeln!(output, "Compiler typechecker (eval):").unwrap();
            let module = Module::new();
            let mut eval = Evaluator::new(&module);

            eval.set_loader(&loader);

            eval.enable_static_typechecking(true);
            let eval_result = eval.eval_module(ast, &globals);
            if eval_result.is_ok() != errors.is_empty() {
                writeln!(output, "Compiler typechecker and eval results mismatch.").unwrap();
                writeln!(output).unwrap();
            }

            // Additional writes must happen above this line otherwise it might be erased by trim_rust_backtrace
            match &eval_result {
                Ok(_) => writeln!(output, "No errors.").unwrap(),
                Err(err) => writeln!(output, "{:?}", err).unwrap(),
            }

            // Help borrow checker.
            drop(eval);

            module.freeze().unwrap()
        };

        golden_test_template(
            &format!("src/typing/tests/golden/{}.golden", test_name),
            trim_rust_backtrace(&output),
        );

        (interface, module)
    }
}

#[test]
fn test_success() {
    TypeCheck::default().ty("y").check(
        "success",
        r#"
def foo(x: str) -> str:
    return x.removeprefix("test")

def bar():
    y = hash(foo("magic"))
"#,
    );
}

#[test]
fn test_failure() {
    TypeCheck::new().check(
        "failure",
        r#"
def test():
    hash(1)
"#,
    );
}

#[test]
fn test_load() {
    let (interface, module) = TypeCheck::new().check(
        "load_0",
        r#"
def foo(x: list[bool]) -> str:
    return "test"
   "#,
    );
    TypeCheck::new()
        .load("foo.bzl", interface, module)
        .ty("res")
        .check(
            "load_1",
            r#"
load("foo.bzl", "foo")
def test():
    res = [foo([])]
"#,
        );
}

/// Test things that have previous claimed incorrectly they were type errors
#[test]
fn test_false_negative() {
    TypeCheck::new().check(
        "false_negative",
        r#"
def test():
    fail("Expected variable expansion in string: `{}`".format("x"))
"#,
    );
}

#[test]
fn test_dot_type() {
    TypeCheck::new().check(
        "dot_type_0",
        r#"
def foo(x: list) -> bool:
    return type(x) == type(list)

def bar():
    foo([1,2,3])
"#,
    );
    TypeCheck::new().check(
        "dot_type_1",
        r#"
def foo(x: list) -> bool:
    return type(x) == []

def bar():
    foo(True)
"#,
    );
}

#[test]
fn test_accepts_iterable() {
    TypeCheck::new().check(
        "accepts_iterable",
        r#"
def test():
    accepts_iterable([1, ()])
"#,
    );

    let mut a = Assert::new();
    a.globals_add(register_typecheck_globals);
    a.pass("accepts_iterable([1, ()])");
}

#[test]
fn test_dict_bug() {
    // TODO(nga): figure out how to fix it.
    //   Type of `y` should be inferred to `str`.
    TypeCheck::new().ty("y").check(
        "dict_bug",
        r#"
def test():
    x = {}
    x.setdefault(33, "x")
    y = x[44]
"#,
    );
}

#[test]
fn test_dict_lookup_by_never() {
    TypeCheck::new().check(
        "dict_never_key",
        r#"
# We use `typing.Never` when expression is an error,
# or it is a type parameter of empty list for example.
# Dict lookup by never should not be an error.
def test(d: dict[typing.Any, str], x: typing.Never):
    y = d[x]
"#,
    );
}

#[test]
fn test_new_list_dict_syntax() {
    TypeCheck::new().ty("x").check(
        "new_list_dict_syntax",
        r#"
def new_list_dict_syntax(d: dict[str, int]) -> list[str]:
    return list(d.keys())

def test():
    # Check type is properly parsed from the function return type.
    x = new_list_dict_syntax({"a": 1, "b": 2})
"#,
    );
}

#[test]
fn test_new_list_dict_syntax_as_value() {
    // TODO(nga): fix.
    TypeCheck::new().ty("x").ty("y").check(
        "new_list_dict_syntax_as_value",
        r#"
def test():
    x = list[str]
    y = dict[int, bool]
"#,
    );
}

#[test]
fn test_int_plus_float() {
    TypeCheck::new().ty("x").check(
        "int_plus_float",
        r#"
def test():
    x = 1 + 1.0
"#,
    );
}

#[test]
fn test_int_bitor_float() {
    TypeCheck::new().ty("x").check(
        "int_bitor_float",
        r#"
def test():
    x = 0x60000000000000000000000 | 1.0
"#,
    );
}

#[test]
fn test_un_op() {
    TypeCheck::new().ty("x").ty("y").ty("z").check(
        "un_op",
        r#"
def test():
    # Good.
    x = -1
    # Bad.
    y = ~True
    # Union good and bad.
    z = -(1 if True else "")
"#,
    );
}

#[test]
fn test_union() {
    TypeCheck::new().check(
        "union",
        r#"
def func_which_returns_union(p) -> str | int:
    if p == 56:
        return "a"
    elif p == 57:
        return 1
    else:
        return []
"#,
    );
}

#[test]
fn test_methods_work_for_ty_starlark_value() {
    TypeCheck::new().ty("x").check(
        "methods_work_for_ty_starlark_value",
        r#"
def test(s: str):
    x = s.startswith("a")
"#,
    );
}

#[test]
fn test_bit_or_return_int() {
    TypeCheck::new().check(
        "bit_or_return_int",
        r#"
test = int | 3

def foo() -> test:
    pass
"#,
    );
}

#[test]
fn test_bit_or_return_list() {
    TypeCheck::new().check(
        "bit_or_return_list",
        r#"
test = int | list[3]

def foo() -> test:
    pass
"#,
    );
}

#[test]
fn test_bit_or_with_load() {
    let (interface, module) = TypeCheck::new().check(
        "test_bit_or_with_load_foo",
        r#"
def foo() -> str:
    return "test"
"#,
    );
    TypeCheck::new().load("foo.bzl", interface, module).check(
        "test_bit_or_with_load",
        r#"
load("foo.bzl", "foo")
test = int | foo()
def test() -> test:
    pass
"#,
    );
}
