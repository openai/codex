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

use std::fmt::Debug;
use std::iter;
use std::marker::PhantomData;

use dupe::Clone_;
use dupe::Dupe_;
use either::Either;
use starlark_map::small_set::SmallSet;
use starlark_syntax::value_error;
use thiserror::Error;

use crate::cast::transmute;
use crate::coerce::coerce;
use crate::coerce::Coerce;
use crate::collections::symbol::symbol::Symbol;
use crate::collections::Hashed;
use crate::collections::SmallMap;
use crate::collections::StarlarkHashValue;
use crate::eval::ParametersSpec;
use crate::hint::unlikely;
use crate::values::dict::Dict;
use crate::values::dict::DictRef;
use crate::values::iter::StarlarkIterator;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::ValueLike;

#[derive(Debug, Clone, Error)]
pub(crate) enum FunctionError {
    #[error("Found {count} extra positional argument(s) for call to {function}")]
    ExtraPositionalArg { count: usize, function: String },
    #[error("Found `{}` extra named parameter(s) for call to {function}", .names.join("` `"))]
    ExtraNamedArg {
        names: Vec<String>,
        function: String,
    },
    #[error("Argument `{name}` occurs more than once")]
    RepeatedArg { name: String },
    #[error("The argument provided for *args is not an identifier")]
    ArgsValueIsNotString,
    #[error("The argument provided for *args is not iterable")]
    ArgsArrayIsNotIterable,
    #[error("The argument provided for **kwargs is not a dictionary")]
    KwArgsIsNotDict,
    #[error("Wrong number of positional arguments, expected {}, got {got}",
        if min == max {min.to_string()} else {format!("between {} and {}", min, max)})]
    WrongNumberOfArgs { min: usize, max: usize, got: usize },
}

impl From<FunctionError> for crate::Error {
    fn from(e: FunctionError) -> Self {
        crate::Error::new_kind(crate::ErrorKind::Function(anyhow::Error::new(e)))
    }
}

/// An object accompanying argument name for faster argument resolution.
pub(crate) trait ArgSymbol: Debug + Coerce<Self> + 'static {
    fn get_index_from_param_spec<'v, V: ValueLike<'v>>(
        &self,
        ps: &ParametersSpec<V>,
    ) -> Option<usize>;

    fn small_hash(&self) -> StarlarkHashValue;
}

impl ArgSymbol for Symbol {
    fn get_index_from_param_spec<'v, V: ValueLike<'v>>(
        &self,
        ps: &ParametersSpec<V>,
    ) -> Option<usize> {
        ps.names.get(self).map(|i| *i as usize)
    }

    fn small_hash(&self) -> StarlarkHashValue {
        self.small_hash()
    }
}

/// `Symbol` resolved to function parameter index.
#[derive(Debug)]
pub(crate) struct ResolvedArgName {
    /// Hash of the argument name.
    pub(crate) hash: StarlarkHashValue,
    /// Parameter index or `None` if the argument should go to kwargs.
    pub(crate) param_index: Option<u32>,
}

impl ArgSymbol for ResolvedArgName {
    fn get_index_from_param_spec<'v, V: ValueLike<'v>>(
        &self,
        _ps: &ParametersSpec<V>,
    ) -> Option<usize> {
        self.param_index.map(|i| i as usize)
    }

    fn small_hash(&self) -> StarlarkHashValue {
        self.hash
    }
}

unsafe impl Coerce<ResolvedArgName> for ResolvedArgName {}

#[derive(Debug, Clone_, Dupe_)]
pub(crate) struct ArgNames<'a, 'v, S: ArgSymbol> {
    /// Names are guaranteed to be unique here.
    names: &'a [(S, StringValue<'v>)],
}

impl<'a, 'v, S: ArgSymbol> Default for ArgNames<'a, 'v, S> {
    fn default() -> Self {
        Self::new_unique(&[])
    }
}

