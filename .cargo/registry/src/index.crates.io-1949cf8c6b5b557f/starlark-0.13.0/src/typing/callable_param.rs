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
use std::fmt::Formatter;
use std::iter;

use allocative::Allocative;
use dupe::Dupe;
use starlark_map::small_set::SmallSet;
use starlark_syntax::other_error;
use starlark_syntax::syntax::def::DefParamIndices;

use crate::eval::runtime::params::display::fmt_param_spec;
use crate::eval::runtime::params::display::ParamFmt;
use crate::eval::runtime::params::display::PARAM_FMT_OPTIONAL;
use crate::typing::small_arc_vec_or_static::SmallArcVec1OrStatic;
use crate::typing::ty::TyDisplay;
use crate::typing::ty::TypeRenderConfig;
use crate::typing::Ty;
use crate::util::arc_str::ArcStr;

/// Indication whether parameter is required.
#[derive(
    Debug, Clone, Dupe, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Allocative
)]
pub enum ParamIsRequired {
    /// Parameter is required.
    Yes,
    /// Parameter is optional.
    No,
}

/// The type of a parameter - can be positional, by name, `*args` or `**kwargs`.
#[derive(Debug, Clone, Dupe, PartialEq, Eq, Hash, PartialOrd, Ord, Allocative)]
pub(crate) enum ParamMode {
    /// Parameter can only be passed by position.
    PosOnly(ParamIsRequired),
    /// Parameter can be passed by position or name.
    PosOrName(ArcStr, ParamIsRequired),
    /// Parameter can only be passed by name.
    NameOnly(ArcStr, ParamIsRequired),
    /// Parameter is `*args`.
    Args,
    /// Parameter is `**kwargs`.
    Kwargs,
}

/// A parameter argument to a function
#[derive(Debug, Clone, Dupe, PartialEq, Eq, Hash, PartialOrd, Ord, Allocative)]
pub(crate) struct Param {
    /// The type of parameter
    pub(crate) mode: ParamMode,
    /// The type of the parameter.
    /// For `*args` it is the type of the tuple elements.
    /// For `**kwargs` it is the type of the dict values.
    pub(crate) ty: Ty,
}

impl Param {
    /// Create a `*args` parameter.
    ///
    /// `ty` is a tuple item type.
    pub const fn args(ty: Ty) -> Self {
        Self {
            mode: ParamMode::Args,
            ty,
        }
    }

    /// Create a `**kwargs` parameter.
    ///
    /// `ty` is a dict value type.
    pub const fn kwargs(ty: Ty) -> Self {
        Self {
            mode: ParamMode::Kwargs,
            ty,
        }
    }

    pub(crate) fn allows_pos(&self) -> bool {
        match self.mode {
            ParamMode::PosOnly(_) | ParamMode::PosOrName(_, _) | ParamMode::Args => true,
            ParamMode::NameOnly(_, _) | ParamMode::Kwargs => false,
        }
    }

    pub(crate) fn name(&self) -> Option<&str> {
        match &self.mode {
            ParamMode::PosOnly(_) => None,
            ParamMode::PosOrName(x, _) => Some(x.as_str()),
            ParamMode::NameOnly(x, _) => Some(x.as_str()),
            ParamMode::Args => None,
            ParamMode::Kwargs => None,
        }
    }

    /// Get a display name for this parameter.
    pub(crate) fn name_display(&self) -> &str {
        match &self.mode {
            ParamMode::PosOnly(_) => "_",
            ParamMode::PosOrName(x, _) => x,
            ParamMode::NameOnly(x, _) => x,
            ParamMode::Args => "*args",
            ParamMode::Kwargs => "**kwargs",
        }
    }
}

struct ParamSpecSplit<'a> {
    pos_only: &'a [Param],
    pos_or_named: &'a [Param],
    args: Option<&'a Param>,
    named_only: &'a [Param],
    kwargs: Option<&'a Param>,
}

/// Callable parameter specification (e.g. positional only followed by `**kwargs`).
#[derive(Debug, Eq, PartialEq, Clone, Dupe, Hash, PartialOrd, Ord, Allocative)]
pub struct ParamSpec {
    params: SmallArcVec1OrStatic<Param>,
    indices: DefParamIndices,
}

impl Display for ParamSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt_with_config(f, &TypeRenderConfig::Default)
    }
}

