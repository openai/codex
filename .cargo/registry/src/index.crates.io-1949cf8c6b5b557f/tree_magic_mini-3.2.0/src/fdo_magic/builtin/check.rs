use super::ALL_RULES;
use crate::{fdo_magic::check::from_u8_walker, read_bytes, Mime};
use petgraph::prelude::*;
use std::collections::HashMap;
use std::fs::File;

pub(crate) struct FdoMagic;

impl crate::Checker for FdoMagic {
    fn match_bytes(&self, bytes: &[u8], mimetype: &str) -> bool {
        // Get magic ruleset
        let Some(graph) = ALL_RULES.get(mimetype) else {
            return false;
        };

        // Check all rulesets
        graph
            .externals(Incoming)
            .any(|node| from_u8_walker(bytes, graph, node, true))
    }

    fn match_file(&self, file: &File, mimetype: &str) -> bool {
        // Get magic ruleset
        let Some(magic_rules) = ALL_RULES.get(mimetype) else {
            return false;
        };

        // Get # of bytes to read
        let scanlen = magic_rules
            .node_weights()
            .map(|rule| rule.scan_len())
            .max()
            .unwrap_or(0);

        let Ok(bytes) = read_bytes(file, scanlen) else {
            return false;
        };

        self.match_bytes(&bytes, mimetype)
    }

    fn get_supported(&self) -> Vec<Mime> {
        super::init::get_supported()
    }

    fn get_subclasses(&self) -> Vec<(Mime, Mime)> {
        super::init::get_subclasses()
    }

    fn get_aliaslist(&self) -> HashMap<Mime, Mime> {
        super::init::get_aliaslist()
    }
}
