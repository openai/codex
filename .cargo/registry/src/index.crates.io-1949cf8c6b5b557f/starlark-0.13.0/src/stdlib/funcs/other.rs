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

//! A module with the standard function and constants that are by default in all
//! dialect of Starlark

use std::cmp::Ordering;

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::eval::Evaluator;
use crate::values::list::AllocList;
use crate::values::tuple::UnpackTuple;
use crate::values::typing::never::StarlarkNever;
use crate::values::typing::ty::AbstractType;
use crate::values::typing::StarlarkIter;
use crate::values::value_of_unchecked::ValueOfUnchecked;
use crate::values::AllocValue;
use crate::values::FrozenStringValue;
use crate::values::Heap;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLike;

#[starlark_module]
pub(crate) fn register_other(builder: &mut GlobalsBuilder) {
    /// fail: fail the execution
    ///
    /// ```
    /// # starlark::assert::fail(r#"
    /// fail("this is an error")  # fail: this is an error
    /// # "#, "this is an error");
    /// # starlark::assert::fail(r#"
    /// fail("oops", 1, False)  # fail: oops 1 False
    /// # "#, "oops 1 False");
    /// ```
    fn fail(#[starlark(args)] args: UnpackTuple<Value>) -> starlark::Result<StarlarkNever> {
        let mut s = String::new();
        for x in args.items {
            s.push(' ');
            match x.unpack_str() {
                Some(x) => s.push_str(x),
                None => x.collect_repr(&mut s),
            }
        }
        Err(starlark::Error::new_kind(starlark::ErrorKind::Fail(
            anyhow::Error::msg(s),
        )))
    }

    /// [any](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#any
    /// ): returns true if any value in the iterable object have a truth value
    /// of true.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// any([0, True]) == True
    /// any([0, 1]) == True
    /// any([0, 1, True]) == True
    /// any([0, 0]) == False
    /// any([0, False]) == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn any<'v>(
        #[starlark(require = pos)] x: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<bool> {
        for i in x.get().iterate(heap)? {
            if i.to_bool() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// [all](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#all
    /// ): returns true if all values in the iterable object have a truth value
    /// of true.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// all([1, True]) == True
    /// all([1, 1]) == True
    /// all([0, 1, True]) == False
    /// all([True, 1, True]) == True
    /// all([0, 0]) == False
    /// all([0, False]) == False
    /// all([True, 0]) == False
    /// all([1, False]) == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn all<'v>(
        #[starlark(require = pos)] x: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<bool> {
        for i in x.get().iterate(heap)? {
            if !i.to_bool() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// [dir](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dir
    /// ): list attributes of a value.
    ///
    /// `dir(x)` returns a list of the names of the attributes (fields and
    /// methods) of its operand. The attributes of a value `x` are the names
    /// `f` such that `x.f` is a valid expression.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "capitalize" in dir("abc")
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn dir(#[starlark(require = pos)] x: Value) -> anyhow::Result<Vec<String>> {
        Ok(x.dir_attr())
    }

    /// [enumerate](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#enumerate
    /// ): return a list of (index, element) from an iterable.
    ///
    /// `enumerate(x)` returns a list of `(index, value)` pairs, each containing
    /// successive values of the iterable sequence and the index of the
    /// value within the sequence.
    ///
    /// The optional second parameter, `start`, specifies an integer value to
    /// add to each index.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// enumerate(["zero", "one", "two"]) == [(0, "zero"), (1, "one"), (2, "two")]
    /// enumerate(["one", "two"], 1) == [(1, "one"), (2, "two")]
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn enumerate<'v>(
        #[starlark(require = pos)] it: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        #[starlark(default = 0)] start: i32,
        heap: &'v Heap,
    ) -> starlark::Result<impl AllocValue<'v>> {
        let v = it
            .get()
            .iterate(heap)?
            .enumerate()
            .map(move |(k, v)| (k as i32 + start, v));
        Ok(AllocList(v))
    }

    /// [getattr](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#getattr
    /// ): returns the value of an attribute
    ///
    /// `getattr(x, name)` returns the value of the attribute (field or method)
    /// of x named `name`. It is a dynamic error if x has no such attribute.
    ///
    /// `getattr(x, "f")` is equivalent to `x.f`.
    ///
    /// `getattr(x, "f", d)` is equivalent to `x.f if hasattr(x, "f") else d`
    /// and will never raise an error.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// getattr("banana", "split")("a") == ["b", "n", "n", ""] # equivalent to "banana".split("a")
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn getattr<'v>(
        #[starlark(require = pos)] a: Value<'v>,
        #[starlark(require = pos)] attr: &str,
        #[starlark(require = pos)] default: Option<Value<'v>>,
        heap: &'v Heap,
    ) -> starlark::Result<Value<'v>> {
        // TODO(nga): this doesn't cache string hash, so it is suboptimal.
        match a.get_attr(attr, heap)? {
            Some(v) => Ok(v),
            None => match default {
                Some(x) => Ok(x),
                None => ValueError::unsupported_owned(a.get_type(), &format!(".{}", attr), None),
            },
        }
    }

    /// [hasattr](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#hasattr
    /// ): test if an object has an attribute
    ///
    /// `hasattr(x, name)` reports whether x has an attribute (field or method)
    /// named `name`.
    #[starlark(speculative_exec_safe)]
    fn hasattr<'v>(
        #[starlark(require = pos)] a: Value<'v>,
        #[starlark(require = pos)] attr: &str,
        heap: &'v Heap,
    ) -> anyhow::Result<bool> {
        Ok(a.has_attr(attr, heap))
    }

    /// [hash](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#hash
    /// ): returns the hash number of a value.
    ///
    /// `hash(x)` returns an integer hash value for x such that `x == y`
    /// implies `hash(x) == hash(y)`.
    ///
    /// `hash` fails if x, or any value upon which its hash depends, is
    /// unhashable.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// hash("hello") != hash("world")
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn hash(#[starlark(require = pos)] a: &str) -> anyhow::Result<i32> {
        // From the starlark spec:
        // > the hash function for strings is the same as that implemented by java.lang.String.hashCode,
        // > a simple polynomial accumulator over the UTF-16 transcoding of the string:
        // > `s[0]*31^(n-1) + s[1]*31^(n-2) + ... + s[n-1]`
        // As per spec the function should only support string and bytes types.
        // We don't have support for bytes, so parameter is forced to be a string.

        // Most strings are ASCII strings, try them first.
        #[allow(clippy::never_loop)]
        'ascii: loop {
            let mut hash = 0i32;
            for &b in a.as_bytes() {
                if b > 0x7f {
                    break 'ascii;
                }
                hash = hash.wrapping_mul(31i32).wrapping_add(b as i32);
            }
            return Ok(hash);
        }

        Ok(a.encode_utf16().fold(0i32, |hash: i32, c: u16| {
            31i32.wrapping_mul(hash).wrapping_add(c as i32)
        }))
    }

    /// [len](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#len
    /// ): get the length of a sequence
    ///
    /// `len(x)` returns the number of elements in its argument.
    ///
    /// It is a dynamic error if its argument is not a sequence.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// len(()) == 0
    /// len({}) == 0
    /// len([]) == 0
    /// len([1]) == 1
    /// len([1,2]) == 2
    /// len({'16': 10}) == 1
    /// # "#);
    /// # starlark::assert::fail(r#"
    /// len(True)    # error: not supported
    /// # "#, "not supported");
    /// ```
    #[starlark(speculative_exec_safe)]
    fn len(#[starlark(require = pos)] a: Value) -> starlark::Result<i32> {
        a.length()
    }

    /// [reversed](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#reversed
    /// ): reverse a sequence
    ///
    /// `reversed(x)` returns a new list containing the elements of the iterable
    /// sequence x in reverse order.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// reversed(['a', 'b', 'c'])              == ['c', 'b', 'a']
    /// reversed(range(5))                     == [4, 3, 2, 1, 0]
    /// reversed("stressed".elems())           == ["d", "e", "s", "s", "e", "r", "t", "s"]
    /// reversed({"one": 1, "two": 2}.keys())  == ["two", "one"]
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn reversed<'v>(
        #[starlark(require = pos)] a: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<Vec<Value<'v>>> {
        let mut v: Vec<Value> = a.get().iterate(heap)?.collect();
        v.reverse();
        Ok(v)
    }

    /// [sorted](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#sorted
    /// ): sort a sequence
    ///
    /// `sorted(x)` returns a new list containing the elements of the iterable
    /// sequence x, in sorted order.  The sort algorithm is stable.
    ///
    /// The optional named parameter `reverse`, if true, causes `sorted` to
    /// return results in reverse sorted order.
    ///
    /// The optional named parameter `key` specifies a function of one
    /// argument to apply to obtain the value's sort key.
    /// The default behavior is the identity function.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// sorted([3, 1, 4, 1, 5, 9])                               == [1, 1, 3, 4, 5, 9]
    /// sorted([3, 1, 4, 1, 5, 9], reverse=True)                 == [9, 5, 4, 3, 1, 1]
    /// sorted(["two", "three", "four"], key=len)                == ["two", "four", "three"] # shortest to longest
    /// sorted(["two", "three", "four"], key=len, reverse=True)  == ["three", "four", "two"] # longest to shortest
    /// # "#);
    /// ```
    // This function is not spec-safe, because it may call `key` function
    // which might be not spec-safe.
    fn sorted<'v>(
        #[starlark(require = pos)] x: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        #[starlark(require = named)] key: Option<Value<'v>>,
        #[starlark(require = named, default = false)] reverse: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<AllocList<impl IntoIterator<Item = Value<'v>>>> {
        let it = x.get().iterate(eval.heap())?;
        let mut it = match key {
            None => it.map(|x| (x, x)).collect(),
            Some(key) => {
                let mut v = Vec::new();
                for el in it {
                    v.push((el, key.invoke_pos(&[el], eval)?));
                }
                v
            }
        };

        let mut compare_ok = Ok(());

        it.sort_by(|x: &(Value, Value), y: &(Value, Value)| {
            let ord_or_err = if reverse {
                x.1.compare(y.1).map(Ordering::reverse)
            } else {
                x.1.compare(y.1)
            };
            match ord_or_err {
                Ok(r) => r,
                Err(e) => {
                    compare_ok = Err(e);
                    Ordering::Equal // does not matter
                }
            }
        });

        compare_ok?;

        Ok(AllocList(it.into_iter().map(|x| x.0)))
    }

    /// [type](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#type
    /// ): returns a string describing the type of its operand.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// type(None)              == "NoneType"
    /// type(0)                 == "int"
    /// type(1)                 == "int"
    /// type(())                == "tuple"
    /// type("hello")           == "string"
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe, as_type = AbstractType)]
    fn r#type<'v>(#[starlark(require = pos)] a: Value) -> anyhow::Result<FrozenStringValue> {
        Ok(a.get_type_value())
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;
    use crate::assert::Assert;

    #[test]
    fn test_abs() {
        assert::eq("1", "abs(1)");
        assert::eq("1", "abs(-1)");
        assert::eq("2147483647", "abs(2147483647)");
        assert::eq("2147483648", "abs(-2147483648)");
        assert::eq("2147483648000", "abs(2147483648000)");
        assert::eq("2147483648000", "abs(-2147483648000)");
        assert::eq("1.23", "abs(-1.23)");
        assert::eq("2.3", "abs(2.3)");
        assert::is_true("isinstance(abs(1), int)");
    }

    #[test]
    fn test_constants() {
        assert::is_true("not None");
        assert::is_true("not False");
        assert::is_true("True");
    }

    #[test]
    fn test_chr() {
        assert::fail("chr(0x110000)", "not a valid UTF-8");
        assert::fail("chr(-1)", "negative");
    }

    #[test]
    fn test_hash() {
        assert::eq("0", "hash('')");
        assert::eq("97", "hash('a')");
        assert::eq("3105", "hash('ab')");
        assert::eq("96354", "hash('abc')");
        assert::eq("2987074", "hash('abcd')");
        assert::eq("92599395", "hash('abcde')");
        assert::eq("-1424385949", "hash('abcdef')");
        assert::all_true(
            r#"
hash("te") == hash("te")
hash("te") != hash("st")
x = "test"; y = "te" + "st"; hash(y) == hash(y)
"#,
        );
        assert::fail("noop(hash)(None)", "doesn't match");
        assert::fail("noop(hash)(True)", "doesn't match");
        assert::fail("noop(hash)(1)", "doesn't match");
        assert::fail("noop(hash)([])", "doesn't match");
        assert::fail("noop(hash)({})", "doesn't match");
        assert::fail("noop(hash)(range(1))", "doesn't match");
        assert::fail("noop(hash)((1, 2))", "doesn't match");
        assert::fail(
            r#"
def foo():
    pass
noop(hash)(foo)
"#,
            "doesn't match",
        );
    }

    #[test]
    fn test_int() {
        assert::eq("2147483647", "int('2147483647')");
        assert::eq("-2147483647 - 1", "int('-2147483648')");
        assert::eq("0", "int('0')");
        assert::eq("0", "int('-0')");
        assert::eq(
            "999999999999999945322333868247445125709646570021247924665841614848",
            "int(1e66)",
        );
        assert::eq("2147483648", "int('2147483648')");
        assert::eq("-2147483649", "int('-2147483649')");
    }

    #[test]
    fn test_tuple() {
        let mut a = Assert::new();
        // TODO(nga): fix and enable.
        a.disable_static_typechecking();
        a.eq("(1, 2)", "tuple((1, 2))");
        a.eq("(1, 2)", "tuple([1, 2])");
    }
}
