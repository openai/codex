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

use std::fmt::Display;

use itertools::Itertools;

use crate::docs::DocFunction;
use crate::docs::DocItem;
use crate::docs::DocMember;
use crate::docs::DocModule;
use crate::docs::DocParam;
use crate::docs::DocParams;
use crate::docs::DocProperty;
use crate::docs::DocReturn;
use crate::docs::DocString;
use crate::docs::DocType;
use crate::eval::runtime::params::display::fmt_param_spec_maybe_multiline;
use crate::eval::runtime::params::display::ParamFmt;
use crate::typing::ty::TypeRenderConfig;
use crate::typing::Ty;

/// There have been bugs around line endings in the textwrap crate. Just join
/// into a single string, and trim the line endings.
fn wrap_trimmed(s: &str, width: usize) -> String {
    textwrap::wrap(s, width).join("\n").trim_end().to_owned()
}

/// There have been bugs around line endings in the textwrap crate. Just trim the line endings.
fn indent_trimmed(s: &str, prefix: &str) -> String {
    textwrap::indent(s, prefix).trim_end().to_owned()
}

impl DocString {
    /// Render this docstring as a "starlark" docstring.
    fn render_as_code(&self) -> String {
        let s = match &self.details {
            Some(details) => {
                format!("{}\n\n{}", self.summary, details)
            }
            None => self.summary.clone(),
        };
        wrap_trimmed(&s, 80)
    }

    /// Render the docstring as in `render_as_code`, but surround it in triple quotes,
    /// a common convention in starlark docstrings.
    fn render_as_quoted_code(&self) -> String {
        format!("\"\"\"\n{}\n\"\"\"", self.render_as_code())
    }
}

impl DocModule {
    pub fn render_as_code(&self) -> String {
        let mut res = self
            .docs
            .as_ref()
            .map(DocString::render_as_quoted_code)
            .unwrap_or_default();
        for (k, v) in &self.members {
            if let Ok(v) = v.try_as_member_with_collapsed_object() {
                res.push('\n');
                match v {
                    DocMember::Property(p) => res.push_str(&p.render_as_code(k)),
                    DocMember::Function(f) => res.push_str(&f.render_as_code(k)),
                }
                res.push('\n');
            }
        }
        res
    }
}

impl DocFunction {
    fn starlark_docstring(&self) -> Option<String> {
        let mut docs = String::new();
        if let Some(main_docs) = self.docs.as_ref().map(DocString::render_as_code) {
            docs.push_str(&main_docs);
        }

        let args_indentation_count = self
            .params
            .doc_params()
            .map(|p| p.name.len() + 2)
            .max()
            .unwrap_or_default();
        let args_indentation = " ".repeat(args_indentation_count);

        let args_docs = self
            .params
            .doc_params()
            .filter_map(|p| p.starlark_docstring(&args_indentation))
            .join("\n");
        if !args_docs.is_empty() {
            let indented = indent_trimmed(&args_docs, "    ");
            docs.push_str(&format!("\n\nArgs:\n{}", indented));
        }

        if let Some(ret_docs) = self.ret.starlark_docstring() {
            let indented = indent_trimmed(&ret_docs, "    ");
            docs.push_str(&format!("\n\nRet:\n{}", indented));
        }
        if docs.is_empty() {
            None
        } else {
            Some(indent_trimmed(
                &format!("\"\"\"\n{}\n\"\"\"", docs.trim_start()),
                "    ",
            ))
        }
    }

    pub fn render_as_code(&self, name: &str) -> String {
        let params_one_line = self.params.render_code(None, &TypeRenderConfig::Default);

        let params = if params_one_line.len() > 60 {
            format!(
                "(\n{})",
                self.params
                    .render_code(Some("    "), &TypeRenderConfig::Default),
            )
        } else {
            format!("({})", params_one_line)
        };
        let docstring = self
            .starlark_docstring()
            .map(|mut ds| {
                ds.push('\n');
                ds
            })
            .unwrap_or_default();
        let ret = Some(&self.ret.typ)
            .filter(|t| t != &&Ty::any())
            .map(|t| format!(" -> {}", t))
            .unwrap_or_default();

        format!("def {}{}{}:\n{}    pass", name, params, ret, docstring)
    }
}

