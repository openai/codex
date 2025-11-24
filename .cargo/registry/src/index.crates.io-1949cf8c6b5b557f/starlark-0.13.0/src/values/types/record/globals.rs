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

//! Implementation of `record` function.

use dupe::Dupe;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::collections::SmallMap;
use crate::environment::GlobalsBuilder;
use crate::eval::Evaluator;
use crate::values::record::field::Field;
use crate::values::record::record_type::RecordType;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::Value;

#[starlark_module]
pub(crate) fn register_record(builder: &mut GlobalsBuilder) {
    /// A `record` type represents a set of named values, each with their own type.
    ///
    /// For example:
    ///
    /// ```python
    /// MyRecord = record(host=str, port=int)
    /// ```
    ///
    /// This above statement defines a record `MyRecord` with 2 fields, the first named `host` that must be of type `str`, and the second named `port` that must be of type `int`.
    ///
    /// Now `MyRecord` is defined, it's possible to do the following:
    ///
    /// * Create values of this type with `MyRecord(host="localhost", port=80)`. It is a runtime error if any arguments are missed, of the wrong type, or if any unexpected arguments are given.
    /// * Get the type of the record suitable for a type annotation with `MyRecord.type`.
    /// * Get the fields of the record. For example, `v = MyRecord(host="localhost", port=80)` will provide `v.host == "localhost"` and `v.port == 80`. Similarly, `dir(v) == ["host", "port"]`.
    ///
    /// It is also possible to specify default values for parameters using the `field` function.
    ///
    /// For example:
    ///
    /// ```python
    /// MyRecord = record(host=str, port=field(int, 80))
    /// ```
    ///
    /// Now the `port` field can be omitted, defaulting to `80` is not present (for example, `MyRecord(host="localhost").port == 80`).
    ///
    /// Records are stored deduplicating their field names, making them more memory efficient than dictionaries.
    fn record<'v>(
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<RecordType<'v>> {
        // Every Value must either be a field or a value (the type)
        let mut mp = SmallMap::with_capacity(kwargs.len());
        for (k, v) in kwargs.into_iter_hashed() {
            let field = match Field::from_value(v) {
                None => Field::new(TypeCompiled::new(v, eval.heap())?, None),
                Some(v) => v.dupe(),
            };
            mp.insert_hashed(k, field);
        }
        Ok(RecordType::new(mp))
    }

    /// Creates a field record. Used as an argument to the `record` function.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// rec_type = record(host=field(str), port=field(int), mask=field(int, default=255))
    /// rec = rec_type(host="localhost", port=80)
    /// rec.port == 80
    /// rec.mask == 255
    /// # "#);
    /// ```
    fn field<'v>(
        #[starlark(require = pos)] typ: Value<'v>,
        default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Field<'v>> {
        // We compile the type even if we don't have a default to raise the error sooner
        let compiled = TypeCompiled::new(typ, eval.heap())?;
        if let Some(d) = default {
            compiled.check_type(d, Some("default"))?;
        }
        Ok(Field::new(compiled, default))
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;
    use crate::assert::Assert;

    #[test]
    fn test_record_pass() {
        assert::pass(
            r#"
rec_type = record(host=str, port=int)
rec1 = rec_type(host = "test", port=80)
rec2 = rec_type(host = "test", port=90)
assert_eq(rec1, rec1)
assert_eq(rec1 == rec2, False)
assert_eq(rec1.host, "test")
assert_eq(rec1.port, 80)
assert_eq(dir(rec1), ["host", "port"])
"#,
        );
    }

    #[test]
    fn test_record_fail_0() {
        assert::fails(
            r#"
rec_type = record(host=str, port=int)
rec_type(host=1, port=80)
"#,
            &[
                "Value `1` of type `int` does not match the type annotation `str` for argument `host`",
            ],
        );
    }

    #[test]
    fn test_record_fail_1() {
        assert::fails(
            r#"
rec_type = record(host=str, port=int)
rec_type(port=80)
"#,
            &["Missing named-only parameter", "`host`"],
        );
    }

    #[test]
    fn test_record_fail_2() {
        assert::fails(
            r#"
rec_type = record(host=str, port=int)
rec_type(host="localhost", port=80, mask=255)
"#,
            &["extra named", "mask"],
        );
    }

    #[test]
    fn test_record_fail_3() {
        assert::pass(
            r#"
rec_type = record(host=str, port=int)
def foo(x: rec_type) -> rec_type:
    return x
foo(rec_type(host="localhost", port=80))"#,
        );
    }

    #[test]
    fn test_record_fail_4() {
        assert::pass(
            r#"
v = [record(host=str, port=int)]
v_0 = v[0]
def foo(y: v_0) -> v_0:
    # TODO(nga): fails at compile time.
    return noop(y)
foo(v[0](host="localhost", port=80))"#,
        );
    }

    #[test]
    fn test_record_fail_5() {
        assert::pass(
            r#"
rec_type = record(host=str, port=field(int, 80), mask=int)
assert_eq(rec_type(host="localhost", mask=255), rec_type(host="localhost", port=80, mask=255))"#,
        );
        // Make sure the default value is heap allocated (used to fail with a GC issue)
        assert::pass(
            r#"
heap_string = "test{}".format(42)
rec_type = record(test_gc=field(str, heap_string))
assert_eq(rec_type().test_gc, "test42")"#,
        );
    }

    #[test]
    fn test_record_equality() {
        assert::pass(
            r#"
rec_type = record(host=str, port=field(int, 80))
assert_eq(rec_type(host="s"), rec_type(host="s"))
assert_eq(rec_type(host="s"), rec_type(host="s", port=80))
assert_ne(rec_type(host="s"), rec_type(host="t"))
"#,
        );

        let mut a = Assert::new();
        a.module(
            "m",
            r#"
rec_type = record(host=str, port=field(int, 80))
rec_val = rec_type(host="s")
"#,
        );
        a.pass(
            r#"
load('m', 'rec_type', 'rec_val')
assert_eq(rec_val, rec_type(host="s"))
assert_ne(rec_val, rec_type(host="t"))
"#,
        );

        a = Assert::new();
        a.module(
            "m",
            r#"
rt = record(host=str)
"#,
        );
        a.pass(
            r#"
load('m', r1='rt')
rt = record(host=str)
diff = record(host=str)
assert_ne(r1(host="test"), rt(host="test"))
assert_ne(r1(host="test"), diff(host="test"))
"#,
        );
    }

    #[test]
    fn test_field_invalid() {
        assert::fails(
            "field(str, None)",
            &["does not match the type", "`default`"],
        );
        assert::fails("field(True)", &["`True`", "not a valid type"]);
    }
}
