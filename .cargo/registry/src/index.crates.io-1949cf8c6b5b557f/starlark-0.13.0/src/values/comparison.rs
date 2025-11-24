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

use std::cmp::Ordering;
use std::hash::Hash;

use itertools::Itertools;
use starlark_map::small_set::SmallSet;
use starlark_map::Equivalent;

use crate::collections::SmallMap;

pub(crate) fn equals_slice<E, X1, X2>(
    xs: &[X1],
    ys: &[X2],
    f: impl Fn(&X1, &X2) -> Result<bool, E>,
) -> Result<bool, E> {
    if xs.len() != ys.len() {
        return Ok(false);
    }
    for (x, y) in xs.iter().zip(ys) {
        if !f(x, y)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn equals_small_map<E, K1: Eq, K2: Eq, V1, V2>(
    x: &SmallMap<K1, V1>,
    y: &SmallMap<K2, V2>,
    f: impl Fn(&V1, &V2) -> Result<bool, E>,
) -> Result<bool, E>
where
    K1: Equivalent<K2>,
{
    if x.len() != y.len() {
        return Ok(false);
    }
    for (xk, xv) in x.iter_hashed() {
        match y.get_hashed(xk) {
            None => return Ok(false),
            Some(yv) => {
                if !f(xv, yv)? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

pub(crate) fn equals_small_set<K1, K2>(xs: &SmallSet<K1>, ys: &SmallSet<K2>) -> bool
where
    K1: Equivalent<K2> + Eq,
    K2: Eq,
{
    if xs.len() != ys.len() {
        return false;
    }

    for x in xs.iter_hashed() {
        if !ys.contains_hashed(x) {
            return false;
        }
    }

    true
}

pub(crate) fn compare_slice<E, X1, X2>(
    xs: &[X1],
    ys: &[X2],
    f: impl Fn(&X1, &X2) -> Result<Ordering, E>,
) -> Result<Ordering, E> {
    for (x, y) in xs.iter().zip(ys) {
        let cmp = f(x, y)?;
        if cmp != Ordering::Equal {
            return Ok(cmp);
        }
    }
    Ok(xs.len().cmp(&ys.len()))
}

pub(crate) fn compare_small_map<E, K, K2: Ord + Hash, V1, V2>(
    x: &SmallMap<K, V1>,
    y: &SmallMap<K, V2>,
    key: impl Fn(&K) -> K2,
    f: impl Fn(&V1, &V2) -> Result<Ordering, E>,
) -> Result<Ordering, E> {
    // TODO(nga): this function is only used to compare structs,
    //   and it compares them incorrectly. This code is supposed to return `False`:
    //   ```
    //   struct(b=1) < struct(a=1, x=1)
    //   ```
    //   It returns `True`.
    let cmp = x.len().cmp(&y.len());
    if cmp != Ordering::Equal {
        return Ok(cmp);
    }
    for ((xk, xv), (yk, yv)) in x
        .iter()
        .sorted_by_key(|(k, _)| key(k))
        .zip(y.iter().sorted_by_key(|(k, _)| key(k)))
    {
        let key_cmp = key(xk).cmp(&key(yk));
        if key_cmp != Ordering::Equal {
            return Ok(key_cmp);
        }
        let value_cmp = f(xv, yv)?;
        if value_cmp != Ordering::Equal {
            return Ok(value_cmp);
        }
    }
    Ok(Ordering::Equal)
}
