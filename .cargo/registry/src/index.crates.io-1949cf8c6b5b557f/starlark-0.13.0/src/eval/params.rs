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

use crate::eval::ParametersSpec;
use crate::eval::ParametersSpecParam;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::util::ArcStr;

/// Build both [`ParametersSpec`] (for parsing) and [`ParamSpec`] (for typechecking)
/// from a list of parameters.
pub fn param_specs<'a, V: Copy>(
    function_name: &str,
    pos_only: impl IntoIterator<Item = (&'a str, ParametersSpecParam<V>, Ty)>,
    pos_or_named: impl IntoIterator<Item = (&'a str, ParametersSpecParam<V>, Ty)>,
    args: Option<Ty>,
    named_only: impl IntoIterator<Item = (&'a str, ParametersSpecParam<V>, Ty)>,
    kwargs: Option<Ty>,
) -> crate::Result<(ParametersSpec<V>, ParamSpec)> {
    let pos_only = Vec::from_iter(pos_only);
    let pos_or_named = Vec::from_iter(pos_or_named);
    let named_only = Vec::from_iter(named_only);

    let parameters_spec = ParametersSpec::new_parts(
        function_name,
        pos_only.iter().map(|(name, param, _ty)| (*name, *param)),
        pos_or_named
            .iter()
            .map(|(name, param, _ty)| (*name, *param)),
        args.is_some(),
        named_only.iter().map(|(name, param, _ty)| (*name, *param)),
        kwargs.is_some(),
    );

    let param_spec = ParamSpec::new_parts(
        pos_only
            .into_iter()
            .map(|(_name, param, ty)| (param.is_required(), ty)),
        pos_or_named
            .into_iter()
            .map(|(name, param, ty)| (ArcStr::from(name), param.is_required(), ty)),
        args,
        named_only
            .into_iter()
            .map(|(name, param, ty)| (ArcStr::from(name), param.is_required(), ty)),
        kwargs,
    )?;

    Ok((parameters_spec, param_spec))
}
