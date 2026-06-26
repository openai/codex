use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use anyhow::Result;

static NEXT_DOCUMENT_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub(crate) struct BrowserHandles {
    document_generation: u64,
    next_node: u64,
    backend_nodes: HashMap<String, u64>,
}

impl Default for BrowserHandles {
    fn default() -> Self {
        Self {
            document_generation: next_document_generation(),
            next_node: 1,
            backend_nodes: HashMap::new(),
        }
    }
}

impl BrowserHandles {
    pub(crate) fn begin_snapshot(&mut self) {
        self.clear();
    }

    pub(crate) fn insert(&mut self, backend_node_id: u64) -> String {
        let node_id = format!("d{:016x}n{}", self.document_generation, self.next_node);
        self.next_node += 1;
        self.backend_nodes.insert(node_id.clone(), backend_node_id);
        node_id
    }

    pub(crate) fn resolve(&self, node_id: &str) -> Result<u64> {
        self.backend_nodes.get(node_id).copied().ok_or_else(|| {
            anyhow::anyhow!("node_not_found: take a new snapshot and use its nodeId")
        })
    }

    pub(crate) fn clear(&mut self) {
        self.document_generation = next_document_generation();
        self.next_node = 1;
        self.backend_nodes.clear();
    }
}

fn next_document_generation() -> u64 {
    NEXT_DOCUMENT_GENERATION.fetch_add(/*val*/ 1, Ordering::Relaxed)
}

#[cfg(test)]
#[path = "handles_tests.rs"]
mod tests;
