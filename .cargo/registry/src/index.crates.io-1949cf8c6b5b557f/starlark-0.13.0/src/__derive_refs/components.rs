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

use dupe::Dupe;

use crate::__derive_refs::param_spec::NativeCallableParam;
use crate::__derive_refs::param_spec::NativeCallableParamDefaultValue;
use crate::__derive_refs::param_spec::NativeCallableParamSpec;
use crate::docs::DocFunction;
use crate::docs::DocItem;
use crate::docs::DocMember;
use crate::docs::DocParam;
use crate::docs::DocParams;
use crate::docs::DocStringKind;
use crate::docs::DocType;
use crate::eval::runtime::params::display::PARAM_FMT_OPTIONAL;
use crate::typing::Ty;

/// A wrapper for the parameters to `GlobalsBuilder::set_function` and `MethodBuilder::set_method`
pub struct NativeCallableComponents {
    pub speculative_exec_safe: bool,
    pub rust_docstring: Option<&'static str>,
    pub param_spec: NativeCallableParamSpec,
    pub return_type: Ty,
}

impl NativeCallableComponents {
    fn doc_params(&self) -> DocParams {
        fn doc_param(p: &NativeCallableParam) -> DocParam {
            let NativeCallableParam { name, ty, required } = p;
            DocParam {
                name: (*name).to_owned(),
                docs: None,
                typ: ty.dupe(),
                default_value: match required {
                    None => None,
                    Some(NativeCallableParamDefaultValue::Optional) => {
                        Some(PARAM_FMT_OPTIONAL.to_owned())
                    }
                    Some(NativeCallableParamDefaultValue::Value(v)) => Some(v.to_value().to_repr()),
                },
            }
        }

        DocParams {
            pos_only: self.param_spec.pos_only.iter().map(doc_param).collect(),
            pos_or_named: self.param_spec.pos_or_named.iter().map(doc_param).collect(),
            args: self.param_spec.args.as_ref().map(doc_param),
            named_only: self.param_spec.named_only.iter().map(doc_param).collect(),
            kwargs: self.param_spec.kwargs.as_ref().map(doc_param),
        }
    }

    pub(crate) fn into_docs(self, as_type: Option<(Ty, DocType)>) -> DocItem {
        let func_docs = DocFunction::from_docstring(
            DocStringKind::Rust,
            self.doc_params(),
            self.return_type.clone(),
            self.rust_docstring,
        );
        match as_type {
            Some((_, ty_docs)) => DocItem::Type(DocType {
                constructor: Some(func_docs),
                ..ty_docs
            }),
            None => DocItem::Member(DocMember::Function(func_docs)),
        }
    }
}