impl<'a, 'v, S: ArgSymbol> Copy for ArgNames<'a, 'v, S> {}

impl<'a, 'v, S: ArgSymbol> ArgNames<'a, 'v, S> {
    /// Names must be unique.
    /// String in `Symbol` must be equal to the `StringValue`,
    /// it is caller responsibility to ensure that.
    ///
    /// When this invariant is violated, it is memory safe,
    /// but behavior will be incorrect (errors in wrong places, missing errors, panics, etc.)
    pub(crate) fn new_unique(names: &'a [(S, StringValue<'v>)]) -> ArgNames<'a, 'v, S> {
        ArgNames { names }
    }

    pub(crate) fn new_check_unique(
        names: &'a [(S, StringValue<'v>)],
    ) -> crate::Result<ArgNames<'a, 'v, S>> {
        let mut set = SmallSet::with_capacity(names.len());
        for (s, name) in names {
            if !set.insert_hashed(Hashed::new_unchecked(s.small_hash(), name.as_str())) {
                return Err(value_error!(
                    "Argument `{}` occurs more than once",
                    name.as_str()
                ));
            }
        }
        Ok(Self::new_unique(names))
    }

    /// Unique names.
    pub(crate) fn names(&self) -> &'a [(S, StringValue<'v>)] {
        self.names
    }
}

/// Either full arguments, or short arguments for positional-only calls.
pub(crate) trait ArgumentsImpl<'v, 'a>: Debug {
    type ArgSymbol: ArgSymbol;
    fn pos(&self) -> &[Value<'v>];
    fn named(&self) -> &[Value<'v>];
    fn names(&self) -> ArgNames<'a, 'v, Self::ArgSymbol>;
    fn args(&self) -> Option<Value<'v>>;
    fn kwargs(&self) -> Option<Value<'v>>;
}

/// Arguments object is passed from the starlark interpreter to function implementation
/// when evaluation function or method calls.
#[derive(Clone_, Dupe_, Debug)]
pub(crate) struct ArgumentsFull<'v, 'a, S: ArgSymbol> {
    /// Positional arguments.
    pub(crate) pos: &'a [Value<'v>],
    /// Named arguments.
    pub(crate) named: &'a [Value<'v>],
    /// Names of named arguments.
    ///
    /// `named` length must be equal to `names` length.
    pub(crate) names: ArgNames<'a, 'v, S>,
    /// `*args` argument.
    pub(crate) args: Option<Value<'v>>,
    /// `**kwargs` argument.
    pub(crate) kwargs: Option<Value<'v>>,
}

impl<'v, 'a, S: ArgSymbol> Default for ArgumentsFull<'v, 'a, S> {
    fn default() -> Self {
        ArgumentsFull {
            pos: &[],
            named: &[],
            names: ArgNames::default(),
            args: None,
            kwargs: None,
        }
    }
}

impl<'v, 'a, S: ArgSymbol> ArgumentsImpl<'v, 'a> for ArgumentsFull<'v, 'a, S> {
    type ArgSymbol = S;

    #[inline]
    fn pos(&self) -> &[Value<'v>] {
        self.pos
    }

    #[inline]
    fn named(&self) -> &[Value<'v>] {
        self.named
    }

    #[inline]
    fn names(&self) -> ArgNames<'a, 'v, S> {
        self.names
    }

    #[inline]
    fn args(&self) -> Option<Value<'v>> {
        self.args
    }

    #[inline]
    fn kwargs(&self) -> Option<Value<'v>> {
        self.kwargs
    }
}

/// Positional-only arguments, smaller and faster than `ArgumentsFull`.
#[derive(Debug)]
pub(crate) struct ArgumentsPos<'v, 'a, S: ArgSymbol> {
    pub(crate) pos: &'a [Value<'v>],
    pub(crate) names: PhantomData<&'static S>,
}

impl<'a, 'v, S: ArgSymbol> ArgumentsImpl<'v, 'a> for ArgumentsPos<'v, 'a, S> {
    type ArgSymbol = S;