impl DocParam {
    fn starlark_docstring(&self, max_indentation: &str) -> Option<String> {
        let DocParam { name, docs, .. } = self;
        let rendered_docs = docs.as_ref()?.render_as_code();
        let mut indented = indent_trimmed(&rendered_docs, max_indentation);
        indented.replace_range(..name.len() + 2, &format!("{}: ", name));
        Some(indented)
    }

    fn fmt_param<'a>(
        &'a self,
        render_config: &'a TypeRenderConfig,
    ) -> ParamFmt<'a, impl Display + 'a, impl Display + 'a> {
        let DocParam {
            name,
            docs: _,
            typ,
            default_value,
        } = self;
        let ty = if typ.is_any() {
            None
        } else {
            Some(typ.display_with(render_config))
        };
        ParamFmt {
            name,
            ty,
            default: default_value.as_ref(),
        }
    }
}

impl DocParams {
    /// Render multiline if `indent` is `Some`.
    pub(crate) fn render_code(
        &self,
        indent: Option<&str>,
        render_config: &TypeRenderConfig,
    ) -> String {
        let mut s = String::new();
        fmt_param_spec_maybe_multiline(
            &mut s,
            indent,
            self.pos_only.iter().map(|p| p.fmt_param(render_config)),
            self.pos_or_named.iter().map(|p| p.fmt_param(render_config)),
            self.args.as_ref().map(|p| p.fmt_param(render_config)),
            self.named_only.iter().map(|p| p.fmt_param(render_config)),
            self.kwargs.as_ref().map(|p| p.fmt_param(render_config)),
        )
        .unwrap();
        s
    }
}

impl DocReturn {
    fn starlark_docstring(&self) -> Option<String> {
        self.docs.as_ref().map(DocString::render_as_code)
    }
}

impl DocProperty {
    pub fn render_as_code(&self, name: &str) -> String {
        match (
            &self.typ,
            self.docs.as_ref().map(DocString::render_as_quoted_code),
        ) {
            // TODO(nmj): The starlark syntax needs to be updated to support type
            //            annotations on values as python does. Afterward, use these
            //            format strings.
            // (Some(t), Some(ds)) => {
            //     format!("{}\n_{}: {} = None", ds, name, t.raw_type)
            // }
            // (Some(t), None) => format!(r#"_{}: {} = None"#, name, t.raw_type),
            (t, Some(ds)) if t.is_any() => format!("{}\n_{} = None", ds, name),
            (t, None) if t.is_any() => format!("_{} = None", name),
            (t, Some(ds)) => {
                format!("{}\n# type: {}\n_{} = None", ds, t, name)
            }
            (t, None) => format!("# type: {}\n_{} = None", t, name),
        }
    }
}

impl DocType {
    fn render_as_code(&self, name: &str) -> String {
        let summary = self
            .docs
            .as_ref()
            .map(|ds| {
                let mut s = ds.render_as_quoted_code();
                s.push('\n');
                s
            })
            .unwrap_or_default();

        let member_docs = self
            .members
            .iter()
            .map(|(name, member)| match member {
                DocMember::Property(p) => p.render_as_code(name),
                DocMember::Function(f) => f.render_as_code(&format!("_{}", name)),
            })
            .join("\n\n");

        let exported_struct_members = self
            .members
            .iter()
            .map(|(name, _)| format!("    {} = _{},", name, name))
            .join("\n");
        let exported_struct = if !exported_struct_members.is_empty() {
            format!(
                "{}{} = struct(\n{}\n)",
                summary, name, exported_struct_members
            )
        } else {
            String::new()
        };

        format!("{}\n\n{}", member_docs, exported_struct)
            .trim()
            .to_owned()
    }
}

impl DocItem {
    pub fn render_as_code(&self, name: &str) -> String {
        match self {
            DocItem::Module(m) => m.render_as_code(),
            DocItem::Type(o) => o.render_as_code(&name),
            DocItem::Member(DocMember::Function(f)) => f.render_as_code(&name),
            DocItem::Member(DocMember::Property(p)) => p.render_as_code(&name),
        }
    }
}
