extern crate petgraph;

use std::collections::HashSet;
use std::fs::File;
use std::io::prelude::*;

use petgraph::graph::{edge_index, node_index};
use petgraph::prelude::*;
use petgraph::EdgeType;

use petgraph::algo::{
    is_isomorphic, is_isomorphic_matching, is_isomorphic_subgraph, subgraph_isomorphisms_iter,
};

/// Petersen A and B are isomorphic
///
/// http://www.dharwadker.org/tevet/isomorphism/
const PETERSEN_A: &str = "
 0 1 0 0 1 0 1 0 0 0
 1 0 1 0 0 0 0 1 0 0
 0 1 0 1 0 0 0 0 1 0
 0 0 1 0 1 0 0 0 0 1
 1 0 0 1 0 1 0 0 0 0
 0 0 0 0 1 0 0 1 1 0
 1 0 0 0 0 0 0 0 1 1
 0 1 0 0 0 1 0 0 0 1
 0 0 1 0 0 1 1 0 0 0
 0 0 0 1 0 0 1 1 0 0
";

const PETERSEN_B: &str = "
 0 0 0 1 0 1 0 0 0 1
 0 0 0 1 1 0 1 0 0 0
 0 0 0 0 0 0 1 1 0 1
 1 1 0 0 0 0 0 1 0 0
 0 1 0 0 0 0 0 0 1 1
 1 0 0 0 0 0 1 0 1 0
 0 1 1 0 0 1 0 0 0 0
 0 0 1 1 0 0 0 0 1 0
 0 0 0 0 1 1 0 1 0 0
 1 0 1 0 1 0 0 0 0 0
";

/// An almost full set, isomorphic
const FULL_A: &str = "
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 0 1 1 1 0 1
 1 1 1 1 1 1 1 1 1 1
";

const FULL_B: &str = "
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 0 1 1 1 0 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
";

/// Praust A and B are not isomorphic
const PRAUST_A: &str = "
 0 1 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0
 1 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0
 1 1 0 1 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0
 1 1 1 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0
 1 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0
 0 1 0 0 1 0 1 1 0 0 0 0 0 1 0 0 0 0 0 0
 0 0 1 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0
 0 0 0 1 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0
 1 0 0 0 0 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0
 0 1 0 0 0 0 0 0 1 0 1 1 0 0 0 0 0 1 0 0
 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 0 0 0 1 0
 0 0 0 1 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1
 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 1 0 0
 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 1 1 0 0 0
 0 0 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1
 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0
 0 0 0 0 0 0 0 0 1 0 0 0 0 1 0 0 0 1 1 1
 0 0 0 0 0 0 0 0 0 1 0 0 1 0 0 0 1 0 1 1
 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 1
 0 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 0
";

const PRAUST_B: &str = "
 0 1 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0
 1 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0
 1 1 0 1 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0
 1 1 1 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0
 1 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0
 0 1 0 0 1 0 1 1 0 0 0 0 0 0 0 0 0 0 0 1
 0 0 1 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0
 0 0 0 1 1 1 1 0 0 0 0 0 0 0 0 0 0 1 0 0
 1 0 0 0 0 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0
 0 1 0 0 0 0 0 0 1 0 1 1 0 1 0 0 0 0 0 0
 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 0 0 0 1 0
 0 0 0 1 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0 0
 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 0 0 1 0 1
 0 0 0 0 0 0 0 0 0 1 0 0 1 0 0 1 1 0 1 0
 0 0 0 0 0 0 1 0 0 0 0 0 1 0 0 1 0 1 0 1
 0 0 0 0 0 0 0 0 0 0 0 1 0 1 1 0 1 0 1 0
 0 0 0 0 0 0 0 0 1 0 0 0 0 1 0 1 0 1 1 0
 0 0 0 0 0 0 0 1 0 0 0 0 1 0 1 0 1 0 0 1
 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 0 0 1
 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 0 0 1 1 0
";

const G1U: &str = "
0 1 1 0 1
1 0 1 0 0
1 1 0 0 0
0 0 0 0 0
1 0 0 0 0
";

const G2U: &str = "
0 1 0 1 0
1 0 0 1 1
0 0 0 0 0
1 1 0 0 0
0 1 0 0 0
";

const G4U: &str = "
0 1 1 0 1
1 0 0 1 0
1 0 0 0 0
0 1 0 0 0
1 0 0 0 0
";

const G1D: &str = "
0 1 1 0 1
0 0 1 0 0
0 0 0 0 0
0 0 0 0 0
0 0 0 0 0
";

const G4D: &str = "
0 1 1 0 1
0 0 0 1 0
0 0 0 0 0
0 0 0 0 0
0 0 0 0 0
";