    #[inline]
    fn pos(&self) -> &[Value<'v>] {
        self.pos
    }

    #[inline]
    fn named(&self) -> &[Value<'v>] {
        &[]
    }

    #[inline]
    fn names(&self) -> ArgNames<'a, 'v, S> {
        ArgNames::default()
    }

    #[inline]
    fn args(&self) -> Option<Value<'v>> {
        None
    }

    #[inline]
    fn kwargs(&self) -> Option<Value<'v>> {
        None
    }
}

/// Arguments object is passed from the starlark interpreter to function implementation
/// when evaluation function or method calls.
#[derive(Default, Clone, Dupe_)]
pub struct Arguments<'v, 'a>(pub(crate) ArgumentsFull<'v, 'a, Symbol>);

impl<'v, 'a> Arguments<'v, 'a> {
    /// Unwrap all named arguments (both explicit and in `**kwargs`) into a map.
    ///
    /// This operation fails if named argument names are not unique.
    pub fn names_map(&self) -> crate::Result<SmallMap<StringValue<'v>, Value<'v>>> {
        match self.unpack_kwargs()? {
            None => {
                let mut result = SmallMap::with_capacity(self.0.names.names().len());
                for (k, v) in self.0.names.names().iter().zip(self.0.named) {
                    result.insert_hashed_unique_unchecked(
                        Hashed::new_unchecked(k.0.small_hash(), k.1),
                        *v,
                    );
                }
                Ok(result)
            }
            Some(kwargs) => {
                if self.0.names().names().is_empty() {
                    match kwargs.downcast_ref_key_string() {
                        Some(kwargs) => Ok(kwargs.clone()),
                        None => Err(FunctionError::ArgsValueIsNotString.into()),
                    }
                } else {
                    // We have to insert the names before the kwargs since the iteration order is observable
                    let mut result =
                        SmallMap::with_capacity(self.0.names.names().len() + kwargs.len());
                    for (k, v) in self.0.names.names().iter().zip(self.0.named) {
                        result.insert_hashed_unique_unchecked(
                            Hashed::new_unchecked(k.0.small_hash(), k.1),
                            *v,
                        );
                    }
                    for (k, v) in kwargs.iter_hashed() {
                        let s = Arguments::unpack_kwargs_key_as_value(*k.key())?;
                        let k = Hashed::new_unchecked(k.hash(), s);
                        let old = result.insert_hashed(k, v);
                        if unlikely(old.is_some()) {
                            return Err(FunctionError::RepeatedArg {
                                name: s.as_str().to_owned(),
                            }
                            .into());
                        }
                    }
                    Ok(result)
                }
            }
        }
    }

    /// The number of arguments, where those inside a args/kwargs are counted as multiple arguments.
    ///
    /// This operation fails if the `kwargs` is not a dictionary, or `args` does not support `len`.
    pub fn len(&self) -> crate::Result<usize> {
        let args = match self.0.args {
            None => 0,
            Some(v) => v.length()? as usize,
        };
        let kwargs = match self.unpack_kwargs()? {
            None => 0,
            Some(v) => v.len(),
        };
        Ok(self.0.pos.len() + self.0.named.len() + args + kwargs)
    }

