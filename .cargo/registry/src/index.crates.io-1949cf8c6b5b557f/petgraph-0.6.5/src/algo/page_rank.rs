use crate::visit::{EdgeRef, IntoEdges, NodeCount, NodeIndexable};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use super::UnitMeasure;
/// \[Generic\] Page Rank algorithm.
///
/// Computes the ranks of every node in a graph using the [Page Rank algorithm][pr].
///
/// Returns a `Vec` container mapping each node index to its rank.
///
/// # Panics
/// The damping factor should be a number of type `f32` or `f64` between 0 and 1 (0 and 1 included). Otherwise, it panics.
///
/// # Complexity
/// Time complexity is **O(N|V|²|E|)**.
/// Space complexity is **O(|V| + |E|)**
/// where **N** is the number of iterations, **|V|** the number of vertices (i.e nodes) and **|E|** the number of edges.
///
/// [pr]: https://en.wikipedia.org/wiki/PageRank
///
/// # Example
/// ```rust
/// use petgraph::Graph;
/// use petgraph::algo::page_rank;
/// let mut g: Graph<(), usize> = Graph::new();
/// assert_eq!(page_rank(&g, 0.5_f64, 1), vec![]); // empty graphs have no node ranks.
/// let a = g.add_node(());
/// let b = g.add_node(());
/// let c = g.add_node(());
/// let d = g.add_node(());
/// let e = g.add_node(());
/// g.extend_with_edges(&[(0, 1), (0, 3), (1, 2), (1, 3)]);
/// // With the following dot representation.
/// //digraph {
/// //    0 [ label = "()" ]
/// //    1 [ label = "()" ]
/// //    2 [ label = "()" ]
/// //    3 [ label = "()" ]
/// //    4 [ label = "()" ]
/// //    0 -> 1 [ label = "0.0" ]
/// //    0 -> 3 [ label = "0.0" ]
/// //    1 -> 2 [ label = "0.0" ]
/// //    1 -> 3 [ label = "0.0" ]
/// //}
/// let damping_factor = 0.7_f32;
/// let number_iterations = 10;
/// let output_ranks = page_rank(&g, damping_factor, number_iterations);
/// let expected_ranks = vec![0.14685437, 0.20267677, 0.22389607, 0.27971846, 0.14685437];
/// assert_eq!(expected_ranks, output_ranks);
/// ```
pub fn page_rank<G, D>(graph: G, damping_factor: D, nb_iter: usize) -> Vec<D>
where
    G: NodeCount + IntoEdges + NodeIndexable,
    D: UnitMeasure + Copy,
{
    let node_count = graph.node_count();
    if node_count == 0 {
        return vec![];
    }
    assert!(
        D::zero() <= damping_factor && damping_factor <= D::one(),
        "Damping factor should be between 0 et 1."
    );
    let nb = D::from_usize(node_count);
    let mut ranks = vec![D::one() / nb; node_count];
    let nodeix = |i| graph.from_index(i);
    let out_degrees: Vec<D> = (0..node_count)
        .map(|i| graph.edges(nodeix(i)).map(|_| D::one()).sum::<D>())
        .collect();

    for _ in 0..nb_iter {
        let pi = (0..node_count)
            .enumerate()
            .map(|(v, _)| {
                ranks
                    .iter()
                    .enumerate()
                    .map(|(w, r)| {
                        let mut w_out_edges = graph.edges(nodeix(w));
                        if w_out_edges.any(|e| e.target() == nodeix(v)) {
                            damping_factor * *r / out_degrees[w]
                        } else if out_degrees[w] == D::zero() {
                            damping_factor * *r / nb // stochastic matrix condition
                        } else {
                            (D::one() - damping_factor) * *r / nb // random jumps
                        }
                    })
                    .sum::<D>()
            })
            .collect::<Vec<D>>();
        let sum = pi.iter().copied().sum::<D>();
        ranks = pi.iter().map(|r| *r / sum).collect::<Vec<D>>();
    }
    ranks
}

#[allow(dead_code)]
fn out_edges_info<G, D>(graph: G, index_w: usize, index_v: usize) -> (D, bool)
where
    G: NodeCount + IntoEdges + NodeIndexable + std::marker::Sync,
    D: UnitMeasure + Copy + std::marker::Send + std::marker::Sync,
{
    let node_w = graph.from_index(index_w);
    let node_v = graph.from_index(index_v);
    let mut out_edges = graph.edges(node_w);
    let mut out_edge = out_edges.next();
    let mut out_degree = D::zero();
    let mut flag_points_to = false;
    while let Some(edge) = out_edge {
        out_degree = out_degree + D::one();
        if edge.target() == node_v {
            flag_points_to = true;
        }
        out_edge = out_edges.next();
    }
    (out_degree, flag_points_to)
}
/// \[Generic\] Parallel Page Rank algorithm.
///
/// See [`page_rank`].
#[cfg(feature = "rayon")]
pub fn parallel_page_rank<G, D>(
    graph: G,
    damping_factor: D,
    nb_iter: usize,
    tol: Option<D>,
) -> Vec<D>
where
    G: NodeCount + IntoEdges + NodeIndexable + std::marker::Sync,
    D: UnitMeasure + Copy + std::marker::Send + std::marker::Sync,
{
    let node_count = graph.node_count();
    if node_count == 0 {
        return vec![];
    }
    assert!(
        D::zero() <= damping_factor && damping_factor <= D::one(),
        "Damping factor should be between 0 et 1."
    );
    let mut tolerance = D::default_tol();
    if let Some(_tol) = tol {
        tolerance = _tol;
    }
    let nb = D::from_usize(node_count);
    let mut ranks: Vec<D> = (0..node_count)
        .into_par_iter()
        .map(|i| D::one() / nb)
        .collect();
    for _ in 0..nb_iter {
        let pi = (0..node_count)
            .into_par_iter()
            .map(|v| {
                ranks
                    .iter()
                    .enumerate()
                    .map(|(w, r)| {
                        let (out_deg, w_points_to_v) = out_edges_info(graph, w, v);
                        if w_points_to_v {
                            damping_factor * *r / out_deg
                        } else if out_deg == D::zero() {
                            damping_factor * *r / nb // stochastic matrix condition
                        } else {
                            (D::one() - damping_factor) * *r / nb // random jumps
                        }
                    })
                    .sum::<D>()
            })
            .collect::<Vec<D>>();
        let sum = pi.par_iter().map(|score| *score).sum::<D>();
        let new_ranks = pi.par_iter().map(|r| *r / sum).collect::<Vec<D>>();
        let squared_norm_2 = new_ranks
            .par_iter()
            .zip(&ranks)
            .map(|(new, old)| (*new - *old) * (*new - *old))
            .sum::<D>();
        if squared_norm_2 <= tolerance {
            return ranks;
        } else {
            ranks = new_ranks;
        }
    }
    ranks
}
