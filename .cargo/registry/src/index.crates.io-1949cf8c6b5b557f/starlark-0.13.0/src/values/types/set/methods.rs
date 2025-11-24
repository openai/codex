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

//! Methods for the `set` type.

use std::mem;

use either::Either;
use starlark_derive::starlark_module;
use starlark_map::small_set::SmallSet;
use starlark_map::Hashed;
use starlark_syntax::value_error;

use crate as starlark;
use crate::environment::MethodsBuilder;
use crate::values::none::NoneType;
use crate::values::set::refs::SetMut;
use crate::values::set::refs::SetRef;
use crate::values::set::value::SetData;
use crate::values::typing::StarlarkIter;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueOfUnchecked;

enum SetFromValue<'v> {
    Set(SmallSet<Value<'v>>),
    Ref(SetRef<'v>),
}

impl<'v> SetFromValue<'v> {
    fn from_value(
        value: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> crate::Result<Self> {
        if let Some(v) = SetRef::unpack_value_opt(value.get()) {
            Ok(SetFromValue::Ref(v))
        } else {
            let mut set = SmallSet::default();
            for elem in value.get().iterate(heap)? {
                set.insert_hashed(elem.get_hashed()?);
            }
            Ok(SetFromValue::Set(set))
        }
    }

    fn get(&self) -> &SmallSet<Value<'v>> {
        match self {
            SetFromValue::Set(set) => set,
            SetFromValue::Ref(set) => &set.aref.content,
        }
    }

    fn into_set(self) -> SmallSet<Value<'v>> {
        match self {
            SetFromValue::Set(set) => set,
            SetFromValue::Ref(set) => set.aref.content.clone(),
        }
    }

    fn is_empty(&self) -> bool {
        self.get().is_empty()
    }

    fn contains_hashed(&self, value: Hashed<Value<'v>>) -> bool {
        self.get().contains_hashed(value.as_ref())
    }
}