impl ParamSpec {
    pub(crate) fn fmt_with_config(
        &self,
        f: &mut Formatter<'_>,
        config: &TypeRenderConfig,
    ) -> fmt::Result {
        fn pf<'a>(
            p: &'a Param,
            config: &'a TypeRenderConfig,
        ) -> ParamFmt<'a, TyDisplay<'a>, &'static str> {
            ParamFmt {
                name: match &p.mode {
                    ParamMode::PosOrName(name, _) | ParamMode::NameOnly(name, _) => name.as_str(),
                    ParamMode::PosOnly(_) => "_",
                    ParamMode::Args => "args",
                    ParamMode::Kwargs => "kwargs",
                },
                ty: Some(p.ty.display_with(config)),
                default: match p.mode {
                    ParamMode::PosOnly(ParamIsRequired::Yes)
                    | ParamMode::PosOrName(_, ParamIsRequired::Yes)
                    | ParamMode::NameOnly(_, ParamIsRequired::Yes) => None,
                    ParamMode::PosOnly(ParamIsRequired::No)
                    | ParamMode::PosOrName(_, ParamIsRequired::No)
                    | ParamMode::NameOnly(_, ParamIsRequired::No) => Some(PARAM_FMT_OPTIONAL),
                    ParamMode::Args | ParamMode::Kwargs => None,
                },
            }
        }

        let ParamSpecSplit {
            pos_only,
            pos_or_named,
            args,
            named_only,
            kwargs,
        } = self.split();

        fmt_param_spec(
            f,
            pos_only.iter().map(|p| pf(p, config)),
            pos_or_named.iter().map(|p| pf(p, config)),
            args.map(|p| pf(p, config)),
            named_only.iter().map(|p| pf(p, config)),
            kwargs.map(|p| pf(p, config)),
        )
    }

    pub(crate) fn display_with<'a>(&'a self, config: &'a TypeRenderConfig) -> ParamSpecDisplay<'a> {
        ParamSpecDisplay {
            param_spec: self,
            config,
        }
    }

    pub(crate) fn params(&self) -> &[Param] {
        &self.params
    }

    /// Create a new parameter specification from different parameter kinds in order.
    pub fn new_parts(
        pos_only: impl IntoIterator<Item = (ParamIsRequired, Ty)>,
        pos_or_name: impl IntoIterator<Item = (ArcStr, ParamIsRequired, Ty)>,
        args: Option<Ty>,
        named_only: impl IntoIterator<Item = (ArcStr, ParamIsRequired, Ty)>,
        kwargs: Option<Ty>,
    ) -> crate::Result<ParamSpec> {
        let pos_only = pos_only.into_iter();
        let pos_or_name = pos_or_name.into_iter();
        let named_only = named_only.into_iter();

        let mut seen_names: SmallSet<ArcStr> = SmallSet::new();

        let mut params = Vec::with_capacity(
            pos_only.size_hint().0
                + pos_or_name.size_hint().0
                + args.is_some() as usize
                + named_only.size_hint().0
                + kwargs.is_some() as usize,
        );

        for (req, ty) in pos_only {
            params.push(Param {
                mode: ParamMode::PosOnly(req),
                ty,
            });
        }

        let num_positional_only = params.len() as u32;

        for (name, req, ty) in pos_or_name {
            if !seen_names.insert(name.dupe()) {
                return Err(other_error!("duplicate parameter name: `{}`", name));
            }
            params.push(Param {
                mode: ParamMode::PosOrName(name, req),
                ty,
            });
        }

        let num_positional = params.len() as u32;

        let mut index_of_args = None;
        if let Some(ty) = args {
            index_of_args = Some(params.len() as u32);
            params.push(Param {
                mode: ParamMode::Args,
                ty,
            });
        }

        for (name, req, ty) in named_only {
            if !seen_names.insert(name.dupe()) {
                return Err(other_error!("duplicate parameter name: `{}`", name));
            }
            params.push(Param {
                mode: ParamMode::NameOnly(name, req),
                ty,
            });
        }

        let mut index_of_kwargs = None;
        if let Some(ty) = kwargs {
            index_of_kwargs = Some(params.len() as u32);
            params.push(Param {
                mode: ParamMode::Kwargs,
                ty,
            });
        }

        Ok(ParamSpec {
            params: SmallArcVec1OrStatic::clone_from_slice(&params),
            indices: DefParamIndices {
                num_positional,
                num_positional_only,
                args: index_of_args,
                kwargs: index_of_kwargs,
            },
        })
    }

    /// `*, x, y`.
    pub fn new_named_only(
        named_only: impl IntoIterator<Item = (ArcStr, ParamIsRequired, Ty)>,
    ) -> crate::Result<ParamSpec> {
        Self::new_parts([], [], None, named_only, None)
    }

    /// `*args`.
    pub(crate) fn args(ty: Ty) -> ParamSpec {
        ParamSpec::new_parts([], [], Some(ty), [], None).expect("Cannot fail")
    }

    /// `**kwargs`.
    pub fn kwargs(ty: Ty) -> ParamSpec {
        ParamSpec::new_parts([], [], None, [], Some(ty)).expect("Cannot fail")
    }

    /// `arg=, arg=, ..., arg, arg, ..., /`.
    pub(crate) fn pos_only(
        required: impl IntoIterator<Item = Ty>,
        optional: impl IntoIterator<Item = Ty>,
    ) -> ParamSpec {
        ParamSpec::new_parts(
            iter::empty()
                .chain(required.into_iter().map(|ty| (ParamIsRequired::Yes, ty)))
                .chain(optional.into_iter().map(|ty| (ParamIsRequired::No, ty))),
            [],
            None,
            [],
            None,
        )
        .expect("Cannot fail")
    }

    /// No parameters.
    pub fn empty() -> ParamSpec {
        ParamSpec::pos_only([], [])
    }

    pub(crate) fn any() -> ParamSpec {
        static ANY_PARAMS: [Param; 2] = [Param::args(Ty::any()), Param::kwargs(Ty::any())];
        ParamSpec {
            params: SmallArcVec1OrStatic::new_static(&ANY_PARAMS),
            indices: DefParamIndices {
                num_positional: 0,
                num_positional_only: 0,
                args: Some(0),
                kwargs: Some(1),
            },
        }
    }

    /// Is `*args, **kwargs`.
    pub(crate) fn is_any(&self) -> bool {
        self == &Self::any()
    }

    fn split(&self) -> ParamSpecSplit<'_> {
        ParamSpecSplit {
            pos_only: &self.params[self.indices.pos_only()],
            pos_or_named: &self.params[self.indices.pos_or_named()],
            args: self.indices.args.map(|a| &self.params[a as usize]),
            named_only: &self.params[self.indices.named_only(self.params.len())],
            kwargs: self.indices.kwargs.map(|a| &self.params[a as usize]),
        }
    }

    /// All parameters are required and positional only.
    pub(crate) fn all_required_pos_only(&self) -> Option<Vec<&Ty>> {
        let (pos_only, named_only) = self.all_required_pos_only_named_only()?;
        if named_only.is_empty() {
            Some(pos_only)
        } else {
            None
        }
    }

    /// All parameters are required and positional only or named only.
    pub(crate) fn all_required_pos_only_named_only(&self) -> Option<(Vec<&Ty>, Vec<(&str, &Ty)>)> {
        match self.split() {
            ParamSpecSplit {
                pos_only,
                pos_or_named: [],
                args: None,
                named_only,
                kwargs: None,
            } => {
                let pos_only: Vec<&Ty> = pos_only
                    .iter()
                    .map(|p| match p.mode {
                        ParamMode::PosOnly(ParamIsRequired::Yes) => Some(&p.ty),
                        _ => None,
                    })
                    .collect::<Option<_>>()?;
                let named_only: Vec<(&str, &Ty)> = named_only
                    .iter()
                    .map(|p| match &p.mode {
                        ParamMode::NameOnly(name, ParamIsRequired::Yes) => {
                            Some((name.as_str(), &p.ty))
                        }
                        _ => None,
                    })
                    .collect::<Option<_>>()?;
                Some((pos_only, named_only))
            }
            _ => None,
        }
    }
}

