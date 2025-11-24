/*
 * Copyright 2018 The Starlark in Rust Authors.
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

//! Test starlark-rust embedding.

use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use derive_more::Display;
use starlark_derive::starlark_module;
use starlark_derive::starlark_value;
use starlark_derive::Trace;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::assert;
use crate::assert::Assert;
use crate::collections::SmallMap;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::any::StarlarkAny;
use crate::values::exported_name::FrozenExportedName;
use crate::values::none::NoneType;
use crate::values::types::exported_name::ExportedName;
use crate::values::types::exported_name::MutableExportedName;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::NoSerialize;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;

#[test]
fn test_export_as() {
    use std::fmt;
    use std::fmt::Debug;
    use std::fmt::Display;

    use crate as starlark;
    use crate::any::ProvidesStaticType;
    use crate::values::AllocValue;
    use crate::values::Heap;
    use crate::values::StarlarkValue;
    use crate::values::Trace;
    use crate::values::Value;

    #[derive(Debug, Trace, ProvidesStaticType, NoSerialize, Allocative, Freeze)]
    #[allocative(skip)]
    struct Exporter<T: ExportedName> {
        named: T,
        value: i32,
    }

    impl<T: ExportedName> Display for Exporter<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}={}", self.named, self.value)
        }
    }

    #[starlark_value(type = "exporter")]
    impl<'v, T: ExportedName> StarlarkValue<'v> for Exporter<T>
    where
        Self: ProvidesStaticType<'v>,
    {
        type Canonical = Exporter<FrozenExportedName>;

        fn export_as(
            &self,
            variable_name: &str,
            _eval: &mut Evaluator<'v, '_, '_>,
        ) -> crate::Result<()> {
            self.named.try_export_as(variable_name);
            Ok(())
        }
    }

    impl AllocValue<'_> for Exporter<MutableExportedName> {
        fn alloc_value(self, heap: &Heap) -> Value {
            heap.alloc_complex(self)
        }
    }

    #[starlark_module]
    fn exporter(builder: &mut GlobalsBuilder) {
        fn exporter(value: i32) -> anyhow::Result<Exporter<MutableExportedName>> {
            Ok(Exporter {
                named: MutableExportedName::default(),
                value,
            })
        }
    }

    let mut a = Assert::new();
    a.globals_add(exporter);
    a.module(
        "a",
        "x = exporter(1); y = x; longer_name = exporter(2); arrayed = [exporter(3)]",
    );
    // could reasonably be x=1 or y=1 twice, since the order
    // of calls to export_as is not defined
    let opt1 = "(x=1, x=1, longer_name=2, <not_exported>=3)".to_owned();
    let opt2 = opt1.replace("x=", "y=");

    a.is_true(&format!(
        r#"
load('a', 'x', 'y', 'longer_name', 'arrayed')
v = str((x, y, longer_name, arrayed[0]))
v == '{}' or v == '{}'"#,
        opt1, opt2
    ))
}

#[test]
// Test that we can express something that loads symbols into the exported module
fn test_load_symbols() {
    #[starlark_module]
    fn module(builder: &mut GlobalsBuilder) {
        fn load_symbol<'v>(
            name: &str,
            value: Value<'v>,
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> starlark::Result<NoneType> {
            eval.set_module_variable_at_some_point(name, value)?;
            Ok(NoneType)
        }
    }

    let mut a = Assert::new();
    a.globals_add(module);
    a.module("a", "load_symbol('x', 6*7)");
    a.is_true("load('a', 'x'); x == 42")
}

#[test]
fn test_load_public_symbols_does_not_reexport() -> starlark::Result<()> {
    let mut a = Assert::new();

    let module_b = a.module("b", "x = 5");
    let module_a = Module::new();
    module_a.import_public_symbols(&module_b);
    a.module_add("a", module_a.freeze()?);
    // Trying to load a symbol transitively should fail.
    a.fail("load('a', 'x')", "Module symbol `x` is not exported");
    Ok(())
}

#[test]
// Test that we can express something that loads symbols into the exported module,
// but not using the very dubious `set_module_variable_at_some_point`.
fn test_load_symbols_extra() -> crate::Result<()> {
    #[starlark_module]
    fn module(builder: &mut GlobalsBuilder) {
        fn load_symbol<'v>(
            name: &str,
            value: Value<'v>,
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> anyhow::Result<NoneType> {
            let extra = eval
                .module()
                .extra_value()
                .unwrap()
                .downcast_ref::<Extra<'v>>()
                .unwrap();
            extra.0.lock().unwrap().insert(name.to_owned(), value);
            Ok(NoneType)
        }
    }

    #[derive(
        ProvidesStaticType,
        Default,
        Trace,
        Debug,
        derive_more::Display,
        NoSerialize,
        Allocative
    )]
    #[display("{:?}", self)]
    struct Extra<'v>(Arc<Mutex<SmallMap<String, Value<'v>>>>);

    #[starlark_value(type = "Extra")]
    impl<'v> StarlarkValue<'v> for Extra<'v> {}

    let modu = Module::new();
    let globals = GlobalsBuilder::extended().with(module).build();
    {
        let mut eval = Evaluator::new(&modu);
        modu.set_extra_value(eval.heap().alloc_complex_no_freeze(Extra::default()));
        eval.eval_module(
            AstModule::parse(
                "a",
                "load_symbol('x', 6*7)".to_owned(),
                &Dialect::AllOptionsInternal,
            )?,
            &globals,
        )?;
    }

    let extra = modu.extra_value().unwrap().downcast_ref::<Extra>().unwrap();
    for (name, value) in extra.0.lock().unwrap().iter() {
        modu.set(name, *value);
    }
    modu.set_extra_value(Value::new_none());
    let mut a = Assert::new();
    a.globals(globals);
    a.module_add("a", modu.freeze()?);
    a.is_true("load('a', 'x'); x == 42");
    Ok(())
}

#[test]
fn test_repr_str() {
    #[derive(ProvidesStaticType, Debug, Display)]
    #[display("{:?}", self)]
    struct Foo(Option<usize>);

    #[starlark_module]
    fn module(builder: &mut GlobalsBuilder) {
        fn mk_foo() -> anyhow::Result<StarlarkAny<Foo>> {
            Ok(StarlarkAny::new(Foo(Some(42))))
        }
    }

    let mut a = Assert::new();
    a.globals_add(module);
    a.pass("assert_eq(repr(mk_foo()), 'Foo(Some(42))')");
}

#[test]
fn test_eval_function() {
    let fun = assert::pass(
        r#"
def fun(a, *, y: str, x = 1) -> str:
    return str((a, y, x))
fun
"#,
    );
    let env = Module::new();
    let mut eval = Evaluator::new(&env);
    let hello = env.heap().alloc("hello");
    let v = eval
        .eval_function(fun.value(), &[Value::testing_new_int(8)], &[("y", hello)])
        .unwrap();
    assert_eq!(v.unpack_str(), Some("(8, \"hello\", 1)"))
}
