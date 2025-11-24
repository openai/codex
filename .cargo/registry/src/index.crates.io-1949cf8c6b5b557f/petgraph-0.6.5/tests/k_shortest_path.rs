use petgraph::algo::k_shortest_path;
use petgraph::prelude::*;
use petgraph::Graph;
use std::collections::HashMap;

#[test]
fn second_shortest_path() {
    let mut graph: Graph<(), (), Directed> = Graph::new();
    let a = graph.add_node(());
    let b = graph.add_node(());
    let c = graph.add_node(());
    let d = graph.add_node(());
    let e = graph.add_node(());
    let f = graph.add_node(());
    let g = graph.add_node(());
    let h = graph.add_node(());
    let i = graph.add_node(());
    let j = graph.add_node(());
    let k = graph.add_node(());
    let l = graph.add_node(());
    let m = graph.add_node(());

    graph.extend_with_edges(&[
        (a, b),
        (b, c),
        (c, d),
        (b, f),
        (f, g),
        (c, g),
        (g, h),
        (d, e),
        (e, h),
        (h, i),
        (h, j),
        (h, k),
        (h, l),
        (i, m),
        (l, k),
        (j, k),
        (j, m),
        (k, m),
        (l, m),
        (m, e),
    ]);

    let res = k_shortest_path(&graph, a, None, 2, |_| 1);

    let expected_res: HashMap<NodeIndex, usize> = [
        (e, 7),
        (g, 3),
        (h, 4),
        (i, 5),
        (j, 5),
        (k, 5),
        (l, 5),
        (m, 6),
    ]
    .iter()
    .cloned()
    .collect();

    assert_eq!(res, expected_res);
}
