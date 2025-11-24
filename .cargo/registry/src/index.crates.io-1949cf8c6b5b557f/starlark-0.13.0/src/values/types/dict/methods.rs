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

//! Methods for the `dict` type.

use std::mem;

use either::Either;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::MethodsBuilder;
use crate::values::dict::DictMut;
use crate::values::dict::DictRef;
use crate::values::list::AllocList;
use crate::values::list::ListRef;
use crate::values::list::UnpackList;
use crate::values::none::NoneType;
use crate::values::typing::StarlarkIter;
use crate::values::Heap;
use crate::values::Value;
use crate::values::ValueOfUnchecked;

#[starlark_module]
pub(crate) fn dict_methods(registry: &mut MethodsBuilder) {
    /// [dict.clear](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict·clear
    /// ): clear a dictionary
    ///
    /// `D.clear()` removes all the entries of dictionary D and returns `None`.
    /// It fails if the dictionary is frozen or if there are active iterators.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = {"one": 1, "two": 2}
    /// x.clear()
    /// x == {}
    /// # "#);
    /// ```
    fn clear(this: Value) -> anyhow::Result<NoneType> {
        let mut this = DictMut::from_value(this)?;
        this.aref.clear();
        Ok(NoneType)
    }

    /// [dict.get](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict·get
    /// ): return an element from the dictionary.
    ///
    /// `D.get(key[, default])` returns the dictionary value corresponding to
    /// the given key. If the dictionary contains no such value, `get`
    /// returns `None`, or the value of the optional `default` parameter if
    /// present.
    ///
    /// `get` fails if `key` is unhashable.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = {"one": 1, "two": 2}
    /// # (
    /// x.get("one") == 1
    /// # and
    /// x.get("three") == None
    /// # and
    /// x.get("three", 0) == 0
    /// # )"#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn get<'v>(
        this: DictRef<'v>,
        #[starlark(require = pos)] key: Value<'v>,
        #[starlark(require = pos)] default: Option<Value<'v>>,
    ) -> starlark::Result<Value<'v>> {
        match this.get(key)? {
            None => Ok(default.unwrap_or_else(Value::new_none)),
            Some(x) => Ok(x),
        }
    }

    /// [dict.items](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict·items
    /// ): get list of (key, value) pairs.
    ///
    /// `D.items()` returns a new list of key/value pairs, one per element in
    /// dictionary D, in the same order as they would be returned by a `for`
    /// loop.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = {"one": 1, "two": 2}
    /// x.items() == [("one", 1), ("two", 2)]
    /// # "#);
    /// ```
    fn items<'v>(
        this: DictRef<'v>,
        heap: &'v Heap,
    ) -> anyhow::Result<ValueOfUnchecked<'v, UnpackList<(Value<'v>, Value<'v>)>>> {
        Ok(heap.alloc_typed_unchecked(AllocList(this.iter())).cast())
    }

    /// [dict.keys](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict·keys
    /// ): get the list of keys of the dictionary.
    ///
    /// `D.keys()` returns a new list containing the keys of dictionary D, in
    /// the same order as they would be returned by a `for` loop.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = {"one": 1, "two": 2}
    /// x.keys() == ["one", "two"]
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn keys<'v>(
        this: DictRef<'v>,
        heap: &'v Heap,
    ) -> anyhow::Result<ValueOfUnchecked<'v, &'v ListRef<'v>>> {
        Ok(ValueOfUnchecked::new(heap.alloc(AllocList(this.keys()))))
    }

    /// [dict.pop](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict·pop
    /// ): return an element and remove it from a dictionary.
    ///
    /// `D.pop(key[, default])` returns the value corresponding to the specified
    /// key, and removes it from the dictionary.  If the dictionary contains no
    /// such value, and the optional `default` parameter is present, `pop`
    /// returns that value; otherwise, it fails.
    ///
    /// `pop` fails if `key` is unhashable, or the dictionary is frozen or has
    /// active iterators.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = {"one": 1, "two": 2}
    /// # (
    /// x.pop("one") == 1
    /// # and
    /// x == {"two": 2}
    /// # and
    /// x.pop("three", 0) == 0
    /// # and
    /// x.pop("three", None) == None
    /// # )"#);
    /// ```
    ///
    /// Failure:
    ///
    /// ```
    /// # starlark::assert::fail(r#"
    /// {'one': 1}.pop('four')   # error: not found
    /// # "#, "not found");
    /// ```
    fn pop<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] key: Value<'v>,
        #[starlark(require = pos)] default: Option<Value<'v>>,
    ) -> starlark::Result<Value<'v>> {
        let mut me = DictMut::from_value(this)?;
        match me.aref.remove_hashed(key.get_hashed()?) {
            Some(x) => Ok(x),
            None => match default {
                Some(v) => Ok(v),
                None => {
                    mem::drop(me);
                    Err(anyhow::anyhow!(
                        "Key `{}` not found in dictionary `{}`",
                        key.to_repr(),
                        this.to_repr()
                    )
                    .into())
                }
            },
        }
    }

    /// [dict.popitem](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict·popitem
    /// ): returns and removes the first key/value pair of a dictionary.
    ///
    /// `D.popitem()` returns the first key/value pair, removing it from the
    /// dictionary.
    ///
    /// `popitem` fails if the dictionary is empty, frozen, or has active
    /// iterators.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = {"one": 1, "two": 2}
    /// # (
    /// x.popitem() == ("one", 1)
    /// # and
    /// x.popitem() == ("two", 2)
    /// # and
    /// x == {}
    /// # )"#);
    /// ```
    ///
    /// Failure:
    ///
    /// ```
    /// # starlark::assert::fail(r#"
    /// {}.popitem()   # error: empty dict
    /// # "#, "empty dict");
    /// ```
    fn popitem<'v>(this: Value<'v>) -> anyhow::Result<(Value<'v>, Value<'v>)> {
        let mut this = DictMut::from_value(this)?;

        // TODO(nga): this implementation is O(N).
        //   https://github.com/bazelbuild/starlark/issues/286

        match this.aref.content.shift_remove_index(0) {
            Some((k, v)) => Ok((k, v)),
            None => Err(anyhow::anyhow!("Cannot .popitem() on an empty dictionary")),
        }
    }

    /// [dict.setdefault](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict·setdefault
    /// ): get a value from a dictionary, setting it to a new value if not
    /// present.
    ///
    /// `D.setdefault(key[, default])` returns the dictionary value
    /// corresponding to the given key. If the dictionary contains no such
    /// value, `setdefault`, like `get`, returns `None` or the value of the
    /// optional `default` parameter if present; `setdefault` additionally
    /// inserts the new key/value entry into the dictionary.
    ///
    /// `setdefault` fails if the key is unhashable or if the dictionary is
    /// frozen.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = {"one": 1, "two": 2}
    /// # (
    /// x.setdefault("one") == 1
    /// # and
    /// x.setdefault("three", 0) == 0
    /// # and
    /// x == {"one": 1, "two": 2, "three": 0}
    /// # and
    /// x.setdefault("four") == None
    /// # and
    /// x == {"one": 1, "two": 2, "three": 0, "four": None}
    /// # )"#)
    /// ```
    fn setdefault<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] key: Value<'v>,
        #[starlark(require = pos)] default: Option<Value<'v>>,
    ) -> starlark::Result<Value<'v>> {
        let mut this = DictMut::from_value(this)?;
        let key = key.get_hashed()?;
        match this.aref.content.entry_hashed(key) {
            starlark_map::small_map::Entry::Occupied(e) => Ok(*e.get()),
            starlark_map::small_map::Entry::Vacant(e) => {
                let default = default.unwrap_or_else(Value::new_none);
                e.insert(default);
                Ok(default)
            }
        }
    }

    /// [dict.update](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict·update
    /// ): update values in the dictionary.
    ///
    /// `D.update([pairs][, name=value[, ...])` makes a sequence of key/value
    /// insertions into dictionary D, then returns `None.`
    ///
    /// If the positional argument `pairs` is present, it must be `None`,
    /// another `dict`, or some other iterable.
    /// If it is another `dict`, then its key/value pairs are inserted into D.
    /// If it is an iterable, it must provide a sequence of pairs (or other
    /// iterables of length 2), each of which is treated as a key/value pair
    /// to be inserted into D.
    ///
    /// For each `name=value` argument present, the name is converted to a
    /// string and used as the key for an insertion into D, with its
    /// corresponding value being `value`.
    ///
    /// `update` fails if the dictionary is frozen.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = {}
    /// x.update([("a", 1), ("b", 2)], c=3)
    /// x.update({"d": 4})
    /// x.update(e=5)
    /// x == {"a": 1, "b": 2, "c": 3, "d": 4, "e": 5}
    /// # "#);
    /// ```
    fn update<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] pairs: Option<
            ValueOfUnchecked<'v, Either<DictRef<'v>, StarlarkIter<(Value<'v>, Value<'v>)>>>,
        >,
        #[starlark(kwargs)] kwargs: DictRef<'v>,
        heap: &'v Heap,
    ) -> starlark::Result<NoneType> {
        let pairs = if pairs.map(|x| x.get().ptr_eq(this)) == Some(true) {
            // someone has done `x.update(x)` - that isn't illegal, but we will have issues
            // with trying to iterate over x while holding x for mutation, and it doesn't do
            // anything useful, so just change pairs back to None
            None
        } else {
            pairs.map(|x| x.get())
        };

        let mut this = DictMut::from_value(this)?;
        if let Some(pairs) = pairs {
            if let Some(dict) = DictRef::from_value(pairs) {
                for (k, v) in dict.iter_hashed() {
                    this.aref.insert_hashed(k, v);
                }
            } else {
                for v in pairs.iterate(heap)? {
                    let mut it = v.iterate(heap)?;
                    // `StarlarkIterator` is fused.
                    let (Some(k), Some(v), None) = (it.next(), it.next(), it.next()) else {
                        return Err(anyhow::anyhow!(
                            "dict.update expect a list of pairs or a dictionary as first argument, got a list of non-pairs.",
                        ).into());
                    };
                    this.aref.insert_hashed(k.get_hashed()?, v);
                }
            }
        }

        for (k, v) in kwargs.iter_hashed() {
            this.aref.insert_hashed(k, v);
        }
        Ok(NoneType)
    }

    /// [dict.values](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict·values
    /// ): get the list of values of the dictionary.
    ///
    /// `D.values()` returns a new list containing the dictionary's values, in
    /// the same order as they would be returned by a `for` loop over the
    /// dictionary.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = {"one": 1, "two": 2}
    /// x.values() == [1, 2]
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn values<'v>(
        this: DictRef<'v>,
        heap: &'v Heap,
    ) -> anyhow::Result<ValueOfUnchecked<'v, &'v ListRef<'v>>> {
        Ok(ValueOfUnchecked::new(heap.alloc_list_iter(this.values())))
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;
    use crate::assert::Assert;

    #[test]
    fn test_error_codes() {
        assert::fail(r#"x = {"one": 1}; x.pop("four")"#, "not found");
        assert::fail("x = {}; x.popitem()", "empty");
    }

    #[test]
    fn test_dict_add() {
        assert::fail("{1: 2} + {3: 4}", "not supported");
    }

    #[test]
    fn test_dict_with_duplicates() {
        // In Starlark spec this is a runtime error. In Python it's fine.
        // We make it a runtime error, plus have a lint that checks for it statically.
        assert::fails("{40+2: 2, 6*7: 3}", &["key repeated", "42"]);
        // Also check we fail if the entire dictionary is static (a different code path).
        assert::fails("{42: 2, 42: 3}", &["key repeated", "42"]);
    }

    #[test]
    fn test_dict_update_with_self_pos() {
        assert::eq("{3: 4, 1: 2}", "d = {3: 4, 1: 2}; d.update(d); d");
    }

    #[test]
    fn test_dict_update_with_self_as_kwargs() {
        assert::eq("{'a': 1, 'b': 2}", "d = {'a': 1, 'b': 2}; d.update(**d); d");
    }

    #[test]
    fn test_frozen_dict_cannot_be_updated_with_self_pos() {
        let mut a = Assert::new();
        a.module("d.star", "D = {7: 8, 9: 0}");
        a.fail(
            r#"
load('d.star', 'D')

D.update(D)
"#,
            "Immutable",
        );
    }

    #[test]
    fn test_frozen_dict_cannot_be_updated_with_self_as_kwargs() {
        let mut a = Assert::new();
        a.module("d.star", "D = {'x': 17, 'y': 19}");
        a.fail(
            r#"
load('d.star', 'D')
D.update(**D)
"#,
            "Immutable",
        );
    }
}
