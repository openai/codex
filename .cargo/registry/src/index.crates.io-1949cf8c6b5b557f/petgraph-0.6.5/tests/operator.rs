use petgraph::operator::complement;
use petgraph::prelude::*;
use petgraph::Graph;

#[test]
fn test_complement() {
    let mut graph: Graph<(), (), Directed> = Graph::new();
    let a = graph.add_node(());
    let b = graph.add_node(());
    let c = graph.add_node(());
    let d = graph.add_node(());

    graph.extend_with_edges(&[(a, b), (b, c), (c, d)]);
    let mut output: Graph<(), (), Directed> = Graph::new();

    complement(&graph, &mut output, ());

    let mut expected_res: Graph<(), (), Directed> = Graph::new();
    let a = expected_res.add_node(());
    let b = expected_res.add_node(());
    let c = expected_res.add_node(());
    let d = expected_res.add_node(());
    expected_res.extend_with_edges(&[
        (a, c),
        (a, d),
        (b, a),
        (b, d),
        (c, a),
        (c, b),
        (d, a),
        (d, b),
        (d, c),
    ]);

    for x in graph.node_indices() {
        for y in graph.node_indices() {
            assert_eq!(output.contains_edge(x, y), expected_res.contains_edge(x, y));
        }
    }
}
