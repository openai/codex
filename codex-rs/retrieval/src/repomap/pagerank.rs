//! PageRank algorithm for repo map file ranking.
//!
//! Implements personalized PageRank with weighted edges for
//! context-aware file importance ranking.

use std::collections::HashMap;

use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::error::Result;
use crate::tags::extractor::CodeTag;

use super::RankedSymbol;
use super::graph::EdgeData;

/// PageRank ranker for file importance scoring.
pub struct PageRanker {
    /// Damping factor (typically 0.85)
    damping_factor: f64,
    /// Maximum iterations before stopping
    max_iterations: i32,
    /// Convergence tolerance
    tolerance: f64,
}

impl PageRanker {
    /// Create a new PageRanker with the given parameters.
    pub fn new(damping_factor: f64, max_iterations: i32, tolerance: f64) -> Self {
        Self {
            damping_factor,
            max_iterations,
            tolerance,
        }
    }

    /// Run personalized PageRank on the graph.
    ///
    /// Returns a map from file path to rank score.
    pub fn rank(
        &self,
        graph: &DiGraph<String, EdgeData>,
        personalization: &HashMap<String, f64>,
    ) -> Result<HashMap<String, f64>> {
        let node_count = graph.node_count();
        if node_count == 0 {
            return Ok(HashMap::new());
        }

        // Build node index to filepath mapping
        let mut idx_to_path: HashMap<NodeIndex, String> = HashMap::new();
        let mut path_to_idx: HashMap<String, NodeIndex> = HashMap::new();

        for idx in graph.node_indices() {
            let path = &graph[idx];
            idx_to_path.insert(idx, path.clone());
            path_to_idx.insert(path.clone(), idx);
        }

        // Initialize ranks
        let initial_rank = 1.0 / node_count as f64;
        let mut ranks: HashMap<NodeIndex, f64> = graph
            .node_indices()
            .map(|idx| (idx, initial_rank))
            .collect();

        // Precompute outgoing edge weights for each node
        let mut out_weights: HashMap<NodeIndex, f64> = HashMap::new();
        for idx in graph.node_indices() {
            let weight_sum: f64 = graph.edges(idx).map(|e| e.weight().weight).sum();
            out_weights.insert(idx, weight_sum);
        }

        // Personalization vector (default to uniform if not provided)
        let pers_vec: HashMap<NodeIndex, f64> = if personalization.is_empty() {
            graph
                .node_indices()
                .map(|idx| (idx, initial_rank))
                .collect()
        } else {
            personalization
                .iter()
                .filter_map(|(path, prob)| path_to_idx.get(path).map(|&idx| (idx, *prob)))
                .collect()
        };

        // Power iteration
        for _iteration in 0..self.max_iterations {
            let mut new_ranks: HashMap<NodeIndex, f64> = HashMap::new();
            let mut diff = 0.0_f64;

            for idx in graph.node_indices() {
                // Sum contributions from incoming edges
                let mut rank_sum = 0.0_f64;

                for edge in graph.edges_directed(idx, petgraph::Direction::Incoming) {
                    let source = edge.source();
                    let edge_weight = edge.weight().weight;
                    let source_out_weight = out_weights.get(&source).copied().unwrap_or(1.0);

                    if source_out_weight > 0.0 {
                        let source_rank = ranks.get(&source).copied().unwrap_or(initial_rank);
                        rank_sum += source_rank * (edge_weight / source_out_weight);
                    }
                }

                // Apply damping and personalization
                let pers_prob = pers_vec.get(&idx).copied().unwrap_or(initial_rank);
                let new_rank =
                    (1.0 - self.damping_factor) * pers_prob + self.damping_factor * rank_sum;

                let old_rank = ranks.get(&idx).copied().unwrap_or(initial_rank);
                diff += (new_rank - old_rank).abs();

                new_ranks.insert(idx, new_rank);
            }

            ranks = new_ranks;

            // Check convergence
            if diff < self.tolerance {
                break;
            }
        }

        // Normalize ranks to sum to 1.0
        let total: f64 = ranks.values().sum();
        if total > 0.0 {
            for rank in ranks.values_mut() {
                *rank /= total;
            }
        }

        // Convert back to filepath keys
        let result: HashMap<String, f64> = ranks
            .into_iter()
            .filter_map(|(idx, rank)| idx_to_path.get(&idx).map(|path| (path.clone(), rank)))
            .collect();

        Ok(result)
    }

