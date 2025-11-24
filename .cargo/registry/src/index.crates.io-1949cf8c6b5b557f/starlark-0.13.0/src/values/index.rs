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

use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueError;

// Helper for convert_slice_indices
fn convert_index_aux(
    len: i32,
    v1: Option<Value>,
    default: i32,
    min: i32,
    max: i32,
) -> anyhow::Result<i32> {
    if let Some(v) = v1 {
        if v.is_none() {
            Ok(default)
        } else {
            let x = i32::unpack_value_err(v)?;
            let i = if x < 0 { len + x } else { x };
            if i < min {
                Ok(min)
            } else if i > max {
                Ok(max)
            } else {
                Ok(i)
            }
        }
    } else {
        Ok(default)
    }
}

/// Function to parse the index for at/set_at methods.
///
/// Return an `i32` from self corresponding to the index recenterd between 0
/// and len. Raise the correct errors if the value is not numeric or the
/// index is out of bound.
pub(crate) fn convert_index(v: Value, len: i32) -> anyhow::Result<i32> {
    let x = i32::unpack_value_err(v)?;
    let i = if x < 0 {
        len.checked_add(x).ok_or(ValueError::IntegerOverflow)?
    } else {
        x
    };
    if i < 0 || i >= len {
        Err(ValueError::IndexOutOfBound(i).into())
    } else {
        Ok(i)
    }
}

/// Parse indices for slicing.
///
/// Takes the object length and 3 optional values and returns `(i32, i32,
/// i32)` with those index correctly converted in range of length.
/// Return the correct errors if the values are not numeric or the stride is
/// 0.
pub(crate) fn convert_slice_indices(
    len: i32,
    start: Option<Value>,
    stop: Option<Value>,
    stride: Option<Value>,
) -> anyhow::Result<(i32, i32, i32)> {
    let stride = match stride {
        None => 1,
        Some(v) if v.is_none() => 1,
        Some(v) => i32::unpack_value_err(v)?,
    };
    match stride {
        0 => Err(ValueError::IndexOutOfBound(0).into()),
        stride => {
            let def_start = if stride < 0 { len - 1 } else { 0 };
            let def_end = if stride < 0 { -1 } else { len };
            let clamp = if stride < 0 { -1 } else { 0 };
            let start = convert_index_aux(len, start, def_start, clamp, len + clamp);
            let stop = convert_index_aux(len, stop, def_end, clamp, len + clamp);
            match (start, stop) {
                (Ok(s1), Ok(s2)) => Ok((s1, s2, stride)),
                (Err(x), ..) => Err(x),
                (Ok(..), Err(x)) => Err(x),
            }
        }
    }
}

pub(crate) fn apply_slice<T: Copy>(
    xs: &[T],
    start: Option<Value>,
    stop: Option<Value>,
    stride: Option<Value>,
) -> anyhow::Result<Vec<T>> {
    let (start, stop, stride) = convert_slice_indices(xs.len() as i32, start, stop, stride)?;
    if stride == 1 {
        if start >= stop {
            return Ok(Vec::new());
        } else {
            return Ok(xs[start as usize..stop as usize].to_vec());
        }
    }

    let (start, stop) = if stride < 0 {
        (stop + 1, start + 1)
    } else {
        (start, stop)
    };
    if start >= stop {
        return Ok(Vec::new());
    }
    let mut res = xs[start as usize..stop as usize].to_vec();
    if stride == -1 {
        res.reverse();
        return Ok(res);
    }
    if stride < 0 {
        res.reverse();
    }
    let astride = stride.abs();
    let res = res
        .into_iter()
        .enumerate()
        .filter_map(|x| {
            if 0 == (x.0 as i32 % astride) {
                Some(x.1)
            } else {
                None
            }
        })
        .collect();
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::types::int::inline_int::InlineInt;
    use crate::values::Heap;

    #[test]
    fn test_convert_index() {
        let heap = Heap::new();
        assert_eq!(Some(6), convert_index(Value::testing_new_int(6), 7).ok());
        assert_eq!(Some(6), convert_index(Value::testing_new_int(-1), 7).ok());
        assert_eq!(
            Some((6, 7, 1)),
            convert_slice_indices(7, Some(Value::testing_new_int(6)), None, None).ok()
        );
        assert_eq!(
            Some((6, -1, -1)),
            convert_slice_indices(
                7,
                Some(Value::testing_new_int(-1)),
                None,
                Some(Value::testing_new_int(-1))
            )
            .ok()
        );
        assert_eq!(
            Some((6, 7, 1)),
            convert_slice_indices(
                7,
                Some(Value::testing_new_int(-1)),
                Some(Value::testing_new_int(10)),
                None
            )
            .ok()
        );
        // Errors
        assert!(convert_index(heap.alloc("a"), 7).is_err());
        assert!(convert_index(Value::testing_new_int(8), 7).is_err()); // 8 > 7 = len
        assert!(convert_index(Value::testing_new_int(-8), 7).is_err()); // -8 + 7 = -1 < 0
    }

    #[test]
    fn test_apply_slice() {
        let s = &[0, 1, 2, 3, 4, 5, 6];

        let x = apply_slice(
            s,
            Some(Value::new_int(InlineInt::MINUS_ONE)),
            None,
            Some(Value::new_int(InlineInt::MINUS_ONE)),
        )
        .unwrap();
        assert_eq!(x, &[6, 5, 4, 3, 2, 1, 0]);

        let x = apply_slice(
            s,
            Some(Value::new_int(InlineInt::MINUS_ONE)),
            None,
            Some(Value::new_int(InlineInt::MINUS_ONE)),
        )
        .unwrap();
        assert_eq!(x, &[6, 5, 4, 3, 2, 1, 0]);

        let x = apply_slice(
            s,
            Some(Value::testing_new_int(0)),
            Some(Value::testing_new_int(3)),
            Some(Value::testing_new_int(2)),
        )
        .unwrap();
        assert_eq!(x, &[0, 2]);

        let x = apply_slice(
            s,
            Some(Value::testing_new_int(5)),
            Some(Value::testing_new_int(2)),
            Some(Value::testing_new_int(-2)),
        )
        .unwrap();
        assert_eq!(x, &[5, 3]);

        let x = apply_slice(
            s,
            Some(Value::testing_new_int(-1)),
            Some(Value::testing_new_int(-5)),
            Some(Value::testing_new_int(-1)),
        )
        .unwrap();
        assert_eq!(x, &[6, 5, 4, 3]);

        let x = apply_slice(
            s,
            Some(Value::testing_new_int(-1)),
            Some(Value::testing_new_int(0)),
            Some(Value::testing_new_int(-1)),
        )
        .unwrap();
        assert_eq!(x, &[6, 5, 4, 3, 2, 1]);

        let x = apply_slice(
            &[1, 2, 3],
            Some(Value::testing_new_int(0)),
            Some(Value::testing_new_int(-2)),
            Some(Value::testing_new_int(-1)),
        )
        .unwrap();
        assert_eq!(x, &[] as &[i32]);
    }
}