    /// Unwrap all named arguments (both explicit and in `**kwargs`) into a dictionary.
    ///
    /// This operation fails if named argument names are not unique.
    pub(crate) fn names(&self) -> crate::Result<Dict<'v>> {
        Ok(Dict::new(coerce(self.names_map()?)))
    }

    /// Unpack all positional parameters into an iterator.
    pub(crate) fn positions<'b>(
        &'b self,
        heap: &'v Heap,
    ) -> crate::Result<impl Iterator<Item = Value<'v>> + 'b> {
        let tail = match self.0.args {
            None => Either::Left(iter::empty()),
            Some(args) => Either::Right(args.iterate(heap)?),
        };
        Ok(self.0.pos.iter().copied().chain(tail))
    }

    /// Examine the `kwargs` field, converting it to a [`Dict`] or failing.
    /// Note that even if this operation succeeds, the keys in the kwargs
    /// will _not_ have been validated to be strings (as they must be).
    /// The arguments may also overlap with named, which would be an error.
    #[inline(always)]
    pub(crate) fn unpack_kwargs(&self) -> crate::Result<Option<DictRef<'v>>> {
        match self.0.kwargs {
            None => Ok(None),
            Some(kwargs) => match DictRef::from_value(kwargs) {
                None => Err(FunctionError::KwArgsIsNotDict.into()),
                Some(x) => Ok(Some(x)),
            },
        }
    }

    /// Confirm that a key in the `kwargs` field is indeed a string, or [`Err`].
    #[inline(always)]
    pub(crate) fn unpack_kwargs_key_as_value(k: Value<'v>) -> crate::Result<StringValue<'v>> {
        match StringValue::new(k) {
            None => Err(FunctionError::ArgsValueIsNotString.into()),
            Some(k) => Ok(k),
        }
    }

    /// Confirm that a key in the `kwargs` field is indeed a string, or [`Err`].
    #[inline(always)]
    pub(crate) fn unpack_kwargs_key(k: Value<'v>) -> crate::Result<&'v str> {
        Arguments::unpack_kwargs_key_as_value(k).map(|k| k.as_str())
    }

    /// Produce [`Err`] if there are any positional arguments.
    #[inline(always)]
    pub fn no_positional_args(&self, heap: &'v Heap) -> crate::Result<()> {
        let [] = self.positional(heap)?;
        Ok(())
    }

    /// Produce [`Err`] if there are any named (i.e. non-positional) arguments.
    #[inline(always)]
    pub fn no_named_args(&self) -> crate::Result<()> {
        #[cold]
        #[inline(never)]
        fn bad(x: &Arguments) -> crate::Result<()> {
            // We might have a empty kwargs dictionary, but probably have an error
            let mut extra = Vec::new();
            extra.extend(x.0.names.names().iter().map(|x| x.0.as_str().to_owned()));
            if let Some(kwargs) = x.unpack_kwargs()? {
                for k in kwargs.keys() {
                    extra.push(Arguments::unpack_kwargs_key(k)?.to_owned());
                }
            }
            if extra.is_empty() {
                Ok(())
            } else {
                // Would be nice to give a better name here, but it's in the call stack, so no big deal
                Err(crate::Error::from(FunctionError::ExtraNamedArg {
                    names: extra,
                    function: "function".to_owned(),
                }))
            }
        }

        if self.0.named.is_empty() && self.0.kwargs.is_none() {
            Ok(())
        } else {
            bad(self)
        }
    }

    /// Collect exactly `N` positional arguments from the [`Arguments`], failing if there are too many/few
    /// arguments. Ignores named arguments.
    #[inline(always)]
    pub(crate) fn positional<const N: usize>(
        &self,
        heap: &'v Heap,
    ) -> crate::Result<[Value<'v>; N]> {
        let (positional, []) = self.optional::<N, 0>(heap)?;
        Ok(positional)
    }

    /// Collect exactly `REQUIRED` positional arguments, plus at most `OPTIONAL` positional arguments
    /// from the [`Arguments`], failing if there are too many/few arguments. Ignores named arguments.
    /// The `OPTIONAL` array will never have a [`Some`] after a [`None`].
    #[inline(always)]
    pub(crate) fn optional<const REQUIRED: usize, const OPTIONAL: usize>(
        &self,
        heap: &'v Heap,
    ) -> crate::Result<([Value<'v>; REQUIRED], [Option<Value<'v>>; OPTIONAL])> {
        #[cold]
        #[inline(never)]
        fn rare<'v, const REQUIRED: usize, const OPTIONAL: usize>(
            x: &Arguments<'v, '_>,
            heap: &'v Heap,
        ) -> crate::Result<([Value<'v>; REQUIRED], [Option<Value<'v>>; OPTIONAL])> {
            // Very sad that we allocate into a vector, but I expect calling into a small positional argument
            // with a *args is very rare.
            let args = match x.0.args {
                None => StarlarkIterator::empty(heap),
                Some(args) => args.iterate(heap)?,
            };
            let xs = x.0.pos.iter().copied().chain(args).collect::<Vec<_>>();
            if xs.len() >= REQUIRED && xs.len() <= REQUIRED + OPTIONAL {
                let required = xs[0..REQUIRED].try_into().unwrap();
                let mut optional = [None; OPTIONAL];
                for (a, b) in optional.iter_mut().zip(&xs[REQUIRED..]) {
                    *a = Some(*b);
                }
                Ok((required, optional))
            } else {
                Err(crate::Error::from(FunctionError::WrongNumberOfArgs {
                    min: REQUIRED,
                    max: REQUIRED + OPTIONAL,
                    got: xs.len(),
                }))
            }
        }

        if self.0.args.is_none()
            && self.0.pos.len() >= REQUIRED
            && self.0.pos.len() <= REQUIRED + OPTIONAL
        {
            let required = self.0.pos[0..REQUIRED].try_into().unwrap();
            let mut optional = [None; OPTIONAL];
            for (a, b) in optional.iter_mut().zip(&self.0.pos[REQUIRED..]) {
                *a = Some(*b);
            }
            Ok((required, optional))
        } else {
            rare(self, heap)
        }
    }

    /// Collect 1 positional arguments from the [`Arguments`], failing if there are too many/few
    /// arguments. Ignores named arguments.
    #[inline(always)]
    pub fn positional1(&self, heap: &'v Heap) -> crate::Result<Value<'v>> {
        // Could be implemented more directly, let's see if profiling shows it up
        let [x] = self.positional(heap)?;
        Ok(x)
    }

    /// Collect up to 1 optional arguments from the [`Arguments`], failing if there are too many
    /// arguments. Ignores named arguments.
    #[inline(always)]
    pub(crate) fn optional1(&self, heap: &'v Heap) -> crate::Result<Option<Value<'v>>> {
        // Could be implemented more directly, let's see if profiling shows it up
        let ([], [x]) = self.optional(heap)?;
        Ok(x)
    }
}

impl<'a> Arguments<'static, 'a> {
    /// Convert `Arguments` with `FrozenValue` (because no other values can have `'v` lifetime)
    /// to arbitrary `'v` lifetime.
    pub(crate) fn frozen_to_v<'v>(&self) -> &Arguments<'v, 'a> {
        unsafe { transmute!(&Arguments, &Arguments, self) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::const_frozen_string;
    use crate::values::StringValueLike;

    #[test]
    fn test_parameter_unpack() {
        let heap = Heap::new();
        fn f<'v, F: Fn(&Arguments<'v, '_>), const N: usize>(heap: &'v Heap, op: F) {
            for i in 0..=N {
                let mut p = Arguments::default();
                let pos = (0..i)
                    .map(|x| Value::testing_new_int(x as i32))
                    .collect::<Vec<_>>();
                let args = (i..N)
                    .map(|x| Value::testing_new_int(x as i32))
                    .collect::<Vec<_>>();
                let empty_args = args.is_empty();
                p.0.pos = &pos;
                p.0.args = Some(heap.alloc(args));
                op(&p);
                if empty_args {
                    p.0.args = None;
                    op(&p);
                }
                assert_eq!(p.len().unwrap(), N);
            }
        }

        f::<_, 0>(&heap, |p| {
            assert_eq!(&p.positional::<0>(&heap).unwrap(), &[]);
            assert!(&p.positional::<1>(&heap).is_err());
            assert!(&p.positional::<2>(&heap).is_err());
            assert_eq!(&p.optional::<0, 1>(&heap).unwrap(), &([], [None]));
            assert!(&p.optional::<1, 1>(&heap).is_err());
            assert_eq!(&p.optional::<0, 2>(&heap).unwrap(), &([], [None, None]));
        });
        f::<_, 1>(&heap, |p| {
            assert!(&p.positional::<0>(&heap).is_err());
            assert_eq!(
                &p.positional::<1>(&heap).unwrap(),
                &[Value::testing_new_int(0)]
            );
            assert!(&p.positional::<2>(&heap).is_err());
            assert_eq!(
                &p.optional::<0, 1>(&heap).unwrap(),
                &([], [Some(Value::testing_new_int(0))])
            );
            assert_eq!(
                &p.optional::<1, 1>(&heap).unwrap(),
                &([Value::testing_new_int(0)], [None])
            );
            assert_eq!(
                &p.optional::<0, 2>(&heap).unwrap(),
                &([], [Some(Value::testing_new_int(0)), None])
            );
        });
        f::<_, 2>(&heap, |p| {
            assert!(&p.positional::<0>(&heap).is_err());
            assert!(&p.positional::<1>(&heap).is_err());
            assert_eq!(
                &p.positional::<2>(&heap).unwrap(),
                &[Value::testing_new_int(0), Value::testing_new_int(1)]
            );
            assert!(p.optional::<0, 1>(&heap).is_err());
            assert_eq!(
                &p.optional::<1, 1>(&heap).unwrap(),
                &(
                    [Value::testing_new_int(0)],
                    [Some(Value::testing_new_int(1))]
                )
            );
            assert_eq!(
                &p.optional::<0, 2>(&heap).unwrap(),
                &(
                    [],
                    [
                        Some(Value::testing_new_int(0)),
                        Some(Value::testing_new_int(1))
                    ]
                )
            );
        });
        f::<_, 3>(&heap, |p| {
            assert!(&p.positional::<0>(&heap).is_err());
            assert!(&p.positional::<1>(&heap).is_err());
            assert!(&p.positional::<2>(&heap).is_err());
            assert!(p.optional::<0, 1>(&heap).is_err());
            assert!(p.optional::<1, 1>(&heap).is_err());
            assert!(p.optional::<0, 2>(&heap).is_err());
        });
    }

    #[test]
    fn test_parameter_no_named() {
        let heap = Heap::new();
        let mut p = Arguments::default();
        assert!(p.no_named_args().is_ok());
        assert_eq!(p.len().unwrap(), 0);

        // Test lots of forms of kwargs work properly
        p.0.kwargs = Some(Value::new_none());
        assert!(p.no_named_args().is_err());
        p.0.kwargs = Some(heap.alloc(Dict::default()));
        assert!(p.no_named_args().is_ok());
        assert_eq!(p.len().unwrap(), 0);
        let mut sm = SmallMap::new();
        sm.insert_hashed(heap.alloc_str("test").get_hashed(), Value::new_none());
        p.0.kwargs = Some(heap.alloc(Dict::new(coerce(sm))));
        assert!(p.no_named_args().is_err());
        assert_eq!(p.len().unwrap(), 1);

        // Test named arguments work properly
        p.0.kwargs = None;
        let named = [Value::new_none()];
        p.0.named = &named;
        let names = [(Symbol::new("test"), heap.alloc_str("test"))];
        p.0.names = ArgNames::new_check_unique(&names).unwrap();
        assert!(p.no_named_args().is_err());
        assert_eq!(p.len().unwrap(), 1);
    }

    #[test]
    fn test_names_map_repeated_name_in_arg_names() {
        let names = vec![
            (
                Symbol::new("a"),
                const_frozen_string!("a").to_string_value(),
            ),
            (
                Symbol::new("a"),
                const_frozen_string!("a").to_string_value(),
            ),
        ];
        assert!(ArgNames::new_check_unique(&names).is_err());
    }
}
