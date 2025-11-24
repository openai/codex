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

use crate::codemap::CodeMap;
use crate::codemap::Span;
use crate::eval_exception::EvalException;
use crate::syntax::ast::ArgumentP;
use crate::syntax::ast::AstArgumentP;
use crate::syntax::ast::AstPayload;
use crate::syntax::ast::CallArgsP;

/// Validated call arguments.
pub struct CallArgsUnpack<'a, P: AstPayload> {
    pub pos: &'a [AstArgumentP<P>],
    pub named: &'a [AstArgumentP<P>],
    pub star: Option<&'a AstArgumentP<P>>,
    pub star_star: Option<&'a AstArgumentP<P>>,
}

#[derive(Eq, PartialEq, PartialOrd, Ord)]
enum ArgsStage {
    Positional,
    Named,
    Args,
    Kwargs,
}

impl<'a, P: AstPayload> CallArgsUnpack<'a, P> {
    pub fn unpack(args: &'a CallArgsP<P>, codemap: &CodeMap) -> Result<Self, EvalException> {
        let err = |span, msg: &str| Err(EvalException::parser_error(msg, span, codemap));

        let args = &args.args;

        let mut stage = ArgsStage::Positional;
        let mut named_args = HashSet::new();
        let mut num_pos = 0;
        let mut num_named = 0;
        let mut star = None;
        let mut star_star = None;
        for arg in args {
            match &arg.node {
                ArgumentP::Positional(_) => {
                    if stage != ArgsStage::Positional {
                        return err(arg.span, "positional argument after non positional");
                    } else {
                        num_pos += 1;
                    }
                }
                ArgumentP::Named(n, _) => {
                    if stage > ArgsStage::Named {
                        return err(arg.span, "named argument after *args or **kwargs");
                    } else if !named_args.insert(&n.node) {
                        // Check the names are distinct
                        return err(n.span, "repeated named argument");
                    } else {
                        stage = ArgsStage::Named;
                        num_named += 1;
                    }
                }
                ArgumentP::Args(_) => {
                    if stage > ArgsStage::Named {
                        return err(arg.span, "Args array after another args or kwargs");
                    } else {
                        stage = ArgsStage::Args;
                        if star.is_some() {
                            return Err(EvalException::internal_error(
                                "Multiple *args in arguments",
                                arg.span,
                                codemap,
                            ));
                        }
                        star = Some(arg);
                    }
                }
                ArgumentP::KwArgs(_) => {
                    if stage == ArgsStage::Kwargs {
                        return err(arg.span, "Multiple kwargs dictionary in arguments");
                    } else {
                        stage = ArgsStage::Kwargs;
                        if star_star.is_some() {
                            return Err(EvalException::internal_error(
                                "Multiple **kwargs in arguments",
                                arg.span,
                                codemap,
                            ));
                        }
                        star_star = Some(arg);
                    }
                }
            }
        }

        if num_pos + num_named + (star.is_some() as usize) + (star_star.is_some() as usize)
            != args.len()
        {
            return Err(EvalException::internal_error(
                "Argument count mismatch",
                Span::merge_all(args.iter().map(|x| x.span)),
                codemap,
            ));
        }

        Ok(CallArgsUnpack {
            pos: &args[..num_pos],
            named: &args[num_pos..num_pos + num_named],
            star,
            star_star,
        })
    }
}
