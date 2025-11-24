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

use allocative::Allocative;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::codemap::Span;
use crate::environment::GlobalsBuilder;
use crate::typing::call_args::TyCallArgs;
use crate::typing::callable::TyCallable;
use crate::typing::error::TypingOrInternalError;
use crate::typing::function::TyCustomFunctionImpl;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::typing::TypingOracleCtx;
use crate::values::tuple::UnpackTuple;
use crate::values::typing::StarlarkIter;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Value;
use crate::values::ValueOfUnchecked;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Allocative)]
struct ZipType;

impl TyCustomFunctionImpl for ZipType {
    fn as_callable(&self) -> TyCallable {
        // TODO(nga): this should be obtained from function signature from function definition.
        TyCallable::new(ParamSpec::args(Ty::iter(Ty::any())), Ty::list(Ty::any()))
    }

    fn validate_call(
        &self,
        _span: Span,
        args: &TyCallArgs,
        oracle: TypingOracleCtx,
    ) -> Result<Ty, TypingOrInternalError> {
        let mut iter_item_types: Vec<Ty> = Vec::new();
        for pos in &args.pos {
            let item_ty = oracle.iter_item(pos.as_ref())?;
            iter_item_types.push(item_ty);
        }
        if args.args.is_some() {
            Ok(Ty::list(Ty::any()))
        } else {
            Ok(Ty::list(Ty::tuple(iter_item_types)))
        }
    }
}

#[starlark_module]
pub(crate) fn register_zip(globals: &mut GlobalsBuilder) {
    /// [zip](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#zip
    /// ): zip several iterables together
    ///
    /// `zip()` returns a new list of n-tuples formed from corresponding
    /// elements of each of the n iterable sequences provided as arguments to
    /// `zip`.  That is, the first tuple contains the first element of each of
    /// the sequences, the second element contains the second element of each
    /// of the sequences, and so on.  The result list is only as long as the
    /// shortest of the input sequences.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// zip()                           == []
    /// zip(range(5))                   == [(0,), (1,), (2,), (3,), (4,)]
    /// zip(range(5), "abc".elems())    == [(0, "a"), (1, "b"), (2, "c")]
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe, ty_custom_function = ZipType)]
    fn zip<'v>(
        #[starlark(args)] args: UnpackTuple<ValueOfUnchecked<'v, StarlarkIter<FrozenValue>>>,
        heap: &'v Heap,
    ) -> starlark::Result<Vec<Value<'v>>> {
        let mut v = Vec::new();
        let mut first = true;
        for arg in args.items {
            let mut idx = 0;
            for e in arg.get().iterate(heap)? {
                if first {
                    v.push(heap.alloc((e,)));
                    idx += 1;
                } else if idx < v.len() {
                    v[idx] = v[idx].add(heap.alloc((e,)), heap)?;
                    idx += 1;
                }
            }
            v.truncate(idx);
            first = false;
        }
        Ok(v)
    }
}
