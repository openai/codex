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

//! Define a common set of errors.

use thiserror::Error;

use crate::values::StarlarkValue;
use crate::values::Value;

/// Common errors returned by Starlark evaluation.
#[derive(Debug, Error)]
#[allow(missing_docs)] // Self-explanatory.
pub enum ValueError {
    #[error("Operation `{op}` not supported on type `{typ}`")]
    OperationNotSupported { op: String, typ: String },
    #[error("Operation `{op}` not supported for types `{left}` and `{right}`")]
    OperationNotSupportedBinary {
        op: String,
        left: String,
        right: String,
    },
    #[error("Cannot divide by zero")]
    DivisionByZero,
    #[error("Integer overflow")]
    IntegerOverflow,
    #[error("Negative shift count")]
    NegativeShiftCount,
    #[error("Type of parameters mismatch")]
    IncorrectParameterType,
    #[error("Type of parameter `{0}` doesn't match")]
    IncorrectParameterTypeNamed(String),
    #[error("Missing this parameter")]
    MissingThis,
    #[error("Missing required parameter `{0}`")]
    MissingRequired(String),
    #[error("Index `{0}` is out of bound")]
    IndexOutOfBound(i32),
    #[error("Key `{0}` was not found")]
    KeyNotFound(String),
    #[error("Immutable")]
    CannotMutateImmutableValue,
    #[error("This operation mutate an iterable for an iterator while iterating.")]
    MutationDuringIteration,
    #[error("Object of type `{0}` has no attribute `{1}`")]
    NoAttr(String, String),
    #[error("Object of type `{0}` has no attribute `{1}`, did you mean `{2}`?")]
    NoAttrDidYouMean(String, String, String),
}

impl From<ValueError> for crate::Error {
    fn from(e: ValueError) -> Self {
        crate::Error::new_kind(crate::ErrorKind::Value(anyhow::Error::new(e)))
    }
}

#[derive(Debug, Error)]
pub(crate) enum ControlError {
    #[error("Value of type `{0}` is not hashable")]
    NotHashableValue(String),
    #[error("Too many recursion levels")]
    TooManyRecursionLevel,
}

impl ValueError {
    #[cold]
    pub(crate) fn unsupported_owned<T>(
        left: &str,
        op: &str,
        right: Option<&str>,
    ) -> crate::Result<T> {
        match right {
            None => Err(ValueError::OperationNotSupported {
                op: op.to_owned(),
                typ: left.to_owned(),
            }
            .into()),
            Some(right) => Err(ValueError::OperationNotSupportedBinary {
                op: op.to_owned(),
                left: left.to_owned(),
                right: right.to_owned(),
            }
            .into()),
        }
    }

    /// Helper to create an [`OperationNotSupported`](ValueError::OperationNotSupported) error.
    #[cold]
    pub fn unsupported<'v, T, V: StarlarkValue<'v>>(_left: &V, op: &str) -> crate::Result<T> {
        Self::unsupported_owned(V::TYPE, op, None)
    }

    #[cold]
    pub(crate) fn unsupported_type<T>(left: Value, op: &str) -> crate::Result<T> {
        Self::unsupported_owned(left.get_type(), op, None)
    }

    /// Helper to create an [`OperationNotSupported`](ValueError::OperationNotSupportedBinary) error.
    #[cold]
    pub fn unsupported_with<'v, T, V: StarlarkValue<'v>>(
        _left: &V,
        op: &str,
        right: Value,
    ) -> crate::Result<T> {
        Self::unsupported_owned(V::TYPE, op, Some(right.get_type()))
    }
}