pub(crate) struct ParamSpecDisplay<'a> {
    param_spec: &'a ParamSpec,
    config: &'a TypeRenderConfig,
}

impl Display for ParamSpecDisplay<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.param_spec.fmt_with_config(f, self.config)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fmt::Write;

    use starlark_syntax::dialect::Dialect;
    use starlark_syntax::golden_test_template::golden_test_template;
    use starlark_syntax::syntax::AstModule;

    use crate::environment::Globals;
    use crate::typing::AstModuleTypecheck;

    #[test]
    fn test_param_spec_display() {
        let functions = r#"
def simple(x, y, z): pass
def default_value(x, y=1, z=2): pass
def param_type(x: int, y: str, z: int, w: list): pass
def named_only_a(x, *, y): pass
def named_only_b(*, y): pass
def pos_only_a(x, /, y): pass
def pos_only_b(x, /, *, y): pass
def pos_only_c(x, /, *args): pass
def pos_only_d(x, /, *args, **kwargs): pass
"#;
        let mut out = String::new();
        let mut first = true;

        for test in functions.lines() {
            let test = test.trim();
            if test.is_empty() {
                continue;
            }

            let ast = AstModule::parse(
                "test_param_spec_display.star",
                test.to_owned(),
                &Dialect::AllOptionsInternal,
            )
            .unwrap();
            let (errors, typemap, _interface, approximations) =
                ast.typecheck(&Globals::standard(), &HashMap::new());
            if let Some(error) = errors.into_iter().next() {
                panic!("Error: {:?}", error);
            }
            assert!(approximations.is_empty());
            let def = typemap.find_first_binding().unwrap();

            if first {
                first = false;
            } else {
                writeln!(out).unwrap();
            }
            write!(out, "{test}\n{def}\n").unwrap();
        }

        golden_test_template(
            "src/typing/callable_param_test_param_spec_display.golden",
            &out,
        );
    }
}