// G8 1,2 are not iso
const G8_1: &str = "
0 1 1 0 0 1 1 1
1 0 1 0 1 0 1 1
1 1 0 1 0 0 1 1
0 0 1 0 1 1 1 1
0 1 0 1 0 1 1 1
1 0 0 1 1 0 1 1
1 1 1 1 1 1 0 1
1 1 1 1 1 1 1 0
";

const G8_2: &str = "
0 1 0 1 0 1 1 1
1 0 1 0 1 0 1 1
0 1 0 1 0 1 1 1
1 0 1 0 1 0 1 1
0 1 0 1 0 1 1 1
1 0 1 0 1 0 1 1
1 1 1 1 1 1 0 1
1 1 1 1 1 1 1 0
";

// G3 1,2 are not iso
const G3_1: &str = "
0 1 0
1 0 1
0 1 0
";
const G3_2: &str = "
0 1 1
1 0 1
1 1 0
";

// Non-isomorphic due to selfloop difference
const S1: &str = "
1 1 1
1 0 1
1 0 0
";
const S2: &str = "
1 1 1
0 1 1
1 0 0
";

/// Parse a text adjacency matrix format into a directed graph
fn parse_graph<Ty: EdgeType>(s: &str) -> Graph<(), (), Ty> {
    let mut gr = Graph::with_capacity(0, 0);
    let s = s.trim();
    let lines = s.lines().filter(|l| !l.is_empty());
    for (row, line) in lines.enumerate() {
        for (col, word) in line.split(' ').filter(|s| !s.is_empty()).enumerate() {
            let has_edge = word.parse::<i32>().unwrap();
            assert!(has_edge == 0 || has_edge == 1);
            if has_edge == 0 {
                continue;
            }
            while col >= gr.node_count() || row >= gr.node_count() {
                gr.add_node(());
            }
            gr.update_edge(node_index(row), node_index(col), ());
        }
    }
    gr
}

fn str_to_graph(s: &str) -> Graph<(), (), Undirected> {
    parse_graph(s)
}

fn str_to_digraph(s: &str) -> Graph<(), (), Directed> {
    parse_graph(s)
}

/// Parse a file in adjacency matrix format into a directed graph
fn graph_from_file(path: &str) -> Graph<(), (), Directed> {
    let mut f = File::open(path).expect("file not found");
    let mut contents = String::new();
    f.read_to_string(&mut contents)
        .expect("failed to read from file");
    parse_graph(&contents)
}

/*
fn graph_to_ad_matrix<N, E, Ty: EdgeType>(g: &Graph<N,E,Ty>)
{
    let n = g.node_count();
    for i in (0..n) {
        for j in (0..n) {
            let ix = NodeIndex::new(i);
            let jx = NodeIndex::new(j);
            let out = match g.find_edge(ix, jx) {
                None => "0",
                Some(_) => "1",
            };
            print!("{} ", out);
        }
        println!("");
    }
}
*/

#[test]
fn petersen_iso() {
    // The correct isomorphism is
    // 0 => 0, 1 => 3, 2 => 1, 3 => 4, 5 => 2, 6 => 5, 7 => 7, 8 => 6, 9 => 8, 4 => 9
    let peta = str_to_digraph(PETERSEN_A);
    let petb = str_to_digraph(PETERSEN_B);
    /*
    println!("{:?}", peta);
    graph_to_ad_matrix(&peta);
    println!("");
    graph_to_ad_matrix(&petb);
    */

    assert!(petgraph::algo::is_isomorphic(&peta, &petb));
}

#[test]
fn petersen_undir_iso() {
    // The correct isomorphism is
    // 0 => 0, 1 => 3, 2 => 1, 3 => 4, 5 => 2, 6 => 5, 7 => 7, 8 => 6, 9 => 8, 4 => 9
    let peta = str_to_digraph(PETERSEN_A);
    let petb = str_to_digraph(PETERSEN_B);

    assert!(petgraph::algo::is_isomorphic(&peta, &petb));
}

