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

use std::collections::HashSet;
use std::ops::Range;

use allocative::Allocative;
use dupe::Dupe;

use crate::codemap::CodeMap;
use crate::codemap::Span;
use crate::codemap::Spanned;
use crate::eval_exception::EvalException;
use crate::syntax::ast::AstAssignIdentP;
use crate::syntax::ast::AstExprP;
use crate::syntax::ast::AstParameterP;
use crate::syntax::ast::AstPayload;
use crate::syntax::ast::AstTypeExprP;
use crate::syntax::ast::ParameterP;

#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq)]
pub enum DefRegularParamMode {
    PosOnly,
    PosOrName,
    NameOnly,
}

pub enum DefParamKind<'a, P: AstPayload> {
    Regular(
        DefRegularParamMode,
        /// Default value.
        Option<&'a AstExprP<P>>,
    ),
    Args,
    Kwargs,
}

pub struct DefParam<'a, P: AstPayload> {
    pub ident: &'a AstAssignIdentP<P>,
    pub kind: DefParamKind<'a, P>,
    pub ty: Option<&'a AstTypeExprP<P>>,
}

/// Parameters internally in starlark-rust are commonly represented as a flat list of parameters,
/// with markers `/` and `*` omitted.
/// This struct contains sizes and indices to split the list into parts.
#[derive(
    Copy, Clone, Dupe, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Allocative
)]
pub struct DefParamIndices {
    /// Number of parameters which can be filled positionally.
    /// That is, number of parameters before first `*`, `*args` or `**kwargs`.
    pub num_positional: u32,
    /// Number of parameters which can only be filled positionally.
    /// Always less or equal to `num_positional`.
    pub num_positional_only: u32,
    /// Index of `*args` parameter, if any.
    /// If present, equal to `num_positional`.
    pub args: Option<u32>,
    /// Index of `**kwargs` parameter, if any.
    /// If present, equal to the number of parameters minus 1.
    pub kwargs: Option<u32>,
}

impl DefParamIndices {
    pub fn pos_only(&self) -> Range<usize> {
        0..self.num_positional_only as usize
    }

    pub fn pos_or_named(&self) -> Range<usize> {
        self.num_positional_only as usize..self.num_positional as usize
    }

    pub fn named_only(&self, param_count: usize) -> Range<usize> {
        self.args
            .map(|a| a as usize + 1)
            .unwrap_or(self.num_positional as usize)
            ..self.kwargs.unwrap_or(param_count as u32) as usize
    }
}

/// Post-processed AST for function parameters.
///
/// * Validated
/// * `*` parameter replaced with `num_positional` field
pub struct DefParams<'a, P: AstPayload> {
    pub params: Vec<Spanned<DefParam<'a, P>>>,
    pub indices: DefParamIndices,
}

fn check_param_name<'a, P: AstPayload, T>(
    argset: &mut HashSet<&'a str>,
    n: &'a AstAssignIdentP<P>,
    arg: &Spanned<T>,
    codemap: &CodeMap,
) -> Result<(), EvalException> {
    if !argset.insert(n.node.ident.as_str()) {
        return Err(EvalException::parser_error(
            "duplicated parameter name",
            arg.span,
            codemap,
        ));
    }
    Ok(())
}

impl<'a, P: AstPayload> DefParams<'a, P> {
    pub fn unpack(
        ast_params: &'a [AstParameterP<P>],
        codemap: &CodeMap,
    ) -> Result<DefParams<'a, P>, EvalException> {
        #[derive(Ord, PartialOrd, Eq, PartialEq)]
        enum State {
            Normal,
            /// After `/`.
            SeenSlash,
            /// After `*` or `*args`.
            SeenStar,
            /// After `**kwargs`.
            SeenStarStar,
        }

        // you can't repeat argument names
        let mut argset = HashSet::new();
        // You can't have more than one *args/*, **kwargs
        // **kwargs must be last
        // You can't have a required `x` after an optional `y=1`
        let mut seen_optional = false;

        let mut params = Vec::with_capacity(ast_params.len());
        let mut num_positional = 0;
        let mut args = None;
        let mut kwargs = None;

        // Index of `*` parameter, if any.
        let mut index_of_star = None;

