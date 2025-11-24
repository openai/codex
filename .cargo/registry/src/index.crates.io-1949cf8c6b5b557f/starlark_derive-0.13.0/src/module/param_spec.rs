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

use crate::module::typ::StarArg;
use crate::module::typ::StarArgPassStyle;

/// Function parameters validated and split into modes.
pub(crate) struct ParamSpec<'a> {
    pub(crate) pos_only: Vec<&'a StarArg>,
    pub(crate) pos_or_named: Vec<&'a StarArg>,
    pub(crate) args: Option<&'a StarArg>,
    pub(crate) named_only: Vec<&'a StarArg>,
    pub(crate) kwargs: Option<&'a StarArg>,
}

impl<'a> ParamSpec<'a> {
    pub(crate) fn split(star_args: &'a [StarArg]) -> syn::Result<ParamSpec<'a>> {
        #[derive(PartialEq, Eq, PartialOrd, Ord)]
        enum CurrentParamStyle {
            PosOnly,
            PosOrNamed,
            NamedOnly,
            NoMore,
        }

        let mut seen_optional = false;

        let mut pos_only = Vec::new();
        let mut pos_or_named = Vec::new();
        let mut args = None;
        let mut named_only = Vec::new();
        let mut kwargs = None;

        let mut last_param_style = CurrentParamStyle::PosOnly;
        for arg in star_args {
            match arg.pass_style {
                StarArgPassStyle::Args => {
                    if last_param_style >= CurrentParamStyle::NamedOnly {
                        return Err(syn::Error::new(
                            arg.span,
                            "`args` cannot follow named-only parameters",
                        ));
                    }
                    if args.is_some() {
                        return Err(syn::Error::new(
                            arg.span,
                            "Cannot have more than one `args` parameter (internal error)",
                        ));
                    }
                    args = Some(arg);
                    last_param_style = CurrentParamStyle::NamedOnly;
                }
                StarArgPassStyle::Kwargs => {
                    if last_param_style == CurrentParamStyle::NoMore {
                        return Err(syn::Error::new(
                            arg.span,
                            "Cannot have more than one `kwargs` parameter",
                        ));
                    }
                    if kwargs.is_some() {
                        return Err(syn::Error::new(
                            arg.span,
                            "Cannot have more than one `kwargs` parameter (internal error)",
                        ));
                    }
                    kwargs = Some(arg);
                    last_param_style = CurrentParamStyle::NoMore;
                }
                StarArgPassStyle::PosOnly => {
                    if last_param_style > CurrentParamStyle::PosOnly {
                        return Err(syn::Error::new(
                            arg.span,
                            "Positional-only parameter after non-positional-only",
                        ));
                    }
                    last_param_style = CurrentParamStyle::PosOnly;
                    pos_only.push(arg);
                }
                StarArgPassStyle::PosOrNamed => {
                    if last_param_style > CurrentParamStyle::PosOrNamed {
                        return Err(syn::Error::new(
                            arg.span,
                            "Positional-or-named parameter after named-only",
                        ));
                    }
                    last_param_style = CurrentParamStyle::PosOrNamed;
                    pos_or_named.push(arg);
                }
                StarArgPassStyle::NamedOnly => {
                    if last_param_style > CurrentParamStyle::NamedOnly {
                        return Err(syn::Error::new(
                            arg.span,
                            "Named-only parameter cannot follow kwargs",
                        ));
                    }
                    named_only.push(arg);
                    last_param_style = CurrentParamStyle::NamedOnly;
                }
            }

            let optional = arg.default.is_some() || arg.is_option();

            if last_param_style <= CurrentParamStyle::PosOrNamed {
                if seen_optional && !optional {
                    return Err(syn::Error::new(
                        arg.span,
                        "Positional parameter without default after optional parameter",
                    ));
                }
            }

            seen_optional |= optional;
        }
        Ok(ParamSpec {
            pos_only,
            pos_or_named,
            args,
            named_only,
            kwargs,
        })
    }
}
