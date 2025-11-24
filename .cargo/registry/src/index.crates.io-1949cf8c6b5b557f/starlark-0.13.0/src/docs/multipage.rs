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

use std::collections::HashMap;

use dupe::Dupe;

use crate::docs::DocItem;
use crate::docs::DocModule;
use crate::docs::DocType;
use crate::typing::ty::TypeRenderConfig;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::typing::TyStarlarkValue;

pub struct DocModuleInfo<'a> {
    pub module: &'a DocModule,
    pub name: String,
    /// A prefix to attach to all of the pages rendered from this module
    pub page_path: String,
}

impl<'a> DocModuleInfo<'a> {
    fn into_page_renders(&self) -> Vec<PageRender<'a>> {
        Self::traverse_inner(&self.module, &self.name, &self.page_path)
    }

    fn traverse_inner(
        docs: &'a DocModule,
        module_name: &str,
        base_path: &str,
    ) -> Vec<PageRender<'a>> {
        let mut result = vec![];

        result.push(PageRender {
            page: DocPageRef::Module(docs),
            path: base_path.to_owned(),
            name: module_name.to_owned(),
            ty: None,
        });

        for (name, doc) in &docs.members {
            let path = if base_path.is_empty() {
                name.to_owned()
            } else {
                format!("{}/{}", base_path, name)
            };
            match doc {
                DocItem::Module(doc_module) => {
                    result.extend(Self::traverse_inner(&doc_module, &name, &path))
                }
                DocItem::Type(doc_type) => result.push(PageRender {
                    page: DocPageRef::Type(doc_type),
                    path,
                    name: name.to_owned(),
                    ty: Some(doc_type.ty.dupe()),
                }),

                DocItem::Member(_) => (),
            }
        }

        result
    }
}

/// A reference to a page to render
/// DocsRender will have all the PageRender it needs to render the docs
/// Since types and some modules are owned by other modules, we need to use the reference here
enum DocPageRef<'a> {
    Module(&'a DocModule),
    Type(&'a DocType),
}

/// A single page to render
struct PageRender<'a> {
    page: DocPageRef<'a>,
    path: String,
    name: String,
    /// The type of the page, if it is a type page. This is used to get the link to the type.
    ty: Option<Ty>,
}

impl<'a> PageRender<'a> {
    fn render_markdown(&self, render_config: &TypeRenderConfig) -> String {
        match self.page {
            DocPageRef::Module(doc_module) => {
                doc_module.render_markdown_page_for_multipage_render(&self.name, render_config)
            }
            DocPageRef::Type(doc_type) => {
                doc_type.render_markdown_page_for_multipage_render(&self.name, render_config)
            }
        }
    }
}

/// Renders the contents into a multi-page tree structure
///
/// The output will contain page-paths like ``, `type1`, `mod1`, and `mod1/type2`, each mapped to
/// the contents of that page. That means that some of the paths may be prefixes of each other,
/// which will need consideration if this is to be materialized to a filesystem.
struct MultipageRender<'a> {
    page_renders: Vec<PageRender<'a>>,
    // used for the linkable type in the markdown
    render_config: TypeRenderConfig,
}

impl<'a> MultipageRender<'a> {
    /// Create a new MultipageRender from a list of DocModuleInfo, and an optional function to map a type path to a linkable path
    /// If the function is not provided, the type will not be linkable
    /// linked_ty_mapper is used to map the **type path** and **type name** to a linkable element in the markdown
    fn new(
        docs: Vec<DocModuleInfo<'a>>,
        linked_ty_mapper: Option<fn(&str, &str) -> String>,
    ) -> Self {
        let mut res = vec![];
        for doc in docs {
            res.extend(doc.into_page_renders());
        }
        let mut render_config = TypeRenderConfig::Default;
        if let Some(linked_ty_mapper) = linked_ty_mapper {
            let mut ty_to_path_map = HashMap::new();
            for page in res.iter() {
                if let Some(ty) = &page.ty {
                    ty_to_path_map.insert(ty.dupe(), page.path.clone());
                }
            }

            let render_linked_ty_starlark_value = move |ty: &TyStarlarkValue| {
                let type_name = ty.to_string();
                if let Some(type_path) =
                    ty_to_path_map.get(&Ty::basic(TyBasic::StarlarkValue(ty.dupe())))
                {
                    linked_ty_mapper(type_path, &type_name)
                } else {
                    type_name.to_owned()
                }
            };

            render_config = TypeRenderConfig::LinkedType {
                render_linked_ty_starlark_value: Box::new(render_linked_ty_starlark_value),
            };
        }
        Self {
            page_renders: res,
            render_config,
        }
    }

    /// Render the docs into a map of markdown paths to markdown content
    fn render_markdown_pages(&self) -> HashMap<String, String> {
        self.page_renders
            .iter()
            .map(|page| (page.path.clone(), page.render_markdown(&self.render_config)))
            .collect()
    }
}

/// Renders the contents into a multi-page tree structure
///
/// The output will contain page-paths like ``, `type1`, `mod1`, and `mod1/type2`, each mapped to
/// the contents of that page. That means that some of the paths may be prefixes of each other,
/// which will need consideration if this is to be materialized to a filesystem.
///
/// It accepts a list of DocModuleInfo, and an optional function linked_ty_mapper
/// linked_ty_mapper is used to map the **type path** and **type name** to a linkable element in the markdown
pub fn render_markdown_multipage(
    modules_infos: Vec<DocModuleInfo<'_>>,
    linked_ty_mapper: Option<fn(&str, &str) -> String>,
) -> HashMap<String, String> {
    let multipage_render = MultipageRender::new(modules_infos, linked_ty_mapper);
    multipage_render.render_markdown_pages()
}