        let num_positional_only = match ast_params
            .iter()
            .position(|p| matches!(p.node, ParameterP::Slash))
        {
            None => 0,
            Some(0) => {
                return Err(EvalException::parser_error(
                    "`/` cannot be first parameter",
                    ast_params[0].span,
                    codemap,
                ));
            }
            Some(n) => match n.try_into() {
                Ok(n) => n,
                Err(_) => {
                    return Err(EvalException::parser_error(
                        format_args!("Too many parameters: {}", ast_params.len()),
                        Span::merge_all(ast_params.iter().map(|p| p.span)),
                        codemap,
                    ));
                }
            },
        };

        let mut state = if num_positional_only == 0 {
            State::SeenSlash
        } else {
            State::Normal
        };

        for (i, param) in ast_params.iter().enumerate() {
            let span = param.span;

            if let Some(name) = param.ident() {
                check_param_name(&mut argset, name, param, codemap)?;
            }

            match &param.node {
                ParameterP::Normal(n, ty, default_value) => {
                    if state >= State::SeenStarStar {
                        return Err(EvalException::parser_error(
                            "Parameter after kwargs",
                            param.span,
                            codemap,
                        ));
                    }
                    match default_value {
                        None => {
                            if seen_optional && state < State::SeenStar {
                                return Err(EvalException::parser_error(
                                    "positional parameter after non positional",
                                    param.span,
                                    codemap,
                                ));
                            }
                        }
                        Some(_default_value) => {
                            seen_optional = true;
                        }
                    }
                    if state < State::SeenStar {
                        num_positional += 1;
                    }
                    let mode = if state < State::SeenSlash {
                        DefRegularParamMode::PosOnly
                    } else if state < State::SeenStar {
                        DefRegularParamMode::PosOrName
                    } else {
                        DefRegularParamMode::NameOnly
                    };
                    params.push(Spanned {
                        span,
                        node: DefParam {
                            ident: n,
                            kind: DefParamKind::Regular(mode, default_value.as_deref()),
                            ty: ty.as_deref(),
                        },
                    });
                }
                ParameterP::NoArgs => {
                    if state >= State::SeenStar {
                        return Err(EvalException::parser_error(
                            "Args parameter after another args or kwargs parameter",
                            param.span,
                            codemap,
                        ));
                    }
                    state = State::SeenStar;
                    if index_of_star.is_some() {
                        return Err(EvalException::internal_error(
                            "Multiple `*` in parameters, must have been caught earlier",
                            param.span,
                            codemap,
                        ));
                    }
                    index_of_star = Some(i);
                }
                ParameterP::Slash => {
                    if state >= State::SeenSlash {
                        return Err(EvalException::parser_error(
                            "Multiple `/` in parameters",
                            param.span,
                            codemap,
                        ));
                    }
                    state = State::SeenSlash;
                }
                ParameterP::Args(n, ty) => {
                    if state >= State::SeenStar {
                        return Err(EvalException::parser_error(
                            "Args parameter after another args or kwargs parameter",
                            param.span,
                            codemap,
                        ));
                    }
                    state = State::SeenStar;
                    if args.is_some() {
                        return Err(EvalException::internal_error(
                            "Multiple *args",
                            param.span,
                            codemap,
                        ));
                    }
                    args = Some(params.len().try_into().unwrap());
                    params.push(Spanned {
                        span,
                        node: DefParam {
                            ident: n,
                            kind: DefParamKind::Args,
                            ty: ty.as_deref(),
                        },
                    });
                }
                ParameterP::KwArgs(n, ty) => {
                    if state >= State::SeenStarStar {
                        return Err(EvalException::parser_error(
                            "Multiple kwargs dictionary in parameters",
                            param.span,
                            codemap,
                        ));
                    }
                    if kwargs.is_some() {
                        return Err(EvalException::internal_error(
                            "Multiple **kwargs",
                            param.span,
                            codemap,
                        ));
                    }
                    kwargs = Some(params.len().try_into().unwrap());
                    state = State::SeenStarStar;
                    params.push(Spanned {
                        span,
                        node: DefParam {
                            ident: n,
                            kind: DefParamKind::Kwargs,
                            ty: ty.as_deref(),
                        },
                    });
                }
            }
        }