#[starlark_module]
pub(crate) fn set_methods(builder: &mut MethodsBuilder) {
    fn clear(this: Value) -> anyhow::Result<NoneType> {
        let mut this = SetMut::from_value(this)?;
        this.aref.clear();
        Ok(NoneType)
    }

    /// Return a new set with elements from the set and all others.
    /// Unlike Python does not support variable number of arguments.
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// y = [3, 4, 5]
    /// x.union(y) == set([1, 2, 3, 4, 5])
    /// # "#);
    /// ```
    fn union<'v>(
        this: SetRef<'v>,
        #[starlark(require=pos)] other: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<SetData<'v>> {
        if this.aref.content.is_empty() {
            let other_set = SetFromValue::from_value(other, heap)?;
            return Ok(SetData {
                content: other_set.into_set(),
            });
        }
        let mut data = this.aref.content.clone();
        for elem in other.get().iterate(heap)? {
            let hashed = elem.get_hashed()?;
            data.insert_hashed(hashed);
        }
        Ok(SetData { content: data })
    }

    /// Return a new set with elements common to the set and all others.
    /// Unlike Python does not support variable number of arguments.
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// y = [3, 4, 5]
    /// x.intersection(y) == set([3])
    /// # "#);
    /// ```
    fn intersection<'v>(
        this: SetRef<'v>,
        #[starlark(require=pos)] other: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<SetData<'v>> {
        let other_set = SetFromValue::from_value(other, heap)?;
        let mut data = SetData::default();
        if other_set.is_empty() {
            return Ok(data);
        }

        for hashed in this.aref.content.iter_hashed() {
            if other_set.contains_hashed(hashed.copied()) {
                data.content.insert_hashed_unique_unchecked(hashed.copied());
            }
        }
        Ok(data)
    }

    /// Returns a new set with elements in either the set or the specified iterable but not both.
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// y = [3, 4, 5]
    /// x.symmetric_difference(y) == set([1, 2, 4, 5])
    /// # "#);
    /// ```
    fn symmetric_difference<'v>(
        this: SetRef<'v>,
        #[starlark(require=pos)] other: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<SetData<'v>> {
        let other_set = SetFromValue::from_value(other, heap)?;

        if other_set.is_empty() {
            return Ok(SetData {
                content: this.aref.content.clone(),
            });
        }

        //TODO(romanp) add symmetric_difference to small set and use it here and in xor
        if this.aref.content.is_empty() {
            return Ok(SetData {
                content: other_set.into_set(),
            });
        }

        let mut data = SetData::default();
        for elem in this.aref.content.iter_hashed() {
            if !other_set.contains_hashed(elem.copied()) {
                data.add_hashed(elem.copied());
            }
        }

        for elem in other_set.get() {
            let hashed = elem.get_hashed()?;
            if !this.aref.content.contains_hashed(hashed.as_ref()) {
                data.add_hashed(hashed);
            }
        }
        Ok(data)
    }

    /// Add an item to the set.
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// x.add(4)
    /// x == set([1, 2, 3, 4])
    /// # "#);
    /// ```
    fn add<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] value: Value<'v>,
    ) -> starlark::Result<NoneType> {
        let mut this = SetMut::from_value(this)?;
        let hashed = value.get_hashed()?;
        this.aref.add_hashed(hashed);
        Ok(NoneType)
    }

    /// Update the set by adding items from an iterable.
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 3, 2])
    /// x.update([4, 3])
    /// list(x) == [1, 3, 2, 4]
    /// # "#);
    /// ```
    fn update<'v>(
        this: Value<'v>,
        #[starlark(require=pos)] other: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<NoneType> {
        let is_self_ptr = other.get().ptr_eq(this);
        let mut this = SetMut::from_value(this)?;
        if is_self_ptr {
            return Ok(NoneType);
        }

        if this.aref.content.is_empty() {
            this.aref.content = SetFromValue::from_value(other, heap)?.into_set();
        } else {
            for elem in other.get().iterate(heap)? {
                let hashed = elem.get_hashed()?;
                this.aref.add_hashed(hashed);
            }
        }

        Ok(NoneType)
    }

    /// Remove the item from the set. It raises an error if there is no such item.
    ///
    /// `remove` fails if the key is unhashable or if the dictionary is
    /// frozen.
    /// Time complexity of this operation is *O(N)* where *N* is the number of entries in the set.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// x.remove(2)
    /// x == set([1, 3])
    /// # "#)
    /// ```
    /// A subsequent call to `x.remove(2)` would yield an error because the
    /// element won't be found.
    /// ```
    /// # starlark::assert::fail(r#"
    /// x = set([1, 2, 3])
    /// x.remove(2)
    /// x.remove(2) # error: not found
    /// # "#, "not found");
    /// ```
    fn remove<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] value: Value<'v>,
    ) -> starlark::Result<NoneType> {
        let mut set = SetMut::from_value(this)?;
        let hashed = value.get_hashed()?;
        if set.aref.remove_hashed(hashed.as_ref()) {
            Ok(NoneType)
        } else {
            mem::drop(set);
            Err(value_error!("`{value}` not found in `{this}`"))
        }
    }

    /// Remove the item from the set. It does nothing if there is no such item.
    ///
    /// `discard` fails if the key is unhashable or if the dictionary is
    /// frozen.
    /// Time complexity of this operation is *O(N)* where *N* is the number of entries in the set.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// x.discard(2)
    /// x == set([1, 3])
    /// # "#)
    /// ```
    /// A subsequent call to `x.discard(2)` would do nothing.
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// x.discard(2)
    /// x.discard(2)
    /// x == set([1, 3])
    /// # "#);
    /// ```
    fn discard<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] value: Value<'v>,
    ) -> starlark::Result<NoneType> {
        let mut set = SetMut::from_value(this)?;
        let hashed = value.get_hashed()?;
        set.aref.remove_hashed(hashed.as_ref());
        Ok(NoneType)
    }

    /// Removes and returns the **last** element of a set.
    ///
    /// `S.pop()` removes and returns the last element of the set S.
    ///
    /// `pop` fails if the set is empty, or if the set is frozen or has active iterators.
    /// Time complexity of this operation is *O(1)*.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// # (
    /// x.pop() == 3
    /// # and
    /// x.pop() == 2
    /// # and
    /// x == set([1])
    /// # )"#);
    /// ```
    fn pop<'v>(this: Value<'v>) -> starlark::Result<Value<'v>> {
        let mut set = SetMut::from_value(this)?;
        match set.aref.content.pop() {
            Some(x) => Ok(x),
            None => Err(value_error!("pop from an empty set")),
        }
    }

    /// Returns a new set with elements unique the set when compared to the specified iterable.
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// y = [3, 4, 5]
    /// x.difference(y) == set([1, 2])
    /// # "#);
    /// ```
    fn difference<'v>(
        this: SetRef<'v>,
        #[starlark(require=pos)] other: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<SetData<'v>> {
        if this.aref.content.is_empty() {
            other.get().iterate(heap)?;
            return Ok(SetData::default());
        }

        let other_set = SetFromValue::from_value(other, heap)?;

        if other_set.is_empty() {
            return Ok(SetData {
                content: this.aref.content.clone(),
            });
        }

        let mut data = SetData::default();
        for elem in this.aref.content.iter_hashed() {
            if !other_set.contains_hashed(elem.copied()) {
                data.add_hashed(elem.copied());
            }
        }
        Ok(data)
    }

    /// Test whether every element other iterable is in the set.
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// y = [1, 3]
    /// x.issuperset(y) == True
    /// # "#);
    /// ```
    fn issuperset<'v>(
        this: SetRef<'v>,
        #[starlark(require=pos)] other: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<bool> {
        let other_var;
        let other = if let Some(other) = SetRef::unpack_value_opt(other.get()) {
            if this.aref.content.len() < other.aref.content.len() {
                return Ok(false);
            }
            other_var = other;
            Either::Left(other_var.aref.content.iter_hashed().map(|v| Ok(v.copied())))
        } else {
            Either::Right(other.get().iterate(heap)?.map(|v| v.get_hashed()))
        };

        for elem in other {
            if !this.aref.contains_hashed(elem?) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Test whether every element in the set is in other iterable.
    /// ```
    /// # starlark::assert::is_true(r#"
    /// x = set([1, 2, 3])
    /// y = [3, 1, 2]
    /// x.issubset(y)
    /// # "#);
    /// ```
    fn issubset<'v>(
        this: SetRef<'v>,
        #[starlark(require=pos)] other: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        heap: &'v Heap,
    ) -> starlark::Result<bool> {
        if this.aref.content.is_empty() {
            other.get().iterate(heap)?;
            return Ok(true);
        }
        let rhs = SetFromValue::from_value(other, heap)?;
        if this.aref.content.len() > rhs.get().len() {
            return Ok(false);
        }
        for elem in this.aref.content.iter_hashed() {
            if !rhs.contains_hashed(elem.copied()) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;
    use crate::assert::Assert;

    #[test]
    fn test_empty() {
        assert::is_true("s = set(); len(s) == 0")
    }

    #[test]
    fn test_single() {
        assert::is_true("s = set([0, 1]); len(s) == 2")
    }

    #[test]
    fn test_eq() {
        assert::is_true("set([1, 2, 3]) == set([3, 2, 1])")
    }

    #[test]
    fn test_clear() {
        assert::is_true("s = set([1, 2, 3]); s.clear(); s == set()")
    }

    #[test]
    fn test_type() {
        assert::eq("type(set([1, 2, 3]))", "'set'")
    }

    #[test]
    fn test_iter() {
        assert::is_true("list([elem for elem in set([1, 2, 3])]) ==  [1, 2, 3]")
    }

    #[test]
    fn test_bool_true() {
        assert::is_true("bool(set([1, 2, 3]))")
    }

    #[test]
    fn test_bool_false() {
        assert::is_false("bool(set())")
    }

    #[test]
    fn test_union() {
        assert::eq(
            "set([1, 2, 3]).union(set([3, 4, 5]))",
            "set([1, 2, 3, 4, 5])",
        )
    }

    #[test]
    fn test_union_empty() {
        assert::eq("set([1, 2, 3]).union(set([]))", "set([1, 2, 3])")
    }

    #[test]
    fn test_union_iter() {
        assert::eq("set([1, 2, 3]).union([3, 4])", "set([1, 2, 3, 4])")
    }

    #[test]
    fn test_union_ordering_mixed() {
        assert::eq("list(set([1, 3, 5]).union(set([4, 3])))", "[1, 3, 5, 4]");
    }

    #[test]
    fn test_intersection() {
        assert::eq("set([1, 2, 3]).intersection(set([3, 4, 5]))", "set([3])")
    }

    #[test]
    fn test_intersection_empty() {
        assert::eq("set([1, 2, 3]).intersection(set([]))", "set([])")
    }

    #[test]
    fn test_intersection_iter() {
        assert::eq("set([1, 2, 3]).intersection([3, 4])", "set([3])")
    }

    #[test]
    fn test_intersection_order() {
        assert::eq("list(set([1, 2, 3]).intersection([4, 3, 1]))", "[1, 3]")
    }

    #[test]
    fn test_symmetric_difference() {
        assert::eq(
            "set([1, 2, 3]).symmetric_difference(set([3, 4, 5]))",
            "set([1, 2, 4, 5])",
        )
    }

    #[test]
    fn test_symmetric_difference_empty() {
        assert::eq(
            "set([1, 2, 3]).symmetric_difference(set([]))",
            "set([1, 2, 3])",
        )
    }

    #[test]
    fn test_symmetric_difference_iter() {
        assert::eq(
            "set([1, 2, 3]).symmetric_difference([3, 4])",
            "set([1, 2, 4])",
        )
    }

    #[test]
    fn test_symmetric_difference_ord() {
        assert::eq(
            "list(set([1, 2, 3, 7]).symmetric_difference(set([4, 3, 1])))",
            "[2, 7, 4]",
        )
    }

    #[test]
    fn test_add() {
        assert::eq(r#"x = set([1, 2, 3]);x.add(0);x"#, "set([0, 1, 2, 3])")
    }

    #[test]
    fn test_add_empty() {
        assert::eq(r#"x = set([]);x.add(0);x"#, "set([0])")
    }

    #[test]
    fn test_add_existing() {
        assert::eq(r#"x = set([0]);x.add(0);x"#, "set([0])")
    }

    #[test]
    fn test_add_order() {
        assert::eq(r#"x = set([1, 2, 3]);x.add(2);list(x)"#, "[1, 2, 3]");
        assert::eq(r#"x = set([1, 2, 3]);x.add(0);list(x)"#, "[1, 2, 3, 0]")
    }

    #[test]
    fn test_remove() {
        assert::eq("x = set([0, 1]);x.remove(1);x", "set([0])")
    }

    #[test]
    fn test_remove_empty() {
        assert::fail("set([]).remove(0)", "`0` not found in `set([])`");
    }

    #[test]
    fn test_remove_not_existing() {
        assert::fail("set([1]).remove(0)", "`0` not found in `set([1])`");
    }

    #[test]
    fn test_discard() {
        assert::eq("x = set([0, 1]);x.discard(1);x", "set([0])")
    }

    #[test]
    fn test_discard_multiple_times() {
        assert::eq("x = set([0, 1]); x.discard(0); x.discard(0); x", "set([1])");
    }

    #[test]
    fn test_pop() {
        assert::is_true("x = set([1, 0]); (x.pop() == 0 and x.pop() == 1 and x == set())");
    }

    #[test]
    fn test_pop_empty() {
        assert::fail("x = set([]); x.pop()", "pop from an empty set");
    }

    #[test]
    fn test_difference() {
        assert::eq("set([1, 2, 3]).difference(set([2]))", "set([1, 3])")
    }

    #[test]
    fn test_difference_iter() {
        assert::eq("set([1, 2, 3]).difference([3, 2])", "set([1])")
    }

    #[test]
    fn test_difference_order() {
        assert::eq("list(set([3, 2, 1]).difference([2]))", "[3, 1]")
    }

    #[test]
    fn test_difference_empty_lhs() {
        assert::eq("set([]).difference(set([2]))", "set([])")
    }

    #[test]
    fn test_difference_empty_rhs() {
        assert::eq("set([1, 2]).difference(set([]))", "set([2, 1])")
    }

    #[test]
    fn test_is_superset() {
        assert::is_true("set([1, 2, 3, 4]).issuperset(set([1, 3, 2]))")
    }

    #[test]
    fn test_is_not_superset() {
        assert::is_false("set([1, 2]).issuperset(set([1, 3, 5]))")
    }

    #[test]
    fn test_is_not_superset_empty_lhs() {
        assert::is_false("set([]).issuperset(set([1]))");
    }

    #[test]
    fn test_is_superset_empty_rhs() {
        assert::is_true("set([1, 2]).issuperset(set([]))");
        assert::is_true("set([]).issuperset(set([]))")
    }

    #[test]
    fn test_is_superset_iter() {
        assert::is_true("set([1, 2, 3]).issuperset([3, 1])")
    }

    #[test]
    fn test_is_subset() {
        assert::is_true("set([1, 2]).issubset(set([1, 3, 2]))")
    }

    #[test]
    fn test_is_not_subset() {
        assert::is_false("set([1, 2]).issubset(set([1, 3, 5]))")
    }

    #[test]
    fn test_is_subset_empty_lhs() {
        assert::is_true("set([]).issubset(set([1, 3, 5]))");
        assert::is_true("set([]).issubset(set([]))")
    }

    #[test]
    fn test_is_not_subset_empty_rhs() {
        assert::is_false("set([1, 2]).issubset(set([]))")
    }

    #[test]
    fn test_is_subset_iter() {
        assert::is_true("set([1, 2]).issubset([1, 3, 2])")
    }

    #[test]
    fn test_update() {
        assert::eq(
            "x = set([1, 3, 2]); x.update([4, 3]); list(x)",
            "[1, 3, 2, 4]",
        )
    }

    #[test]
    fn test_update_empty() {
        assert::eq("x = set([1, 3, 2]); x.update([]); list(x)", "[1, 3, 2]");
        assert::eq("x = set(); x.update(set([4, 3])); list(x)", "[4, 3]");
        assert::eq("x = set(); x.update([]); x", "set()");
    }

    #[test]
    fn test_update_self() {
        assert::eq("x = set([1, 3, 2]); x.update(x); list(x)", "[1, 3, 2]")
    }

    #[test]
    fn test_update_frozen_set_cannot_be_updated_with_self() {
        let mut a = Assert::new();
        a.module("s.star", "S = set([1, 2, 3])");
        a.fail(
            r#"
load('s.star', 'S')

S.update(S)
"#,
            "Immutable",
        );
    }
}