    /// Distribute file ranks to individual symbol definitions.
    ///
    /// Returns a vector of (filepath, symbol_name, rank) sorted by rank descending.
    ///
    /// # Arguments
    /// * `file_ranks` - PageRank scores for each file
    /// * `definitions` - Map from symbol name to list of (filepath, tag)
    /// * `file_def_counts` - Total definition count per file (from graph.compute_file_definition_counts)
    pub fn distribute_to_definitions(
        &self,
        file_ranks: &HashMap<String, f64>,
        definitions: &HashMap<String, Vec<(String, CodeTag)>>,
        file_def_counts: &HashMap<String, i32>,
    ) -> Vec<RankedSymbol> {
        let mut ranked_symbols = Vec::new();

        // Collect all definitions with their file ranks
        for (symbol_name, def_locations) in definitions {
            for (filepath, tag) in def_locations {
                let file_rank = file_ranks.get(filepath).copied().unwrap_or(0.0);

                // Distribute file rank proportionally to total definitions in file
                // This ensures symbols in files with many definitions get smaller individual ranks
                let total_defs = file_def_counts.get(filepath).copied().unwrap_or(1);
                let symbol_rank = file_rank / total_defs.max(1) as f64;

                ranked_symbols.push(RankedSymbol {
                    tag: CodeTag {
                        name: symbol_name.clone(),
                        kind: tag.kind.clone(),
                        start_line: tag.start_line,
                        end_line: tag.end_line,
                        start_byte: tag.start_byte,
                        end_byte: tag.end_byte,
                        signature: tag.signature.clone(),
                        docs: tag.docs.clone(),
                        is_definition: true,
                    },
                    rank: symbol_rank,
                    filepath: filepath.clone(),
                });
            }
        }

        // Sort by rank descending
        ranked_symbols.sort_by(|a, b| {
            b.rank
                .partial_cmp(&a.rank)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        ranked_symbols
    }
}

impl Default for PageRanker {
    fn default() -> Self {
        Self::new(0.85, 100, 1e-6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::extractor::TagKind;

    #[test]
    fn test_empty_graph() {
        let ranker = PageRanker::default();
        let graph: DiGraph<String, EdgeData> = DiGraph::new();
        let pers = HashMap::new();

        let ranks = ranker.rank(&graph, &pers).unwrap();
        assert!(ranks.is_empty());
    }

    #[test]
    fn test_single_node() {
        let ranker = PageRanker::default();
        let mut graph: DiGraph<String, EdgeData> = DiGraph::new();
        graph.add_node("a.rs".to_string());

        let ranks = ranker.rank(&graph, &HashMap::new()).unwrap();
        assert_eq!(ranks.len(), 1);
        assert!((ranks["a.rs"] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_two_nodes_with_edge() {
        let ranker = PageRanker::default();
        let mut graph: DiGraph<String, EdgeData> = DiGraph::new();

        let a = graph.add_node("a.rs".to_string());
        let b = graph.add_node("b.rs".to_string());

        // a references b (edge a -> b)
        graph.add_edge(
            a,
            b,
            EdgeData {
                weight: 1.0,
                symbol: "foo".to_string(),
            },
        );

        let ranks = ranker.rank(&graph, &HashMap::new()).unwrap();

        // b should have higher rank (it's referenced)
        assert!(ranks["b.rs"] > ranks["a.rs"]);
    }

    #[test]
    fn test_personalization_boost() {
        let ranker = PageRanker::default();
        let mut graph: DiGraph<String, EdgeData> = DiGraph::new();

        graph.add_node("a.rs".to_string());
        graph.add_node("b.rs".to_string());

        // Personalize to boost a.rs
        let mut pers = HashMap::new();
        pers.insert("a.rs".to_string(), 0.9);
        pers.insert("b.rs".to_string(), 0.1);

        let ranks = ranker.rank(&graph, &pers).unwrap();

        // a.rs should have higher rank due to personalization
        assert!(ranks["a.rs"] > ranks["b.rs"]);
    }

    #[test]
    fn test_distribute_to_definitions() {
        let ranker = PageRanker::default();

        let mut file_ranks = HashMap::new();
        file_ranks.insert("a.rs".to_string(), 0.6);
        file_ranks.insert("b.rs".to_string(), 0.4);

        let mut definitions = HashMap::new();
        definitions.insert(
            "foo".to_string(),
            vec![(
                "a.rs".to_string(),
                CodeTag {
                    name: "foo".to_string(),
                    kind: TagKind::Function,
                    start_line: 10,
                    end_line: 20,
                    start_byte: 100,
                    end_byte: 200,
                    signature: Some("fn foo()".to_string()),
                    docs: None,
                    is_definition: true,
                },
            )],
        );
        definitions.insert(
            "bar".to_string(),
            vec![(
                "b.rs".to_string(),
                CodeTag {
                    name: "bar".to_string(),
                    kind: TagKind::Function,
                    start_line: 5,
                    end_line: 15,
                    start_byte: 50,
                    end_byte: 150,
                    signature: Some("fn bar()".to_string()),
                    docs: None,
                    is_definition: true,
                },
            )],
        );

        // Each file has 1 definition
        let mut file_def_counts = HashMap::new();
        file_def_counts.insert("a.rs".to_string(), 1);
        file_def_counts.insert("b.rs".to_string(), 1);

        let ranked = ranker.distribute_to_definitions(&file_ranks, &definitions, &file_def_counts);

        assert_eq!(ranked.len(), 2);
        // foo (from a.rs with 0.6 rank) should be first
        assert_eq!(ranked[0].tag.name, "foo");
        assert_eq!(ranked[1].tag.name, "bar");
    }

    #[test]
    fn test_distribute_with_multiple_defs_per_file() {
        let ranker = PageRanker::default();

        let mut file_ranks = HashMap::new();
        file_ranks.insert("a.rs".to_string(), 0.6);

        // File a.rs has 3 definitions: foo, bar, baz
        let mut definitions = HashMap::new();
        definitions.insert(
            "foo".to_string(),
            vec![(
                "a.rs".to_string(),
                CodeTag {
                    name: "foo".to_string(),
                    kind: TagKind::Function,
                    start_line: 10,
                    end_line: 20,
                    start_byte: 100,
                    end_byte: 200,
                    signature: None,
                    docs: None,
                    is_definition: true,
                },
            )],
        );
        definitions.insert(
            "bar".to_string(),
            vec![(
                "a.rs".to_string(),
                CodeTag {
                    name: "bar".to_string(),
                    kind: TagKind::Function,
                    start_line: 30,
                    end_line: 40,
                    start_byte: 300,
                    end_byte: 400,
                    signature: None,
                    docs: None,
                    is_definition: true,
                },
            )],
        );
        definitions.insert(
            "baz".to_string(),
            vec![(
                "a.rs".to_string(),
                CodeTag {
                    name: "baz".to_string(),
                    kind: TagKind::Function,
                    start_line: 50,
                    end_line: 60,
                    start_byte: 500,
                    end_byte: 600,
                    signature: None,
                    docs: None,
                    is_definition: true,
                },
            )],
        );

        // File a.rs has 3 definitions total
        let mut file_def_counts = HashMap::new();
        file_def_counts.insert("a.rs".to_string(), 3);

        let ranked = ranker.distribute_to_definitions(&file_ranks, &definitions, &file_def_counts);

        assert_eq!(ranked.len(), 3);

        // Each symbol should get 0.6 / 3 = 0.2 rank
        for sym in &ranked {
            assert!(
                (sym.rank - 0.2).abs() < 0.001,
                "Expected rank ~0.2, got {}",
                sym.rank
            );
        }
    }
}
