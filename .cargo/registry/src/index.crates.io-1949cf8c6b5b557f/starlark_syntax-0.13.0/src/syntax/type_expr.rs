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

use crate::codemap::CodeMap;
use crate::codemap::Spanned;
use crate::diagnostic::WithDiagnostic;
use crate::slice_vec_ext::SliceExt;
use crate::syntax::ast::AstExprP;
use crate::syntax::ast::AstIdentP;
use crate::syntax::ast::AstLiteral;
use crate::syntax::ast::AstPayload;
use crate::syntax::ast::BinOp;
use crate::syntax::ast::ExprP;

#[derive(Debug, thiserror::Error)]
pub enum TypeExprUnpackError {
    #[error("{0} expression is not allowed in type expression")]
    InvalidType(&'static str),
    #[error("Empty list is not allowed in type expression")]
    EmptyListInType,
    #[error("Only dot expression of form `ident.ident` is allowed in type expression")]
    DotInType,
    #[error("Expecting path like `a.b.c`")]
    ExpectingPath,
    #[error(r#"`{0}.type` is not allowed in type expression, use `{0}` instead"#)]
    DotTypeBan(String),
}

impl From<TypeExprUnpackError> for crate::Error {
    fn from(e: TypeExprUnpackError) -> Self {
        crate::Error::new_other(e)
    }
}

/// Types that are `""` or start with `"_"` are wildcard - they match everything
/// (also deprecated).
pub fn type_str_literal_is_wildcard(s: &str) -> bool {
    s == "" || s.starts_with('_')
}

/// Path component of type.
#[derive(Debug)]
pub struct TypePathP<'a, P: AstPayload> {
    pub first: &'a AstIdentP<P>,
    pub rem: Vec<Spanned<&'a str>>,
}

