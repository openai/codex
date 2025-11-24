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

//! Implementation of `enum` function.
use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::values::enumeration::EnumType;
use crate::values::tuple::UnpackTuple;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::Value;

#[starlark_module]
pub fn register_enum(builder: &mut GlobalsBuilder) {
    /// The `enum` type represents one value picked from a set of values.
    ///
    /// For example:
    ///
    /// ```python
    /// MyEnum = enum("option1", "option2", "option3")
    /// ```
    ///
    /// This statement defines an enumeration `MyEnum` that consists of the three values `"option1"`, `"option2"` and `option3`.
    ///
    /// Now `MyEnum` is defined, it's possible to do the following:
    ///
    /// * Create values of this type with `MyEnum("option2")`. It is a runtime error if the argument is not one of the predeclared values of the enumeration.
    /// * Get the type of the enum suitable for a type annotation with `MyEnum`.
    /// * Given a value of the enum (for example, `v = MyEnum("option2")`), get the underlying value `v.value == "option2"` or the index in the enumeration `v.index == 1`.
    /// * Get a list of the values that make up the array with `MyEnum.values() == ["option1", "option2", "option3"]`.
    /// * Treat `MyEnum` a bit like an array, with `len(MyEnum) == 3`, `MyEnum[1] == MyEnum("option2")` and iteration over enums `[x.value for x in MyEnum] == ["option1", "option2", "option3"]`.
    ///
    /// Enumeration types store each value once, which are then efficiently referenced by enumeration values.
    fn r#enum<'v>(
        #[starlark(args)] args: UnpackTuple<StringValue<'v>>,
        heap: &'v Heap,
    ) -> starlark::Result<Value<'v>> {
        // Every Value must either be a field or a value (the type)
        EnumType::new(args.items, heap)
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;
    use crate::assert::Assert;

    #[test]
    fn test_enum() {
        assert::pass(
            r#"
enum_type = enum("option1", "option2", "option3")
x = enum_type("option1")
assert_eq(x.value, "option1")
assert_eq(enum_type("option3").value, "option3")
assert_eq(enum_type.type, "enum_type")
"#,
        );
        assert::fails(
            r#"
enum_type = enum("option1", "option2", "option3")
enum_type(False)"#,
            &["Unknown enum element", "`False`", "option1"],
        );
        assert::fails(
            r#"
enum_type = enum("option1", "option2", "option3")
enum_type("option4")"#,
            &["Unknown enum element", "`option4`"],
        );
        assert::fails(
            r#"
enum_type = enum("option1", "option1")
enum_type("option3")"#,
            &["distinct", "option1"],
        );
        assert::pass(
            r#"
enum_type = enum("option1","option2")
def foo(x: enum_type) -> enum_type:
    return x
foo(enum_type("option1"))"#,
        );
        assert::pass(
            r#"
v = [enum("option1","option2")]
v_0 = v[0]
def foo(y: v_0) -> v_0:
    # TODO(nga): fails at compile time.
    return noop(y)
foo(v[0]("option1"))"#,
        );
        assert::pass(
            r#"
enum_type = enum("option1","option2")
assert_eq(enum_type.values(), ["option1","option2"])
assert_eq([enum_type[i].value for i in range(len(enum_type))], ["option1","option2"])
assert_eq(enum_type("option2").index, 1)
assert_eq([x.value for x in enum_type], ["option1","option2"])"#,
        );
        assert::pass(
            r#"
enum_type = enum("option1","option2")
x = enum_type("option1")
assert_eq(str(enum_type), "enum(\"option1\", \"option2\")")
assert_eq(str(x), "enum_type(\"option1\")")
"#,
        );
        assert::pass(
            r#"
enum_type = enum("option1","option2")
repr(enum_type) # Check it is finite
"#,
        );
    }

    #[test]
    fn test_enum_equality() {
        assert::pass(
            r#"
enum_type = enum("option1", "option2", "option3")
assert_eq(enum_type("option1"), enum_type("option1"))
assert_eq(enum_type("option3"), enum_type("option3"))
assert_ne(enum_type("option1"), enum_type("option3"))
"#,
        );

        let mut a = Assert::new();
        a.module(
            "m",
            r#"
enum_type = enum("option1", "option2", "option3")
enum_val = enum_type("option1")
"#,
        );
        a.pass(
            r#"
load('m', 'enum_type', 'enum_val')
assert_eq(enum_val, enum_type("option1"))
assert_ne(enum_val, enum_type("option3"))
"#,
        );

        a = Assert::new();
        a.module(
            "m1",
            r#"
rt = enum("one")
"#,
        );
        a.module(
            "m2",
            r#"
rt = enum("one", "two")
"#,
        );
        a.pass(
            r#"
load('m1', r1='rt')
load('m2', r2='rt')
rt = enum("one")
diff = enum("one")
assert_ne(r1("one"), rt("one"))
assert_ne(rt("one"), r2("one"))
assert_ne(rt("one"), diff("one"))
"#,
        );
    }

    #[test]
    fn test_enum_repr() {
        assert::pass(
            r#"
enum_type = enum("option1", "option2")
assert_eq("enum_type(\"option1\")", repr(enum_type("option1")))
assert_eq("enum()(\"option1\")", repr(enum("option1", "option2")("option1")))
"#,
        );
    }
}