#[test]
fn full_iso() {
    let a = str_to_graph(FULL_A);
    let b = str_to_graph(FULL_B);

    assert!(petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
#[cfg_attr(miri, ignore = "Takes too long to run in Miri")]
fn praust_dir_no_iso() {
    let a = str_to_digraph(PRAUST_A);
    let b = str_to_digraph(PRAUST_B);

    assert!(!petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
#[cfg_attr(miri, ignore = "Takes too long to run in Miri")]
fn praust_undir_no_iso() {
    let a = str_to_graph(PRAUST_A);
    let b = str_to_graph(PRAUST_B);

    assert!(!petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
fn coxeter_di_iso() {
    // The correct isomorphism is
    let a = str_to_digraph(COXETER_A);
    let b = str_to_digraph(COXETER_B);
    assert!(petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
fn coxeter_undi_iso() {
    // The correct isomorphism is
    let a = str_to_graph(COXETER_A);
    let b = str_to_graph(COXETER_B);
    assert!(petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
fn g14_dir_not_iso() {
    let a = str_to_digraph(G1D);
    let b = str_to_digraph(G4D);
    assert!(!petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
fn g14_undir_not_iso() {
    let a = str_to_digraph(G1U);
    let b = str_to_digraph(G4U);
    assert!(!petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
fn g12_undir_iso() {
    let a = str_to_digraph(G1U);
    let b = str_to_digraph(G2U);
    assert!(petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
fn g3_not_iso() {
    let a = str_to_digraph(G3_1);
    let b = str_to_digraph(G3_2);
    assert!(!petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
fn g8_not_iso() {
    let a = str_to_digraph(G8_1);
    let b = str_to_digraph(G8_2);
    assert_eq!(a.edge_count(), b.edge_count());
    assert_eq!(a.node_count(), b.node_count());
    assert!(!petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
fn s12_not_iso() {
    let a = str_to_digraph(S1);
    let b = str_to_digraph(S2);
    assert_eq!(a.edge_count(), b.edge_count());
    assert_eq!(a.node_count(), b.node_count());
    assert!(!petgraph::algo::is_isomorphic(&a, &b));
}

#[test]
fn iso1() {
    let mut g0 = Graph::<_, ()>::new();
    let mut g1 = Graph::<_, ()>::new();
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));

    // very simple cases
    let a0 = g0.add_node(0);
    let a1 = g1.add_node(0);
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
    let b0 = g0.add_node(1);
    let b1 = g1.add_node(1);
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
    let _ = g0.add_node(2);
    assert!(!petgraph::algo::is_isomorphic(&g0, &g1));
    let _ = g1.add_node(2);
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
    g0.add_edge(a0, b0, ());
    assert!(!petgraph::algo::is_isomorphic(&g0, &g1));
    g1.add_edge(a1, b1, ());
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
}

#[test]
fn iso2() {
    let mut g0 = Graph::<_, ()>::new();
    let mut g1 = Graph::<_, ()>::new();

    let a0 = g0.add_node(0);
    let a1 = g1.add_node(0);
    let b0 = g0.add_node(1);
    let b1 = g1.add_node(1);
    let c0 = g0.add_node(2);
    let c1 = g1.add_node(2);
    g0.add_edge(a0, b0, ());
    g1.add_edge(c1, b1, ());
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
    // a -> b
    // a -> c
    // vs.
    // c -> b
    // c -> a
    g0.add_edge(a0, c0, ());
    g1.add_edge(c1, a1, ());
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));

    // add
    // b -> c
    // vs
    // b -> a

    let _ = g0.add_edge(b0, c0, ());
    let _ = g1.add_edge(b1, a1, ());
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
    let d0 = g0.add_node(3);
    let d1 = g1.add_node(3);
    let e0 = g0.add_node(4);
    let e1 = g1.add_node(4);
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
    // add
    // b -> e -> d
    // vs
    // b -> d -> e
    g0.add_edge(b0, e0, ());
    g0.add_edge(e0, d0, ());
    g1.add_edge(b1, d1, ());
    g1.add_edge(d1, e1, ());
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
}

#[test]
fn iso_matching() {
    let g0 = Graph::<(), _>::from_edges(&[(0, 0, 1), (0, 1, 2), (0, 2, 3), (1, 2, 4)]);

    let mut g1 = g0.clone();
    g1[edge_index(0)] = 0;
    assert!(!is_isomorphic_matching(
        &g0,
        &g1,
        |x, y| x == y,
        |x, y| x == y
    ));
    let mut g2 = g0.clone();
    g2[edge_index(1)] = 0;
    assert!(!is_isomorphic_matching(
        &g0,
        &g2,
        |x, y| x == y,
        |x, y| x == y
    ));
}

#[test]
fn iso_100n_100e() {
    let g0 = str_to_digraph(include_str!("res/graph_100n_100e.txt"));
    let g1 = str_to_digraph(include_str!("res/graph_100n_100e_iso.txt"));
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
}

#[test]
#[cfg_attr(miri, ignore = "Too large for Miri")]
fn iso_large() {
    let g0 = graph_from_file("tests/res/graph_1000n_1000e.txt");
    let g1 = graph_from_file("tests/res/graph_1000n_1000e.txt");
    assert!(petgraph::algo::is_isomorphic(&g0, &g1));
}

// isomorphism isn't correct for multigraphs.
// Keep this testcase to document how
#[should_panic]
#[test]
fn iso_multigraph_failure() {
    let g0 = Graph::<(), ()>::from_edges(&[(0, 0), (0, 0), (0, 1), (1, 1), (1, 1), (1, 0)]);

    let g1 = Graph::<(), ()>::from_edges(&[(0, 0), (0, 1), (0, 1), (1, 1), (1, 0), (1, 0)]);
    assert!(!is_isomorphic(&g0, &g1));
}

#[test]
#[cfg_attr(miri, ignore = "Takes too long to run in Miri")]
fn iso_subgraph() {
    let g0 = Graph::<(), ()>::from_edges(&[(0, 1), (1, 2), (2, 0)]);
    let g1 = Graph::<(), ()>::from_edges(&[(0, 1), (1, 2), (2, 0), (2, 3), (0, 4)]);
    assert!(!is_isomorphic(&g0, &g1));
    assert!(is_isomorphic_subgraph(&g0, &g1));
}

#[test]
#[cfg_attr(miri, ignore = "Takes too long to run in Miri")]
fn iter_subgraph() {
    let a = Graph::<(), ()>::from_edges(&[(0, 1), (1, 2), (2, 0)]);
    let b = Graph::<(), ()>::from_edges(&[(0, 1), (1, 2), (2, 0), (2, 3), (0, 4)]);
    let a_ref = &a;
    let b_ref = &b;
    let mut node_match = { |x: &(), y: &()| x == y };
    let mut edge_match = { |x: &(), y: &()| x == y };

    let mappings =
        subgraph_isomorphisms_iter(&a_ref, &b_ref, &mut node_match, &mut edge_match).unwrap();

    // Verify the iterator returns the expected mappings
    let expected_mappings: Vec<Vec<usize>> = vec![vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]];
    for mapping in mappings {
        assert!(expected_mappings.contains(&mapping))
    }

    // Verify all the mappings from the iterator are different
    let a = str_to_digraph(COXETER_A);
    let b = str_to_digraph(COXETER_B);
    let a_ref = &a;
    let b_ref = &b;

    let mut unique = HashSet::new();
    assert!(
        subgraph_isomorphisms_iter(&a_ref, &b_ref, &mut node_match, &mut edge_match)
            .unwrap()
            .all(|x| unique.insert(x))
    );

    // The iterator should return None for graphs that are not isomorphic
    let a = str_to_digraph(G8_1);
    let b = str_to_digraph(G8_2);
    let a_ref = &a;
    let b_ref = &b;

    assert!(
        subgraph_isomorphisms_iter(&a_ref, &b_ref, &mut node_match, &mut edge_match)
            .unwrap()
            .next()
            .is_none()
    );

    // https://github.com/petgraph/petgraph/issues/534
    let mut g = Graph::<String, ()>::new();
    let e1 = g.add_node("l1".to_string());
    let e2 = g.add_node("l2".to_string());
    g.add_edge(e1, e2, ());
    let e3 = g.add_node("l3".to_string());
    g.add_edge(e2, e3, ());
    let e4 = g.add_node("l4".to_string());
    g.add_edge(e3, e4, ());

    let mut sub = Graph::<String, ()>::new();
    let e3 = sub.add_node("l3".to_string());
    let e4 = sub.add_node("l4".to_string());
    sub.add_edge(e3, e4, ());

    let mut node_match = { |x: &String, y: &String| x == y };
    let mut edge_match = { |x: &(), y: &()| x == y };
    assert_eq!(
        subgraph_isomorphisms_iter(&&sub, &&g, &mut node_match, &mut edge_match)
            .unwrap()
            .collect::<Vec<_>>(),
        vec![vec![2, 3]]
    );
}

/// Isomorphic pair
const COXETER_A: &str = "
 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 1
 1 0 1 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0
 0 0 1 0 1 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0
 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0
 0 0 0 0 0 0 1 0 1 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 1 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0
 0 0 0 1 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 1 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 1
 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 1 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0
 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0
 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 1 0 1 0 0 0 0
 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0 1 0 0
 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0
 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1
 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0
";

const COXETER_B: &str = "
 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 1 0 0
 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 1 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 1 0 0 1 0 0 0 0 0 0
 0 0 0 0 0 0 1 0 0 1 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 1 0 0 0 0 0 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1 0 0 0 0 0 0 0 0 0 1 0 0 0
 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 1 0 0 0 0 0
 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0
 0 0 0 0 1 0 0 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 1 0 0 0 0 0 0 0 0 1 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0
 0 0 0 0 0 1 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0
 0 0 0 0 0 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0
 1 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 1 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 1 1 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0
 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1
 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 1
 0 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0
 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0
 0 1 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0
 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0
 1 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 1 0 0 1
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 1 0
";