/// This type should be used instead of `TypeExprP`, but a lot of code needs to be updated.
#[derive(Debug)]
pub enum TypeExprUnpackP<'a, P: AstPayload> {
    Ellipsis,
    Path(TypePathP<'a, P>),
    /// `list[str]`.
    Index(&'a AstIdentP<P>, Box<Spanned<TypeExprUnpackP<'a, P>>>),
    /// `dict[str, int]` or `typing.Callable[[int], str]`.
    Index2(
        Spanned<TypePathP<'a, P>>,
        Box<Spanned<TypeExprUnpackP<'a, P>>>,
        Box<Spanned<TypeExprUnpackP<'a, P>>>,
    ),
    /// List argument in `typing.Callable[[int], str]`.
    List(Vec<Spanned<TypeExprUnpackP<'a, P>>>),
    Union(Vec<Spanned<TypeExprUnpackP<'a, P>>>),
    Tuple(Vec<Spanned<TypeExprUnpackP<'a, P>>>),
}

impl<'a, P: AstPayload> TypeExprUnpackP<'a, P> {
    fn unpack_path(
        expr: &'a AstExprP<P>,
        codemap: &CodeMap,
    ) -> Result<Spanned<TypePathP<'a, P>>, WithDiagnostic<TypeExprUnpackError>> {
        let span = expr.span;
        match &expr.node {
            ExprP::Identifier(ident) => Ok(Spanned {
                span,
                node: TypePathP {
                    first: ident,
                    rem: Vec::new(),
                },
            }),
            ExprP::Dot(object, field) => {
                let mut current: &AstExprP<P> = object;
                let mut rem: Vec<Spanned<_>> = vec![field.as_ref().map(|x| x.as_str())];
                loop {
                    match &current.node {
                        ExprP::Dot(o, f) => {
                            current = o;
                            rem.push(f.as_ref().map(|x| x.as_str()));
                        }
                        ExprP::Identifier(i) => {
                            rem.reverse();
                            if let Some((last, but_last)) = rem.split_last() {
                                if last.node == "type" {
                                    let mut full_path = i.node.ident.clone();
                                    for elem in but_last {
                                        full_path.push_str(&format!(".{}", elem.node));
                                    }
                                    // TODO(nga): allow it after we prohibit
                                    //   string constants as types.
                                    return Err(WithDiagnostic::new_spanned(
                                        TypeExprUnpackError::DotTypeBan(full_path),
                                        current.span,
                                        codemap,
                                    ));
                                }
                            }
                            return Ok(Spanned {
                                span,
                                node: TypePathP { first: i, rem },
                            });
                        }
                        _ => {
                            return Err(WithDiagnostic::new_spanned(
                                TypeExprUnpackError::DotInType,
                                current.span,
                                codemap,
                            ));
                        }
                    }
                }
            }
            _ => Err(WithDiagnostic::new_spanned(
                TypeExprUnpackError::ExpectingPath,
                expr.span,
                codemap,
            )),
        }
    }

    fn unpack_argument(
        expr: &'a AstExprP<P>,
        codemap: &CodeMap,
    ) -> Result<Spanned<TypeExprUnpackP<'a, P>>, WithDiagnostic<TypeExprUnpackError>> {
        let span = expr.span;
        match &expr.node {
            ExprP::List(items) => {
                let items = items.try_map(|x| TypeExprUnpackP::unpack_argument(x, codemap))?;
                Ok(Spanned {
                    span,
                    node: TypeExprUnpackP::List(items),
                })
            }
            _ => TypeExprUnpackP::unpack(expr, codemap),
        }
    }

    pub fn unpack(
        expr: &'a AstExprP<P>,
        codemap: &CodeMap,
    ) -> Result<Spanned<TypeExprUnpackP<'a, P>>, WithDiagnostic<TypeExprUnpackError>> {
        let span = expr.span;
        let err = |t| {
            Err(WithDiagnostic::new_spanned(
                TypeExprUnpackError::InvalidType(t),
                expr.span,
                codemap,
            ))
        };

        match &expr.node {
            ExprP::Tuple(xs) => {
                let xs = xs.try_map(|x| TypeExprUnpackP::unpack(x, codemap))?;
                Ok(Spanned {
                    span,
                    node: TypeExprUnpackP::Tuple(xs),
                })
            }
            ExprP::Dot(..) => {
                let path = Self::unpack_path(expr, codemap)?;
                Ok(Spanned {
                    span,
                    node: TypeExprUnpackP::Path(path.node),
                })
            }
            ExprP::Call(..) => err("call"),
            ExprP::Index(a_i) => {
                let (a, i) = &**a_i;
                match &a.node {
                    ExprP::Identifier(ident) => {
                        let i = TypeExprUnpackP::unpack(i, codemap)?;
                        Ok(Spanned {
                            span,
                            node: TypeExprUnpackP::Index(ident, Box::new(i)),
                        })
                    }
                    _ => err("array indirection where array is not an identifier"),
                }
            }
            ExprP::Index2(a_i0_i1) => {
                let (a, i0, i1) = &**a_i0_i1;
                let path = Self::unpack_path(a, codemap)?;
                let i0 = TypeExprUnpackP::unpack_argument(i0, codemap)?;
                let i1 = TypeExprUnpackP::unpack_argument(i1, codemap)?;
                Ok(Spanned {
                    span,
                    node: TypeExprUnpackP::Index2(path, Box::new(i0), Box::new(i1)),
                })
            }
            ExprP::Slice(..) => err("slice"),
            ExprP::Identifier(..) => {
                let path = Self::unpack_path(expr, codemap)?;
                Ok(Spanned {
                    span,
                    node: TypeExprUnpackP::Path(path.node),
                })
            }
            ExprP::Lambda(..) => err("lambda"),
            ExprP::Literal(AstLiteral::String(_)) => {
                // TODO(nga): eventually this should be allowed for self-referential types:
                //   https://www.internalfb.com/tasks/?t=184482361
                err("string literal")
            }
            ExprP::Literal(AstLiteral::Int(_)) => err("int"),
            ExprP::Literal(AstLiteral::Float(_)) => err("float"),
            ExprP::Literal(AstLiteral::Ellipsis) => Ok(Spanned {
                span,
                node: TypeExprUnpackP::Ellipsis,
            }),
            ExprP::Not(..) => err("not"),
            ExprP::Minus(..) => err("minus"),
            ExprP::Plus(..) => err("plus"),
            ExprP::BitNot(..) => err("bit not"),
            ExprP::Op(a, BinOp::BitOr, b) => {
                let a = TypeExprUnpackP::unpack(a, codemap)?;
                let b = TypeExprUnpackP::unpack(b, codemap)?;
                Ok(Spanned {
                    span,
                    node: TypeExprUnpackP::Union(vec![a, b]),
                })
            }
            ExprP::Op(..) => err("bin op except `|`"),
            ExprP::If(..) => err("if"),
            ExprP::List(xs) => {
                if xs.is_empty() {
                    Err(WithDiagnostic::new_spanned(
                        TypeExprUnpackError::EmptyListInType,
                        expr.span,
                        codemap,
                    ))
                } else if xs.len() == 1 {
                    err("list of 1 element")
                } else {
                    let xs = xs.try_map(|x| TypeExprUnpackP::unpack(x, codemap))?;
                    Ok(Spanned {
                        span,
                        node: TypeExprUnpackP::Union(xs),
                    })
                }
            }
            ExprP::Dict(..) => err("dict"),
            ExprP::ListComprehension(..) => err("list comprehension"),
            ExprP::DictComprehension(..) => err("dict comprehension"),
            ExprP::FString(..) => err("f-string"),
        }
    }
}
