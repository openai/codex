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

//! Utilities for implementing [`StarlarkValue::export_as`](crate::values::StarlarkValue::export_as).
//!
//! See the trait [`ExportedName`](crate::values::exported_name::ExportedName) for more details.

use std::cell::Ref;
use std::cell::RefCell;
use std::fmt::Debug;
use std::fmt::Display;

use allocative::Allocative;
use either::Either;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::Trace;

/// Borrowed [`ExportedName`].
pub struct BorrowedExportedName<'a> {
    either: Either<&'a str, Ref<'a, str>>,
}

impl<'a> BorrowedExportedName<'a> {
    /// Dereference to a string slice.
    pub fn as_str(&self) -> &str {
        match &self.either {
            Either::Left(s) => s,
            Either::Right(s) => s,
        }
    }
}

impl<'a> PartialEq for BorrowedExportedName<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<'a> Eq for BorrowedExportedName<'a> {}

/// Type parameter for types which need to be exported with a name.
///
/// Typical use case is this:
///
/// ```
/// use allocative::Allocative;
/// use starlark::eval::Evaluator;
/// use starlark::values::exported_name::ExportedName;
/// use starlark::values::exported_name::FrozenExportedName;
/// use starlark::values::StarlarkValue;
/// use starlark_derive::starlark_value;
/// use starlark_derive::NoSerialize;
/// use starlark_derive::ProvidesStaticType;
///
/// #[derive(
///     Debug,
///     NoSerialize,
///     ProvidesStaticType,
///     Allocative,
///     derive_more::Display
/// )]
/// #[display("{:?}", self)]
/// struct MyStruct<T: ExportedName + 'static> {
///     name: T,
/// }
///
/// #[starlark_value(type = "MyStruct")]
/// impl<'v, T: ExportedName> StarlarkValue<'v> for MyStruct<T> {
///     type Canonical = MyStruct<FrozenExportedName>;
///
///     fn export_as(
///         &self,
///         variable_name: &str,
///         _eval: &mut Evaluator<'v, '_, '_>,
///     ) -> starlark::Result<()> {
///         self.name.try_export_as(variable_name);
///         Ok(())
///     }
/// }
/// ```
///
/// This is an utility implementing common pattern,
/// but it is completely optional when implementing
/// [`StarlarkValue::export_as`](crate::values::StarlarkValue::export_as).
pub trait ExportedName:
    Debug + Display + Freeze<Frozen = FrozenExportedName> + Allocative + 'static
{
    /// Borrow the name.
    fn borrow(&self) -> Option<BorrowedExportedName>;

    /// Name is equal to the given string.
    fn equal_to(&self, rhs: &str) -> bool;

    /// Try update the name.
    ///
    /// This operation is no-op, if
    /// - the name is already set (first export wins)
    /// - the name is frozen
    fn try_export_as(&self, name: &str);
}

/// Type parameter for unfrozen types.
#[derive(Debug, Allocative, Default, Trace, ProvidesStaticType)]
pub struct MutableExportedName {
    name: RefCell<Option<String>>,
}

/// Type parameter for frozen types.
#[derive(Debug, Allocative, Default, Trace, Freeze, ProvidesStaticType)]
pub struct FrozenExportedName {
    name: Option<String>,
}

impl Freeze for MutableExportedName {
    type Frozen = FrozenExportedName;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(FrozenExportedName {
            name: self.name.into_inner(),
        })
    }
}

impl Display for MutableExportedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.name.borrow().as_deref().unwrap_or("<not_exported>")
        )
    }
}

impl Display for FrozenExportedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name.as_deref().unwrap_or("<not_exported>"))
    }
}

impl ExportedName for MutableExportedName {
    fn borrow(&self) -> Option<BorrowedExportedName> {
        let r = Ref::filter_map(self.name.borrow(), |n| n.as_deref());
        r.ok().map(|n| BorrowedExportedName {
            either: Either::Right(n),
        })
    }

    fn equal_to(&self, rhs: &str) -> bool {
        self.name.borrow().as_deref() == Some(rhs)
    }

    fn try_export_as(&self, name: &str) {
        self.name
            .borrow_mut()
            .get_or_insert_with(|| name.to_owned());
    }
}

impl ExportedName for FrozenExportedName {
    fn borrow(&self) -> Option<BorrowedExportedName> {
        self.name.as_deref().map(|n| BorrowedExportedName {
            either: Either::Left(n),
        })
    }

    fn equal_to(&self, rhs: &str) -> bool {
        self.name.as_deref() == Some(rhs)
    }

    fn try_export_as(&self, name: &str) {
        let _ignore = name;
    }
}
