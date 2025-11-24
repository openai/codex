use petgraph::{algo::min_spanning_tree, dot::Dot, graph::UnGraph, Graph};

#[test]
fn mst() {
    use petgraph::data::FromElements;

    let mut gr = Graph::<_, _>::new();
    let a = gr.add_node("A");
    let b = gr.add_node("B");
    let c = gr.add_node("C");
    let d = gr.add_node("D");
    let e = gr.add_node("E");
    let f = gr.add_node("F");
    let g = gr.add_node("G");
    gr.add_edge(a, b, 7.);
    gr.add_edge(a, d, 5.);
    gr.add_edge(d, b, 9.);
    gr.add_edge(b, c, 8.);
    gr.add_edge(b, e, 7.);
    gr.add_edge(c, e, 5.);
    gr.add_edge(d, e, 15.);
    gr.add_edge(d, f, 6.);
    gr.add_edge(f, e, 8.);
    gr.add_edge(f, g, 11.);
    gr.add_edge(e, g, 9.);

    // add a disjoint part
    let h = gr.add_node("H");
    let i = gr.add_node("I");
    let j = gr.add_node("J");
    gr.add_edge(h, i, 1.);
    gr.add_edge(h, j, 3.);
    gr.add_edge(i, j, 1.);

    println!("{}", Dot::new(&gr));

    let mst = UnGraph::from_elements(min_spanning_tree(&gr));

    println!("{}", Dot::new(&mst));
    println!("{:?}", Dot::new(&mst));
    println!("MST is:\n{:#?}", mst);
    assert!(mst.node_count() == gr.node_count());
    // |E| = |N| - 2  because there are two disconnected components.
    assert!(mst.edge_count() == gr.node_count() - 2);

    // check the exact edges are there
    assert!(mst.find_edge(a, b).is_some());
    assert!(mst.find_edge(a, d).is_some());
    assert!(mst.find_edge(b, e).is_some());
    assert!(mst.find_edge(e, c).is_some());
    assert!(mst.find_edge(e, g).is_some());
    assert!(mst.find_edge(d, f).is_some());

    assert!(mst.find_edge(h, i).is_some());
    assert!(mst.find_edge(i, j).is_some());

    assert!(mst.find_edge(d, b).is_none());
    assert!(mst.find_edge(b, c).is_none());
}