        if let Some(index_of_star) = index_of_star {
            let Some(next) = ast_params.get(index_of_star + 1) else {
                return Err(EvalException::parser_error(
                    "`*` parameter must not be last",
                    ast_params[index_of_star].span,
                    codemap,
                ));
            };
            match &next.node {
                ParameterP::Normal(..) => {}
                ParameterP::KwArgs(_, _)
                | ParameterP::Args(_, _)
                | ParameterP::NoArgs
                | ParameterP::Slash => {
                    // We get here only for `**kwargs`, the rest is handled above.
                    return Err(EvalException::parser_error(
                        "`*` must be followed by named parameter",
                        next.span,
                        codemap,
                    ));
                }
            }
        }

        Ok(DefParams {
            params,
            indices: DefParamIndices {
                num_positional: u32::try_from(num_positional).unwrap(),
                num_positional_only,
                args,
                kwargs,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::golden_test_template::golden_test_template;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;

    fn fails_dialect(test_name: &str, program: &str, dialect: &Dialect) {
        let e = AstModule::parse("test.star", program.to_owned(), dialect).unwrap_err();
        let text = format!("Program:\n{program}\n\nError: {e}\n");
        golden_test_template(&format!("src/syntax/def_tests/{test_name}.golden"), &text);
    }

    fn fails(test_name: &str, program: &str) {
        fails_dialect(test_name, program, &Dialect::AllOptionsInternal);
    }

    fn passes(program: &str) {
        AstModule::parse(
            "test.star",
            program.to_owned(),
            &Dialect::AllOptionsInternal,
        )
        .unwrap();
    }

    #[test]
    fn test_params_unpack() {
        fails("dup_name", "def test(x, y, x): pass");
        fails("pos_after_default", "def test(x=1, y): pass");
        fails("default_after_kwargs", "def test(**kwargs, y=1): pass");
        fails("args_args", "def test(*x, *y): pass");
        fails("kwargs_args", "def test(**x, *y): pass");
        fails("kwargs_kwargs", "def test(**x, **y): pass");

        passes("def test(x, y, z=1, *args, **kwargs): pass");
    }

    #[test]
    fn test_params_noargs() {
        fails("star_star", "def test(*, *): pass");
        fails("normal_after_default", "def test(x, y=1, z): pass");

        passes("def test(*args, x): pass");
        passes("def test(*args, x=1): pass");
        passes("def test(*args, x, y=1): pass");
        passes("def test(x=1, *args, y): pass");
        passes("def test(*args, x, y=1, z): pass");
        passes("def test(*, x, y=1, z): pass");
    }

    #[test]
    fn test_star_cannot_be_last() {
        fails("star_cannot_be_last", "def test(x, *): pass");
    }

    #[test]
    fn test_star_then_args() {
        fails("star_then_args", "def test(x, *, *args): pass");
    }

    #[test]
    fn test_star_then_kwargs() {
        fails("star_then_kwargs", "def test(x, *, **kwargs): pass");
    }

    #[test]
    fn test_positional_only() {
        passes("def test(x, /): pass");
    }

    #[test]
    fn test_positional_only_cannot_be_first() {
        fails("positional_only_cannot_be_first", "def test(/, x): pass");
    }

    #[test]
    fn test_slash_slash() {
        fails("slash_slash", "def test(x, /, y, /): pass");
    }

    #[test]
    fn test_named_only_in_standard_dialect_def() {
        fails_dialect(
            "named_only_in_standard_dialect_def",
            "def test(*, x): pass",
            &Dialect::Standard,
        );
    }

    #[test]
    fn test_named_only_in_standard_dialect_lambda() {
        fails_dialect(
            "named_only_in_standard_dialect_lambda",
            "lambda *, x: 17",
            &Dialect::Standard,
        );
    }

    #[test]
    fn test_positional_only_in_standard_dialect_def() {
        fails_dialect(
            "positional_only_in_standard_dialect_def",
            "def test(/, x): pass",
            &Dialect::Standard,
        );
    }

    #[test]
    fn test_positional_only_in_standard_dialect_lambda() {
        fails_dialect(
            "positional_only_in_standard_dialect_lambda",
            "lambda /, x: 17",
            &Dialect::Standard,
        );
    }
}
