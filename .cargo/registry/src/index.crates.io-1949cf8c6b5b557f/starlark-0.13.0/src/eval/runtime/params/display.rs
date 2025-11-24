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
use std::iter;

/// Parameter or `*` or `/` separator, but only if needed for formatting.
pub enum FmtParam<T> {
    /// Positional-only, positional-or-named, or named-only parameter.
    Regular(T),
    /// `*args` parameter.
    Args(T),
    /// `**kwargs` parameter.
    Kwargs(T),
    /// `/` separator.
    Slash,
    /// `*` separator.
    Star,
}

/// Flatten parameters and insert `/` and `*` separators if needed.
pub(crate) fn iter_fmt_param_spec<T>(
    pos_only: impl IntoIterator<Item = T>,
    pos_named: impl IntoIterator<Item = T>,
    args: Option<T>,
    named_only: impl IntoIterator<Item = T>,
    kwargs: Option<T>,
) -> impl Iterator<Item = FmtParam<T>> {
    let mut pos_only = pos_only.into_iter().peekable();
    let slash = match pos_only.peek().is_some() {
        true => Some(FmtParam::Slash),
        false => None,
    };

    let mut named_only = named_only.into_iter().peekable();
    // `*args`, otherwise `*` if needed.
    let args_or_star = match (named_only.peek().is_some(), args) {
        (_, Some(args)) => Some(FmtParam::Args(args)),
        (true, None) => Some(FmtParam::Star),
        (false, None) => None,
    };

    iter::empty()
        .chain(pos_only.map(FmtParam::Regular))
        .chain(slash)
        .chain(pos_named.into_iter().map(FmtParam::Regular))
        .chain(args_or_star)
        .chain(named_only.map(FmtParam::Regular))
        .chain(kwargs.map(FmtParam::Kwargs))
}

/// What to print for unknown default/optional.
pub(crate) const PARAM_FMT_OPTIONAL: &str = "...";

pub(crate) struct ParamFmt<'a, T: Display, D: Display> {
    /// Parameter name.
    pub(crate) name: &'a str,
    /// Parameter type. If `None`, it will be omitted.
    pub(crate) ty: Option<T>,
    pub(crate) default: Option<D>,
}

/// Utility to format function signature.
pub(crate) fn fmt_param_spec<'n, T: Display, D: Display>(
    f: &mut dyn fmt::Write,
    pos_only: impl IntoIterator<Item = ParamFmt<'n, T, D>>,
    pos_named: impl IntoIterator<Item = ParamFmt<'n, T, D>>,
    args: Option<ParamFmt<'n, T, D>>,
    named_only: impl IntoIterator<Item = ParamFmt<'n, T, D>>,
    kwargs: Option<ParamFmt<'n, T, D>>,
) -> fmt::Result {
    fmt_param_spec_maybe_multiline(f, None, pos_only, pos_named, args, named_only, kwargs)
}

#[allow(clippy::write_with_newline)]
pub(crate) fn fmt_param_spec_maybe_multiline<'n, T: Display, D: Display>(
    f: &mut dyn fmt::Write,
    // Single-line if `None`.
    indent: Option<&str>,
    pos_only: impl IntoIterator<Item = ParamFmt<'n, T, D>>,
    pos_named: impl IntoIterator<Item = ParamFmt<'n, T, D>>,
    args: Option<ParamFmt<'n, T, D>>,
    named_only: impl IntoIterator<Item = ParamFmt<'n, T, D>>,
    kwargs: Option<ParamFmt<'n, T, D>>,
) -> fmt::Result {
    struct Printer<'w> {
        f: &'w mut dyn fmt::Write,
    }

    impl<'w> Printer<'w> {
        fn write_param(
            &mut self,
            name: impl Display,
            ty: Option<impl Display>,
            default: Option<impl Display>,
        ) -> fmt::Result {
            write!(self.f, "{name}")?;
            if let Some(ty) = ty {
                write!(self.f, ": {ty}")?;
            }
            if let Some(default) = default {
                write!(self.f, " = {default}")?;
            }
            Ok(())
        }
    }

    let mut printer = Printer { f };

    let mut iter = iter_fmt_param_spec(pos_only, pos_named, args, named_only, kwargs).peekable();

    let not_empty = iter.peek().is_some();

    for (i, param) in iter.enumerate() {
        if i == 0 {
            if let Some(indent) = indent {
                write!(printer.f, "{indent}")?;
            }
        } else {
            if let Some(indent) = indent {
                write!(printer.f, ",\n{indent}")?;
            } else {
                write!(printer.f, ", ")?;
            }
        }
        match param {
            FmtParam::Regular(p) => {
                printer.write_param(p.name, p.ty, p.default)?;
            }
            FmtParam::Args(p) => {
                printer.write_param(format_args!("*{}", p.name), p.ty, p.default)?;
            }
            FmtParam::Kwargs(p) => {
                printer.write_param(format_args!("**{}", p.name), p.ty, p.default)?;
            }
            FmtParam::Slash => {
                write!(printer.f, "/")?;
            }
            FmtParam::Star => {
                write!(printer.f, "*")?;
            }
        }
    }

    if not_empty && indent.is_some() {
        write!(printer.f, ",\n")?;
    }

    Ok(())
}
