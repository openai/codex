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

use std::fmt;
use std::fmt::Display;
use std::num::NonZeroI32;

use allocative::Allocative;
use dupe::Dupe;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::starlark_simple_value;
use crate::typing::Ty;
use crate::values::index::convert_index;
use crate::values::index::convert_slice_indices;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLike;

/// Representation of `range()` type.
#[derive(Clone, Copy, Dupe, Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct Range {
    start: i32,
    stop: i32,
    step: NonZeroI32,
}

impl Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.step.get() != 1 {
            write!(f, "range({}, {}, {})", self.start, self.stop, self.step)
        } else if self.start != 0 {
            write!(f, "range({}, {})", self.start, self.stop)
        } else {
            write!(f, "range({})", self.stop)
        }
    }
}

starlark_simple_value!(Range);

impl Range {
    /// The result of calling `type()` on a range.
    pub const TYPE: &'static str = "range";

    /// Create a new [`Range`].
    pub fn new(start: i32, stop: i32, step: NonZeroI32) -> Range {
        Range { start, stop, step }
    }

    fn equals_range(&self, other: &Range) -> crate::Result<bool> {
        let self_length = self.length()?;
        let other_length = other.length()?;
        if self_length == 0 || other_length == 0 {
            return Ok(self_length == other_length);
        }
        if self.start != other.start {
            return Ok(false);
        }
        if self_length == 1 || other_length == 1 {
            return Ok(self_length == other_length);
        }
        debug_assert!(self_length > 1);
        debug_assert!(other_length > 1);
        if self.step.get() == other.step.get() {
            return Ok(self_length == other_length);
        } else {
            return Ok(false);
        }
    }

    fn rem_range_at_iter(&self, index: usize) -> Option<Range> {
        let index = i64::try_from(index).ok()?;

        let start =
            (self.start as i64).saturating_add(index.saturating_mul(self.step.get() as i64));

        Some(Range {
            start: i32::try_from(start).ok()?,
            stop: self.stop,
            step: self.step,
        })
    }
}

#[starlark_value(type = Range::TYPE)]
impl<'v> StarlarkValue<'v> for Range {
    fn to_bool(&self) -> bool {
        (self.start < self.stop && self.step.get() > 0)
            || (self.start > self.stop && self.step.get() < 0)
    }

    fn length(&self) -> crate::Result<i32> {
        if self.start == self.stop {
            return Ok(0);
        }

        // If step is into opposite direction of stop, then length is zero.
        if (self.stop >= self.start) != (self.step.get() > 0) {
            return Ok(0);
        }

        // Convert range and step to `u64`
        let (dist, step) = if self.step.get() >= 0 {
            (
                self.stop.wrapping_sub(self.start) as u64,
                self.step.get() as u64,
            )
        } else {
            (
                self.start.wrapping_sub(self.stop) as u64,
                self.step.get().wrapping_neg() as u64,
            )
        };
        let i = ((dist - 1) / step + 1) as i32;
        if i >= 0 {
            Ok(i)
        } else {
            Err(ValueError::IntegerOverflow.into())
        }
    }

    fn at(&self, index: Value, heap: &'v Heap) -> crate::Result<Value<'v>> {
        let index = convert_index(index, self.length()?)?;
        // Must not overflow if `length` is computed correctly
        Ok(heap.alloc(self.start + self.step.get() * index))
    }

    fn equals(&self, other: Value) -> crate::Result<bool> {
        if let Some(other) = other.downcast_ref::<Self>() {
            self.equals_range(other)
        } else {
            Ok(false)
        }
    }

    fn slice(
        &self,
        start: Option<Value>,
        stop: Option<Value>,
        stride: Option<Value>,
        heap: &'v Heap,
    ) -> crate::Result<Value<'v>> {
        let (start, stop, step) = convert_slice_indices(self.length()?, start, stop, stride)?;
        return Ok(heap.alloc(Range {
            start: self
                .start
                .checked_add(
                    start
                        .checked_mul(self.step.get())
                        .ok_or(ValueError::IntegerOverflow)?,
                )
                .ok_or(ValueError::IntegerOverflow)?,
            stop: self
                .start
                .checked_add(
                    stop.checked_mul(self.step.get())
                        .ok_or(ValueError::IntegerOverflow)?,
                )
                .ok_or(ValueError::IntegerOverflow)?,
            step: NonZeroI32::new(
                step.checked_mul(self.step.get())
                    .ok_or(ValueError::IntegerOverflow)?,
            )
            .unwrap(),
        }));
    }

    unsafe fn iterate(&self, me: Value<'v>, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(me)
    }

