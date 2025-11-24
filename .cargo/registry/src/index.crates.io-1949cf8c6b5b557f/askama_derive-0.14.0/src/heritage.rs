use core::fmt;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use parser::node::{BlockDef, Macro};
use parser::{Node, Parsed, Span};
use rustc_hash::FxBuildHasher;

use crate::config::Config;
use crate::{CompileError, FileInfo};

pub(crate) struct Heritage<'a, 'h> {
    pub(crate) root: &'h Context<'a>,
    pub(crate) blocks: BlockAncestry<'a, 'h>,
}

impl<'a, 'h> Heritage<'a, 'h> {
    pub(crate) fn new(
        mut root: &'h Context<'a>,
        contexts: &'a HashMap<&'a Arc<Path>, Context<'a>, FxBuildHasher>,
    ) -> Self {
        let mut blocks: BlockAncestry<'a, 'h> = root
            .blocks
            .iter()
            .map(|(name, def)| (*name, vec![(root, *def)]))
            .collect();

        while let Some(path) = &root.extends {
            root = &contexts[path];
            for (name, def) in &root.blocks {
                blocks.entry(name).or_default().push((root, def));
            }
        }

        Self { root, blocks }
    }
}

type BlockAncestry<'a, 'h> =
    HashMap<&'a str, Vec<(&'h Context<'a>, &'a BlockDef<'a>)>, FxBuildHasher>;

#[derive(Clone)]
pub(crate) struct Context<'a> {
    pub(crate) nodes: &'a [Node<'a>],
    pub(crate) extends: Option<Arc<Path>>,
    pub(crate) blocks: HashMap<&'a str, &'a BlockDef<'a>, FxBuildHasher>,
    pub(crate) macros: HashMap<&'a str, &'a Macro<'a>, FxBuildHasher>,
    pub(crate) imports: HashMap<&'a str, Arc<Path>, FxBuildHasher>,
    pub(crate) path: Option<&'a Path>,
    pub(crate) parsed: &'a Parsed,
}

impl<'a> Context<'a> {
    pub(crate) fn empty(parsed: &Parsed) -> Context<'_> {
        Context {
            nodes: &[],
            extends: None,
            blocks: HashMap::default(),
            macros: HashMap::default(),
            imports: HashMap::default(),
            path: None,
            parsed,
        }
    }

    pub(crate) fn new(
        config: &Config,
        path: &'a Path,
        parsed: &'a Parsed,
    ) -> Result<Self, CompileError> {
        let mut extends = None;
        let mut blocks = HashMap::default();
        let mut macros = HashMap::default();
        let mut imports = HashMap::default();
        let mut nested = vec![parsed.nodes()];
        let mut top = true;

        while let Some(nodes) = nested.pop() {
            for n in nodes {
                match n {
                    Node::Extends(e) => {
                        ensure_top(top, e.span(), path, parsed, "extends")?;
                        if extends.is_some() {
                            return Err(CompileError::new(
                                "multiple extend blocks found",
                                Some(FileInfo::of(e.span(), path, parsed)),
                            ));
                        }
                        extends = Some(config.find_template(
                            e.path,
                            Some(path),
                            Some(FileInfo::of(e.span(), path, parsed)),
                        )?);
                    }
                    Node::Macro(m) => {
                        ensure_top(top, m.span(), path, parsed, "macro")?;
                        macros.insert(m.name, &**m);
                    }
                    Node::Import(import) => {
                        ensure_top(top, import.span(), path, parsed, "import")?;
                        let path = config.find_template(
                            import.path,
                            Some(path),
                            Some(FileInfo::of(import.span(), path, parsed)),
                        )?;
                        imports.insert(import.scope, path);
                    }
                    Node::BlockDef(b) => {
                        blocks.insert(b.name, &**b);
                        nested.push(&b.nodes);
                    }
                    Node::If(i) => {
                        for cond in &i.branches {
                            nested.push(&cond.nodes);
                        }
                    }
                    Node::Loop(l) => {
                        nested.push(&l.body);
                        nested.push(&l.else_nodes);
                    }
                    Node::Match(m) => {
                        for arm in &m.arms {
                            nested.push(&arm.nodes);
                        }
                    }
                    _ => {}
                }
            }
            top = false;
        }

        Ok(Context {
            nodes: parsed.nodes(),
            extends,
            blocks,
            macros,
            imports,
            parsed,
            path: Some(path),
        })
    }

    pub(crate) fn generate_error(&self, msg: impl fmt::Display, node: Span<'_>) -> CompileError {
        CompileError::new(msg, self.file_info_of(node))
    }

    pub(crate) fn file_info_of(&self, node: Span<'a>) -> Option<FileInfo<'a>> {
        self.path.map(|path| FileInfo::of(node, path, self.parsed))
    }
}

fn ensure_top(
    top: bool,
    node: Span<'_>,
    path: &Path,
    parsed: &Parsed,
    kind: &str,
) -> Result<(), CompileError> {
    if top {
        Ok(())
    } else {
        Err(CompileError::new(
            format!("`{kind}` blocks are not allowed below top level"),
            Some(FileInfo::of(node, path, parsed)),
        ))
    }
}