    unsafe fn iter_next(&self, index: usize, heap: &'v Heap) -> Option<Value<'v>> {
        let rem_range = self.rem_range_at_iter(index)?;

        if !rem_range.to_bool() {
            return None;
        }

        Some(heap.alloc(rem_range.start))
    }

    unsafe fn iter_size_hint(&self, index: usize) -> (usize, Option<usize>) {
        let Some(rem_range) = self.rem_range_at_iter(index) else {
            return (0, Some(0));
        };
        match rem_range.length() {
            Ok(length) => (length as usize, Some(length as usize)),
            Err(_) => (0, None),
        }
    }

    unsafe fn iter_stop(&self) {}

    fn is_in(&self, other: Value) -> crate::Result<bool> {
        let other = match other.unpack_num().and_then(|n| n.as_int()) {
            Some(other) => other,
            None => {
                // Consider `"a" in range(3)`
                //
                // Should we error or return false?
                // Go Starlark errors. Python returns false.
                // Discussion at https://github.com/bazelbuild/starlark/issues/175
                return Ok(false);
            }
        };
        if !self.to_bool() {
            return Ok(false);
        }
        if self.start == other {
            return Ok(true);
        }
        if self.step.get() > 0 {
            if other < self.start || other >= self.stop {
                return Ok(false);
            }
            Ok((other.wrapping_sub(self.start) as u64) % (self.step.get() as u64) == 0)
        } else {
            if other > self.start || other <= self.stop {
                return Ok(false);
            }
            Ok(
                (self.start.wrapping_sub(other) as u64) % (self.step.get().wrapping_neg() as u64)
                    == 0,
            )
        }
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::starlark_value::<Range>()
    }
}

/// For tests
impl PartialEq for Range {
    fn eq(&self, other: &Range) -> bool {
        self.equals_range(other).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroI32;

    use crate::assert;
    use crate::values::range::Range;
    use crate::values::types::int::inline_int::InlineInt;
    use crate::values::Heap;
    use crate::values::StarlarkValue;
    use crate::values::Value;

    fn range(start: i32, stop: i32, range: i32) -> Range {
        Range {
            start,
            stop,
            step: NonZeroI32::new(range).unwrap(),
        }
    }

    fn range_start_stop(start: i32, stop: i32) -> Range {
        range(start, stop, 1)
    }

    fn range_stop(stop: i32) -> Range {
        range_start_stop(0, stop)
    }

    #[test]
    fn length_stop() {
        assert_eq!(Some(0), range_stop(0).length().ok());
        assert_eq!(Some(17), range_stop(17).length().ok());
    }

    #[test]
    fn length_start_stop() {
        assert_eq!(Some(20), range_start_stop(10, 30).length().ok());
        assert_eq!(Some(0), range_start_stop(10, -30).length().ok());
        assert_eq!(Some(i32::MAX), range_start_stop(0, i32::MAX).length().ok());
        assert!(range_start_stop(-1, i32::MAX).length().is_err());
    }

    #[test]
    fn length_start_stop_step() {
        assert_eq!(Some(5), range(0, 10, 2).length().ok());
        assert_eq!(Some(5), range(0, 9, 2).length().ok());
        assert_eq!(Some(0), range(0, 10, -2).length().ok());
        assert_eq!(Some(5), range(10, 0, -2).length().ok());
        assert_eq!(Some(5), range(9, 0, -2).length().ok());
        assert_eq!(Some(1), range(4, 14, 10).length().ok());
    }

    #[test]
    fn eq() {
        assert_eq!(range_stop(0), range(2, 1, 3));
    }

    #[test]
    fn test_range_exhaustive() {
        // The range implementation is fairly hairy. Lots of corner cases etc.
        // Especially around equality, length.
        // Therefore, generate ranges exhaustively over a very small range
        // and test lots of properties about them.
        let mut ranges = Vec::with_capacity(294);
        for start in -3..4 {
            for stop in -3..4 {
                for step in -3..3 {
                    let step = if step >= 0 { step + 1 } else { step };
                    ranges.push(range(start, stop, step))
                }
            }
        }
        assert_eq!(ranges.len(), 294); // Assert we don't accidentally take too long

        let heap = Heap::new();
        for x in &ranges {
            let x = heap.alloc_simple(*x);
            let full: Vec<Value> = x.iterate(&heap).unwrap().collect();
            assert_eq!(x.length().unwrap(), full.len() as i32);
            for (i, v) in full.iter().enumerate() {
                assert_eq!(x.at(heap.alloc(i), &heap).unwrap(), *v);
            }
        }

        // Takes 294^2 steps - but completes instantly
        for x in &ranges {
            for y in &ranges {
                let x = heap.alloc_simple(*x);
                let y = heap.alloc_simple(*y);
                assert_eq!(
                    x == y,
                    Iterator::eq(x.iterate(&heap).unwrap(), y.iterate(&heap).unwrap())
                )
            }
        }
    }

    #[test]
    fn test_max_len() {
        assert::eq(
            &InlineInt::MAX.to_string(),
            &format!("len(range({}))", InlineInt::MAX),
        );
        assert::eq(
            &InlineInt::MAX.to_string(),
            &format!("len(range({}, -1))", InlineInt::MIN),
        );
    }
}
