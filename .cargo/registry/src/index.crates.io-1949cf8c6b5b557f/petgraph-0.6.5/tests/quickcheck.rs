#![cfg(feature = "quickcheck")]
#[macro_use]
extern crate quickcheck;
extern crate petgraph;
extern crate rand;
#[macro_use]
extern crate defmac;

extern crate itertools;
extern crate odds;

mod utils;

use utils::{Small, Tournament};

use odds::prelude::*;
use std::collections::HashSet;
use std::hash::Hash;

use itertools::assert_equal;
use itertools::cloned;
use quickcheck::{Arbitrary, Gen};
use rand::Rng;

use petgraph::algo::{
    bellman_ford, condensation, dijkstra, find_negative_cycle, floyd_warshall, ford_fulkerson,
    greedy_feedback_arc_set, greedy_matching, is_cyclic_directed, is_cyclic_undirected,
    is_isomorphic, is_isomorphic_matching, k_shortest_path, kosaraju_scc, maximum_matching,
    min_spanning_tree, page_rank, tarjan_scc, toposort, Matching,
};
use petgraph::data::FromElements;
use petgraph::dot::{Config, Dot};
use petgraph::graph::{edge_index, node_index, IndexType};
use petgraph::graphmap::NodeTrait;
use petgraph::operator::complement;
use petgraph::prelude::*;
use petgraph::visit::{
    EdgeFiltered, EdgeIndexable, IntoEdgeReferences, IntoEdges, IntoNeighbors, IntoNodeIdentifiers,
    IntoNodeReferences, NodeCount, NodeIndexable, Reversed, Topo, VisitMap, Visitable,
};
use petgraph::EdgeType;

fn mst_graph<N, E, Ty, Ix>(g: &Graph<N, E, Ty, Ix>) -> Graph<N, E, Undirected, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
    N: Clone,
    E: Clone + PartialOrd,
{
    Graph::from_elements(min_spanning_tree(&g))
}

use std::fmt;

quickcheck! {
    fn mst_directed(g: Small<Graph<(), u32>>) -> bool {
        // filter out isolated nodes
        let no_singles = g.filter_map(
            |nx, w| g.neighbors_undirected(nx).next().map(|_| w),
            |_, w| Some(w));
        for i in no_singles.node_indices() {
            assert!(no_singles.neighbors_undirected(i).count() > 0);
        }
        assert_eq!(no_singles.edge_count(), g.edge_count());
        let mst = mst_graph(&no_singles);
        assert!(!is_cyclic_undirected(&mst));
        true
    }
}

quickcheck! {
    fn mst_undirected(g: Graph<(), u32, Undirected>) -> bool {
        // filter out isolated nodes
        let no_singles = g.filter_map(
            |nx, w| g.neighbors_undirected(nx).next().map(|_| w),
            |_, w| Some(w));
        for i in no_singles.node_indices() {
            assert!(no_singles.neighbors_undirected(i).count() > 0);
        }
        assert_eq!(no_singles.edge_count(), g.edge_count());
        let mst = mst_graph(&no_singles);
        assert!(!is_cyclic_undirected(&mst));
        true
    }
}

quickcheck! {
    fn reverse_undirected(g: Small<UnGraph<(), ()>>) -> bool {
        let mut h = (*g).clone();
        h.reverse();
        is_isomorphic(&*g, &h)
    }
}

fn assert_graph_consistent<N, E, Ty, Ix>(g: &Graph<N, E, Ty, Ix>)
where
    Ty: EdgeType,
    Ix: IndexType,
{
    assert_eq!(g.node_count(), g.node_indices().count());
    assert_eq!(g.edge_count(), g.edge_indices().count());
    for edge in g.raw_edges() {
        assert!(
            g.find_edge(edge.source(), edge.target()).is_some(),
            "Edge not in graph! {:?} to {:?}",
            edge.source(),
            edge.target()
        );
    }
}

#[test]
fn reverse_directed() {
    fn prop<Ty: EdgeType>(mut g: Graph<(), (), Ty>) -> bool {
        let node_outdegrees = g
            .node_indices()
            .map(|i| g.neighbors_directed(i, Outgoing).count())
            .collect::<Vec<_>>();
        let node_indegrees = g
            .node_indices()
            .map(|i| g.neighbors_directed(i, Incoming).count())
            .collect::<Vec<_>>();

        g.reverse();
        let new_outdegrees = g
            .node_indices()
            .map(|i| g.neighbors_directed(i, Outgoing).count())
            .collect::<Vec<_>>();
        let new_indegrees = g
            .node_indices()
            .map(|i| g.neighbors_directed(i, Incoming).count())
            .collect::<Vec<_>>();
        assert_eq!(node_outdegrees, new_indegrees);
        assert_eq!(node_indegrees, new_outdegrees);
        assert_graph_consistent(&g);
        true
    }
    quickcheck::quickcheck(prop as fn(Graph<_, _, Directed>) -> bool);
}

#[test]
fn graph_retain_nodes() {
    fn prop<Ty: EdgeType>(mut g: Graph<i32, i32, Ty>) -> bool {
        // Remove all negative nodes, these should be randomly spread
        let og = g.clone();
        let nodes = g.node_count();
        let num_negs = g.raw_nodes().iter().filter(|n| n.weight < 0).count();
        let mut removed = 0;
        g.retain_nodes(|g, i| {
            let keep = g[i] >= 0;
            if !keep {
                removed += 1;
            }
            keep
        });
        let num_negs_post = g.raw_nodes().iter().filter(|n| n.weight < 0).count();
        let num_pos_post = g.raw_nodes().iter().filter(|n| n.weight >= 0).count();
        assert_eq!(num_negs_post, 0);
        assert_eq!(removed, num_negs);
        assert_eq!(num_negs + g.node_count(), nodes);
        assert_eq!(num_pos_post, g.node_count());

        // check against filter_map
        let filtered = og.filter_map(
            |_, w| if *w >= 0 { Some(*w) } else { None },
            |_, w| Some(*w),
        );
        assert_eq!(g.node_count(), filtered.node_count());
        /*
        println!("Iso of graph with nodes={}, edges={}",
                 g.node_count(), g.edge_count());
                 */
        assert!(is_isomorphic_matching(
            &filtered,
            &g,
            PartialEq::eq,
            PartialEq::eq
        ));

        true
    }
    quickcheck::quickcheck(prop as fn(Graph<_, _, Directed>) -> bool);
    quickcheck::quickcheck(prop as fn(Graph<_, _, Undirected>) -> bool);
}

#[test]
fn graph_retain_edges() {
    fn prop<Ty: EdgeType>(mut g: Graph<(), i32, Ty>) -> bool {
        // Remove all negative edges, these should be randomly spread
        let og = g.clone();
        let edges = g.edge_count();
        let num_negs = g.raw_edges().iter().filter(|n| n.weight < 0).count();
        let mut removed = 0;
        g.retain_edges(|g, i| {
            let keep = g[i] >= 0;
            if !keep {
                removed += 1;
            }
            keep
        });
        let num_negs_post = g.raw_edges().iter().filter(|n| n.weight < 0).count();
        let num_pos_post = g.raw_edges().iter().filter(|n| n.weight >= 0).count();
        assert_eq!(num_negs_post, 0);
        assert_eq!(removed, num_negs);
        assert_eq!(num_negs + g.edge_count(), edges);
        assert_eq!(num_pos_post, g.edge_count());
        if og.edge_count() < 30 {
            // check against filter_map
            let filtered = og.filter_map(
                |_, w| Some(*w),
                |_, w| if *w >= 0 { Some(*w) } else { None },
            );
            assert_eq!(g.node_count(), filtered.node_count());
            assert!(is_isomorphic(&filtered, &g));
        }
        true
    }
    quickcheck::quickcheck(prop as fn(Graph<_, _, Directed>) -> bool);
    quickcheck::quickcheck(prop as fn(Graph<_, _, Undirected>) -> bool);
}

#[test]
fn stable_graph_retain_edges() {
    fn prop<Ty: EdgeType>(mut g: StableGraph<(), i32, Ty>) -> bool {
        // Remove all negative edges, these should be randomly spread
        let og = g.clone();
        let edges = g.edge_count();
        let num_negs = g.edge_references().filter(|n| *n.weight() < 0).count();
        let mut removed = 0;
        g.retain_edges(|g, i| {
            let keep = g[i] >= 0;
            if !keep {
                removed += 1;
            }
            keep
        });
        let num_negs_post = g.edge_references().filter(|n| *n.weight() < 0).count();
        let num_pos_post = g.edge_references().filter(|n| *n.weight() >= 0).count();
        assert_eq!(num_negs_post, 0);
        assert_eq!(removed, num_negs);
        assert_eq!(num_negs + g.edge_count(), edges);
        assert_eq!(num_pos_post, g.edge_count());
        if og.edge_count() < 30 {
            // check against filter_map
            let filtered = og.filter_map(
                |_, w| Some(*w),
                |_, w| if *w >= 0 { Some(*w) } else { None },
            );
            assert_eq!(g.node_count(), filtered.node_count());
        }
        true
    }
    quickcheck::quickcheck(prop as fn(StableGraph<_, _, Directed>) -> bool);
    quickcheck::quickcheck(prop as fn(StableGraph<_, _, Undirected>) -> bool);
}

#[test]
fn isomorphism_1() {
    // using small weights so that duplicates are likely
    fn prop<Ty: EdgeType>(g: Small<Graph<i8, i8, Ty>>) -> bool {
        let mut rng = rand::thread_rng();
        // several trials of different isomorphisms of the same graph
        // mapping of node indices
        let mut map = g.node_indices().collect::<Vec<_>>();
        let mut ng = Graph::<_, _, Ty>::with_capacity(g.node_count(), g.edge_count());
        for _ in 0..1 {
            rng.shuffle(&mut map);
            ng.clear();

            for _ in g.node_indices() {
                ng.add_node(0);
            }
            // Assign node weights
            for i in g.node_indices() {
                ng[map[i.index()]] = g[i];
            }
            // Add edges
            for i in g.edge_indices() {
                let (s, t) = g.edge_endpoints(i).unwrap();
                ng.add_edge(map[s.index()], map[t.index()], g[i]);
            }
            if g.node_count() < 20 && g.edge_count() < 50 {
                assert!(is_isomorphic(&*g, &ng));
            }
            assert!(is_isomorphic_matching(
                &*g,
                &ng,
                PartialEq::eq,
                PartialEq::eq
            ));
        }
        true
    }
    quickcheck::quickcheck(prop::<Undirected> as fn(_) -> bool);
    quickcheck::quickcheck(prop::<Directed> as fn(_) -> bool);
}

#[test]
fn isomorphism_modify() {
    // using small weights so that duplicates are likely
    fn prop<Ty: EdgeType>(g: Small<Graph<i16, i8, Ty>>, node: u8, edge: u8) -> bool {
        println!("graph {:#?}", g);
        let mut ng = (*g).clone();
        let i = node_index(node as usize);
        let j = edge_index(edge as usize);
        if i.index() < g.node_count() {
            ng[i] = (g[i] == 0) as i16;
        }
        if j.index() < g.edge_count() {
            ng[j] = (g[j] == 0) as i8;
        }
        if i.index() < g.node_count() || j.index() < g.edge_count() {
            assert!(!is_isomorphic_matching(
                &*g,
                &ng,
                PartialEq::eq,
                PartialEq::eq
            ));
        } else {
            assert!(is_isomorphic_matching(
                &*g,
                &ng,
                PartialEq::eq,
                PartialEq::eq
            ));
        }
        true
    }
    quickcheck::quickcheck(prop::<Undirected> as fn(_, _, _) -> bool);
    quickcheck::quickcheck(prop::<Directed> as fn(_, _, _) -> bool);
}

#[test]
fn graph_remove_edge() {
    fn prop<Ty: EdgeType>(mut g: Graph<(), (), Ty>, a: u8, b: u8) -> bool {
        let a = node_index(a as usize);
        let b = node_index(b as usize);
        let edge = g.find_edge(a, b);
        if !g.is_directed() {
            assert_eq!(edge.is_some(), g.find_edge(b, a).is_some());
        }
        if let Some(ex) = edge {
            assert!(g.remove_edge(ex).is_some());
        }
        assert_graph_consistent(&g);
        assert!(g.find_edge(a, b).is_none());
        assert!(g.neighbors(a).find(|x| *x == b).is_none());
        if !g.is_directed() {
            assert!(g.neighbors(b).find(|x| *x == a).is_none());
        }
        true
    }
    quickcheck::quickcheck(prop as fn(Graph<_, _, Undirected>, _, _) -> bool);
    quickcheck::quickcheck(prop as fn(Graph<_, _, Directed>, _, _) -> bool);
}

#[cfg(feature = "stable_graph")]
#[test]
fn stable_graph_remove_edge() {
    fn prop<Ty: EdgeType>(mut g: StableGraph<(), (), Ty>, a: u8, b: u8) -> bool {
        let a = node_index(a as usize);
        let b = node_index(b as usize);
        let edge = g.find_edge(a, b);
        if !g.is_directed() {
            assert_eq!(edge.is_some(), g.find_edge(b, a).is_some());
        }
        if let Some(ex) = edge {
            assert!(g.remove_edge(ex).is_some());
        }
        //assert_graph_consistent(&g);
        assert!(g.find_edge(a, b).is_none());
        assert!(g.neighbors(a).find(|x| *x == b).is_none());
        if !g.is_directed() {
            assert!(g.find_edge(b, a).is_none());
            assert!(g.neighbors(b).find(|x| *x == a).is_none());
        }
        true
    }
    quickcheck::quickcheck(prop as fn(StableGraph<_, _, Undirected>, _, _) -> bool);
    quickcheck::quickcheck(prop as fn(StableGraph<_, _, Directed>, _, _) -> bool);
}

#[cfg(feature = "stable_graph")]
#[test]
fn stable_graph_add_remove_edges() {
    fn prop<Ty: EdgeType>(mut g: StableGraph<(), (), Ty>, edges: Vec<(u8, u8)>) -> bool {
        for &(a, b) in &edges {
            let a = node_index(a as usize);
            let b = node_index(b as usize);
            let edge = g.find_edge(a, b);

            if edge.is_none() && g.contains_node(a) && g.contains_node(b) {
                let _index = g.add_edge(a, b, ());
                continue;
            }

            if !g.is_directed() {
                assert_eq!(edge.is_some(), g.find_edge(b, a).is_some());
            }
            if let Some(ex) = edge {
                assert!(g.remove_edge(ex).is_some());
            }
            //assert_graph_consistent(&g);
            assert!(
                g.find_edge(a, b).is_none(),
                "failed to remove edge {:?} from graph {:?}",
                (a, b),
                g
            );
            assert!(g.neighbors(a).find(|x| *x == b).is_none());
            if !g.is_directed() {
                assert!(g.find_edge(b, a).is_none());
                assert!(g.neighbors(b).find(|x| *x == a).is_none());
            }
        }
        true
    }
    quickcheck::quickcheck(prop as fn(StableGraph<_, _, Undirected>, _) -> bool);
    quickcheck::quickcheck(prop as fn(StableGraph<_, _, Directed>, _) -> bool);
}

fn assert_graphmap_consistent<N, E, Ty>(g: &GraphMap<N, E, Ty>)
where
    Ty: EdgeType,
    N: NodeTrait + fmt::Debug,
{
    for (a, b, _weight) in g.all_edges() {
        assert!(
            g.contains_edge(a, b),
            "Edge not in graph! {:?} to {:?}",
            a,
            b
        );
        assert!(
            g.neighbors(a).find(|x| *x == b).is_some(),
            "Edge {:?} not in neighbor list for {:?}",
            (a, b),
            a
        );
        if !g.is_directed() {
            assert!(
                g.neighbors(b).find(|x| *x == a).is_some(),
                "Edge {:?} not in neighbor list for {:?}",
                (b, a),
                b
            );
        }
    }
}

#[test]
fn graphmap_remove() {
    fn prop<Ty: EdgeType>(mut g: GraphMap<i8, (), Ty>, a: i8, b: i8) -> bool {
        //if g.edge_count() > 20 { return true; }
        assert_graphmap_consistent(&g);
        let contains = g.contains_edge(a, b);
        if !g.is_directed() {
            assert_eq!(contains, g.contains_edge(b, a));
        }
        assert_eq!(g.remove_edge(a, b).is_some(), contains);
        assert!(!g.contains_edge(a, b) && g.neighbors(a).find(|x| *x == b).is_none());
        //(g.is_directed() || g.neighbors(b).find(|x| *x == a).is_none()));
        assert!(g.remove_edge(a, b).is_none());
        assert_graphmap_consistent(&g);
        true
    }
    quickcheck::quickcheck(prop as fn(DiGraphMap<_, _>, _, _) -> bool);
    quickcheck::quickcheck(prop as fn(UnGraphMap<_, _>, _, _) -> bool);
}

#[test]
fn graphmap_add_remove() {
    fn prop(mut g: UnGraphMap<i8, ()>, a: i8, b: i8) -> bool {
        assert_eq!(g.contains_edge(a, b), g.add_edge(a, b, ()).is_some());
        g.remove_edge(a, b);
        !g.contains_edge(a, b)
            && g.neighbors(a).find(|x| *x == b).is_none()
            && g.neighbors(b).find(|x| *x == a).is_none()
    }
    quickcheck::quickcheck(prop as fn(_, _, _) -> bool);
}

fn sort_sccs<T: Ord>(v: &mut [Vec<T>]) {
    for scc in &mut *v {
        scc.sort();
    }
    v.sort();
}

quickcheck! {
    fn graph_sccs(g: Graph<(), ()>) -> bool {
        let mut sccs = kosaraju_scc(&g);
        let mut tsccs = tarjan_scc(&g);
        sort_sccs(&mut sccs);
        sort_sccs(&mut tsccs);
        if sccs != tsccs {
            println!("{:?}",
                     Dot::with_config(&g, &[Config::EdgeNoLabel,
                                      Config::NodeIndexLabel]));
            println!("Sccs {:?}", sccs);
            println!("Sccs (Tarjan) {:?}", tsccs);
            return false;
        }
        true
    }
}

quickcheck! {
    fn kosaraju_scc_is_topo_sort(g: Graph<(), ()>) -> bool {
        let tsccs = kosaraju_scc(&g);
        let firsts = tsccs.iter().rev().map(|v| v[0]).collect::<Vec<_>>();
        subset_is_topo_order(&g, &firsts)
    }
}

quickcheck! {
    fn tarjan_scc_is_topo_sort(g: Graph<(), ()>) -> bool {
        let tsccs = tarjan_scc(&g);
        let firsts = tsccs.iter().rev().map(|v| v[0]).collect::<Vec<_>>();
        subset_is_topo_order(&g, &firsts)
    }
}

quickcheck! {
    // Reversed edges gives the same sccs (when sorted)
    fn graph_reverse_sccs(g: Graph<(), ()>) -> bool {
        let mut sccs = kosaraju_scc(&g);
        let mut tsccs = kosaraju_scc(Reversed(&g));
        sort_sccs(&mut sccs);
        sort_sccs(&mut tsccs);
        if sccs != tsccs {
            println!("{:?}",
                     Dot::with_config(&g, &[Config::EdgeNoLabel,
                                      Config::NodeIndexLabel]));
            println!("Sccs {:?}", sccs);
            println!("Sccs (Reversed) {:?}", tsccs);
            return false;
        }
        true
    }
}

quickcheck! {
    // Reversed edges gives the same sccs (when sorted)
    fn graphmap_reverse_sccs(g: DiGraphMap<u16, ()>) -> bool {
        let mut sccs = kosaraju_scc(&g);
        let mut tsccs = kosaraju_scc(Reversed(&g));
        sort_sccs(&mut sccs);
        sort_sccs(&mut tsccs);
        if sccs != tsccs {
            println!("{:?}",
                     Dot::with_config(&g, &[Config::EdgeNoLabel,
                                      Config::NodeIndexLabel]));
            println!("Sccs {:?}", sccs);
            println!("Sccs (Reversed) {:?}", tsccs);
            return false;
        }
        true
    }
}

#[test]
fn graph_condensation_acyclic() {
    fn prop(g: Graph<(), ()>) -> bool {
        !is_cyclic_directed(&condensation(g, /* make_acyclic */ true))
    }
    quickcheck::quickcheck(prop as fn(_) -> bool);
}

#[derive(Debug, Clone)]
struct DAG<N: Default + Clone + Send + 'static>(Graph<N, ()>);

impl<N: Default + Clone + Send + 'static> Arbitrary for DAG<N> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let nodes = usize::arbitrary(g);
        if nodes == 0 {
            return DAG(Graph::with_capacity(0, 0));
        }
        let split = g.gen_range(0., 1.);
        let max_width = f64::sqrt(nodes as f64) as usize;
        let tall = (max_width as f64 * split) as usize;
        let fat = max_width - tall;

        let edge_prob = 1. - (1. - g.gen_range(0., 1.)) * (1. - g.gen_range(0., 1.));
        let edges = ((nodes as f64).powi(2) * edge_prob) as usize;
        let mut gr = Graph::with_capacity(nodes, edges);
        let mut nodes = 0;
        for _ in 0..tall {
            let cur_nodes = g.gen_range(0, fat);
            for _ in 0..cur_nodes {
                gr.add_node(N::default());
            }
            for j in 0..nodes {
                for k in 0..cur_nodes {
                    if g.gen_range(0., 1.) < edge_prob {
                        gr.add_edge(NodeIndex::new(j), NodeIndex::new(k + nodes), ());
                    }
                }
            }
            nodes += cur_nodes;
        }
        DAG(gr)
    }

    // shrink the graph by splitting it in two by a very
    // simple algorithm, just even and odd node indices
    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        let self_ = self.clone();
        Box::new((0..2).filter_map(move |x| {
            let gr = self_.0.filter_map(
                |i, w| {
                    if i.index() % 2 == x {
                        Some(w.clone())
                    } else {
                        None
                    }
                },
                |_, w| Some(w.clone()),
            );
            // make sure we shrink
            if gr.node_count() < self_.0.node_count() {
                Some(DAG(gr))
            } else {
                None
            }
        }))
    }
}

fn is_topo_order<N>(gr: &Graph<N, (), Directed>, order: &[NodeIndex]) -> bool {
    if gr.node_count() != order.len() {
        println!(
            "Graph ({}) and count ({}) had different amount of nodes.",
            gr.node_count(),
            order.len()
        );
        return false;
    }
    // check all the edges of the graph
    for edge in gr.raw_edges() {
        let a = edge.source();
        let b = edge.target();
        let ai = order.find(&a).unwrap();
        let bi = order.find(&b).unwrap();
        if ai >= bi {
            println!("{:?} > {:?} ", a, b);
            return false;
        }
    }
    true
}

fn subset_is_topo_order<N>(gr: &Graph<N, (), Directed>, order: &[NodeIndex]) -> bool {
    if gr.node_count() < order.len() {
        println!(
            "Graph (len={}) had less nodes than order (len={})",
            gr.node_count(),
            order.len()
        );
        return false;
    }
    // check all the edges of the graph
    for edge in gr.raw_edges() {
        let a = edge.source();
        let b = edge.target();
        if a == b {
            continue;
        }
        // skip those that are not in the subset
        let ai = match order.find(&a) {
            Some(i) => i,
            None => continue,
        };
        let bi = match order.find(&b) {
            Some(i) => i,
            None => continue,
        };
        if ai >= bi {
            println!("{:?} > {:?} ", a, b);
            return false;
        }
    }
    true
}

#[test]
fn full_topo() {
    fn prop(DAG(gr): DAG<()>) -> bool {
        let order = toposort(&gr, None).unwrap();
        is_topo_order(&gr, &order)
    }
    quickcheck::quickcheck(prop as fn(_) -> bool);
}

#[test]
fn full_topo_generic() {
    fn prop_generic(DAG(mut gr): DAG<usize>) -> bool {
        assert!(!is_cyclic_directed(&gr));
        let mut index = 0;
        let mut topo = Topo::new(&gr);
        while let Some(nx) = topo.next(&gr) {
            gr[nx] = index;
            index += 1;
        }

        let mut order = Vec::new();
        index = 0;
        let mut topo = Topo::new(&gr);
        while let Some(nx) = topo.next(&gr) {
            order.push(nx);
            assert_eq!(gr[nx], index);
            index += 1;
        }
        if !is_topo_order(&gr, &order) {
            println!("{:?}", gr);
            return false;
        }

        {
            order.clear();
            let mut topo = Topo::new(&gr);
            while let Some(nx) = topo.next(&gr) {
                order.push(nx);
            }
            if !is_topo_order(&gr, &order) {
                println!("{:?}", gr);
                return false;
            }
        }

        {
            order.clear();
            let init_nodes = gr.node_identifiers().filter(|n| {
                gr.neighbors_directed(n.clone(), Direction::Incoming)
                    .next()
                    .is_none()
            });
            let mut topo = Topo::with_initials(&gr, init_nodes);
            while let Some(nx) = topo.next(&gr) {
                order.push(nx);
            }
            if !is_topo_order(&gr, &order) {
                println!("{:?}", gr);
                return false;
            }
        }

        {
            order.clear();
            let mut topo = Topo::with_initials(&gr, gr.node_identifiers());
            while let Some(nx) = topo.next(&gr) {
                order.push(nx);
            }
            if !is_topo_order(&gr, &order) {
                println!("{:?}", gr);
                return false;
            }
        }
        true
    }
    quickcheck::quickcheck(prop_generic as fn(_) -> bool);
}

quickcheck! {
    // checks that the distances computed by dijkstra satisfy the triangle
    // inequality.
    fn dijkstra_triangle_ineq(g: Graph<u32, u32>, node: usize) -> bool {
        if g.node_count() == 0 {
            return true;
        }
        let v = node_index(node % g.node_count());
        let distances = dijkstra(&g, v, None, |e| *e.weight());
        for v2 in distances.keys() {
            let dv2 = distances[v2];
            // triangle inequality:
            // d(v,u) <= d(v,v2) + w(v2,u)
            for edge in g.edges(*v2) {
                let u = edge.target();
                let w = edge.weight();
                if distances.contains_key(&u) && distances[&u] > dv2 + w {
                    return false;
                }
            }
        }
        true
    }
}

quickcheck! {
    // checks that the distances computed by k'th shortest path is always greater or equal compared to their dijkstra computation
    fn k_shortest_path_(g: Graph<u32, u32>, node: usize) -> bool {
        if g.node_count() == 0 {
            return true;
        }
        let v = node_index(node % g.node_count());
        let second_best_distances = k_shortest_path(&g, v, None, 2, |e| *e.weight());
        let dijkstra_distances = dijkstra(&g, v, None, |e| *e.weight());
        for v in second_best_distances.keys() {
            if second_best_distances[&v] < dijkstra_distances[&v] {
                return false;
            }
        }
        true
    }
}

quickcheck! {
    // checks floyd_warshall against dijkstra results
    fn floyd_warshall_(g: Graph<u32, u32>) -> bool {
        if g.node_count() == 0 {
            return true;
        }

        let fw_res = floyd_warshall(&g, |e| *e.weight()).unwrap();

        for node1 in g.node_identifiers() {
            let dijkstra_res = dijkstra(&g, node1, None, |e| *e.weight());

            for node2 in g.node_identifiers() {
                // if dijkstra found a path then the results must be same
                if let Some(distance) = dijkstra_res.get(&node2) {
                    let floyd_distance = fw_res.get(&(node1, node2)).unwrap();
                    if distance != floyd_distance {
                        return false;
                    }
                } else {
                    // if there are no path between two nodes then floyd_warshall will return maximum value possible
                    if *fw_res.get(&(node1, node2)).unwrap() != u32::MAX {
                        return false;
                    }
                }
            }
         }
        true
    }
}

quickcheck! {
    // checks that the complement of the complement is the same as the input if the input does not contain self-loops
    fn complement_(g: Graph<u32, u32>, _node: usize) -> bool {
        if g.node_count() == 0 {
            return true;
        }
        for x in g.node_indices() {
            if g.contains_edge(x, x) {
                return true;
            }
        }
        let mut complement_graph: Graph<u32, u32>  = Graph::new();
        let mut result: Graph<u32, u32> = Graph::new();
        complement(&g, &mut complement_graph, 0);
        complement(&complement_graph, &mut result, 0);

        for x in g.node_indices() {
            for y in g.node_indices() {
                if g.contains_edge(x, y) != result.contains_edge(x, y){
                    return false;
                }
            }
        }
        true
    }
}

fn set<I>(iter: I) -> HashSet<I::Item>
where
    I: IntoIterator,
    I::Item: Hash + Eq,
{
    iter.into_iter().collect()
}

quickcheck! {
    fn dfs_visit(gr: Graph<(), ()>, node: usize) -> bool {
        use petgraph::visit::{Visitable, VisitMap};
        use petgraph::visit::DfsEvent::*;
        use petgraph::visit::{Time, depth_first_search};
        if gr.node_count() == 0 {
            return true;
        }
        let start_node = node_index(node % gr.node_count());

        let invalid_time = Time(!0);
        let mut discover_time = vec![invalid_time; gr.node_count()];
        let mut finish_time = vec![invalid_time; gr.node_count()];
        let mut has_tree_edge = gr.visit_map();
        let mut edges = HashSet::new();
        depth_first_search(&gr, Some(start_node).into_iter().chain(gr.node_indices()),
                           |evt| {
            match evt {
                Discover(n, t) => discover_time[n.index()] = t,
                Finish(n, t) => finish_time[n.index()] = t,
                TreeEdge(u, v) => {
                    // v is an ancestor of u
                    assert!(has_tree_edge.visit(v), "Two tree edges to {:?}!", v);
                    assert!(discover_time[v.index()] == invalid_time);
                    assert!(discover_time[u.index()] != invalid_time);
                    assert!(finish_time[u.index()] == invalid_time);
                    edges.insert((u, v));
                }
                BackEdge(u, v) => {
                    // u is an ancestor of v
                    assert!(discover_time[v.index()] != invalid_time);
                    assert!(finish_time[v.index()] == invalid_time);
                    edges.insert((u, v));
                }
                CrossForwardEdge(u, v) => {
                    edges.insert((u, v));
                }
            }
        });
        assert!(discover_time.iter().all(|x| *x != invalid_time));
        assert!(finish_time.iter().all(|x| *x != invalid_time));
        assert_eq!(edges.len(), gr.edge_count());
        assert_eq!(edges, set(gr.edge_references().map(|e| (e.source(), e.target()))));
        true
    }
}

quickcheck! {
    fn test_bellman_ford(gr: Graph<(), f32>) -> bool {
        let mut gr = gr;
        for elt in gr.edge_weights_mut() {
            *elt = elt.abs();
        }
        if gr.node_count() == 0 {
            return true;
        }
        for (i, start) in gr.node_indices().enumerate() {
            if i >= 10 { break; } // testing all is too slow
            bellman_ford(&gr, start).unwrap();
        }
        true
    }
}

quickcheck! {
    fn test_find_negative_cycle(gr: Graph<(), f32>) -> bool {
        let gr = gr;
        if gr.node_count() == 0 {
            return true;
        }
        for (i, start) in gr.node_indices().enumerate() {
            if i >= 10 { break; } // testing all is too slow
            if let Some(path) = find_negative_cycle(&gr, start) {
                assert!(path.len() >= 1);
            }
        }
        true
    }
}

quickcheck! {
    fn test_bellman_ford_undir(gr: Graph<(), f32, Undirected>) -> bool {
        let mut gr = gr;
        for elt in gr.edge_weights_mut() {
            *elt = elt.abs();
        }
        if gr.node_count() == 0 {
            return true;
        }
        for (i, start) in gr.node_indices().enumerate() {
            if i >= 10 { break; } // testing all is too slow
            bellman_ford(&gr, start).unwrap();
        }
        true
    }
}

defmac!(iter_eq a, b => a.eq(b));
defmac!(nodes_eq ref a, ref b => a.node_references().eq(b.node_references()));
defmac!(edgew_eq ref a, ref b => a.edge_references().eq(b.edge_references()));
defmac!(edges_eq ref a, ref b =>
        iter_eq!(
            a.edge_references().map(|e| (e.source(), e.target())),
            b.edge_references().map(|e| (e.source(), e.target()))));

quickcheck! {
    fn test_di_from(gr1: DiGraph<i32, i32>) -> () {
        let sgr = StableGraph::from(gr1.clone());
        let gr2 = Graph::from(sgr);

        assert!(nodes_eq!(&gr1, &gr2));
        assert!(edgew_eq!(&gr1, &gr2));
        assert!(edges_eq!(&gr1, &gr2));
    }
    fn test_un_from(gr1: UnGraph<i32, i32>) -> () {
        let sgr = StableGraph::from(gr1.clone());
        let gr2 = Graph::from(sgr);

        assert!(nodes_eq!(&gr1, &gr2));
        assert!(edgew_eq!(&gr1, &gr2));
        assert!(edges_eq!(&gr1, &gr2));
    }

    fn test_graph_from_stable_graph(gr1: StableDiGraph<usize, usize>) -> () {
        let mut gr1 = gr1;
        let gr2 = Graph::from(gr1.clone());

        // renumber the stablegraph nodes and put the new index in the
        // associated data
        let mut index = 0;
        for i in 0..gr1.node_bound() {
            let ni = node_index(i);
            if gr1.contains_node(ni) {
                gr1[ni] = index;
                index += 1;
            }
        }
        if let Some(edge_bound) = gr1.edge_references().next_back()
            .map(|ed| ed.id().index() + 1)
        {
            index = 0;
            for i in 0..edge_bound {
                let ni = edge_index(i);
                if gr1.edge_weight(ni).is_some() {
                    gr1[ni] = index;
                    index += 1;
                }
            }
        }

        assert_equal(
            // Remap the stablegraph to compact indices
            gr1.edge_references().map(|ed| (edge_index(*ed.weight()), gr1[ed.source()], gr1[ed.target()])),
            gr2.edge_references().map(|ed| (ed.id(), ed.source().index(), ed.target().index()))
        );
    }

    fn stable_di_graph_map_id(gr1: StableDiGraph<usize, usize>) -> () {
        let gr2 = gr1.map(|_, &nw| nw, |_, &ew| ew);
        assert!(nodes_eq!(&gr1, &gr2));
        assert!(edgew_eq!(&gr1, &gr2));
        assert!(edges_eq!(&gr1, &gr2));
    }

    fn stable_un_graph_map_id(gr1: StableUnGraph<usize, usize>) -> () {
        let gr2 = gr1.map(|_, &nw| nw, |_, &ew| ew);
        assert!(nodes_eq!(&gr1, &gr2));
        assert!(edgew_eq!(&gr1, &gr2));
        assert!(edges_eq!(&gr1, &gr2));
    }

    fn stable_di_graph_filter_map_id(gr1: StableDiGraph<usize, usize>) -> () {
        let gr2 = gr1.filter_map(|_, &nw| Some(nw), |_, &ew| Some(ew));
        assert!(nodes_eq!(&gr1, &gr2));
        assert!(edgew_eq!(&gr1, &gr2));
        assert!(edges_eq!(&gr1, &gr2));
    }

    fn test_stable_un_graph_filter_map_id(gr1: StableUnGraph<usize, usize>) -> () {
        let gr2 = gr1.filter_map(|_, &nw| Some(nw), |_, &ew| Some(ew));
        assert!(nodes_eq!(&gr1, &gr2));
        assert!(edgew_eq!(&gr1, &gr2));
        assert!(edges_eq!(&gr1, &gr2));
    }

    fn stable_di_graph_filter_map_remove(gr1: Small<StableDiGraph<i32, i32>>,
                                         nodes: Vec<usize>,
                                         edges: Vec<usize>) -> ()
    {
        let gr2 = gr1.filter_map(|ix, &nw| {
            if !nodes.contains(&ix.index()) { Some(nw) } else { None }
        },
        |ix, &ew| {
            if !edges.contains(&ix.index()) { Some(ew) } else { None }
        });
        let check_nodes = &set(gr1.node_indices()) - &set(cloned(&nodes).map(node_index));
        let mut check_edges = &set(gr1.edge_indices()) - &set(cloned(&edges).map(edge_index));
        // remove all edges with endpoint in removed nodes
        for edge in gr1.edge_references() {
            if nodes.contains(&edge.source().index()) ||
                nodes.contains(&edge.target().index()) {
                check_edges.remove(&edge.id());
            }
        }
        // assert maintained
        for i in check_nodes {
            assert_eq!(gr1[i], gr2[i]);
        }
        for i in check_edges {
            assert_eq!(gr1[i], gr2[i]);
            assert_eq!(gr1.edge_endpoints(i), gr2.edge_endpoints(i));
        }

        // assert removals
        for i in nodes {
            assert!(gr2.node_weight(node_index(i)).is_none());
        }
        for i in edges {
            assert!(gr2.edge_weight(edge_index(i)).is_none());
        }
    }
}

fn naive_closure_foreach<G, F>(g: G, mut f: F)
where
    G: Visitable + IntoNeighbors + IntoNodeIdentifiers,
    F: FnMut(G::NodeId, G::NodeId),
{
    let mut dfs = Dfs::empty(&g);
    for i in g.node_identifiers() {
        dfs.reset(&g);
        dfs.move_to(i);
        while let Some(nx) = dfs.next(&g) {
            if i != nx {
                f(i, nx);
            }
        }
    }
}

fn naive_closure<G>(g: G) -> Vec<(G::NodeId, G::NodeId)>
where
    G: Visitable + IntoNodeIdentifiers + IntoNeighbors,
{
    let mut res = Vec::new();
    naive_closure_foreach(g, |a, b| res.push((a, b)));
    res
}

fn naive_closure_edgecount<G>(g: G) -> usize
where
    G: Visitable + IntoNodeIdentifiers + IntoNeighbors,
{
    let mut res = 0;
    naive_closure_foreach(g, |_, _| res += 1);
    res
}

quickcheck! {
    fn test_tred(g: DAG<()>) -> bool {
        let acyclic = g.0;
        println!("acyclic graph {:#?}", &acyclic);
        let toposort = toposort(&acyclic, None).unwrap();
        println!("Toposort:");
        for (new, old) in toposort.iter().enumerate() {
            println!("{} -> {}", old.index(), new);
        }
        let (toposorted, revtopo): (petgraph::adj::List<(), usize>, _) =
            petgraph::algo::tred::dag_to_toposorted_adjacency_list(&acyclic, &toposort);
        println!("checking revtopo");
        for (i, ix) in toposort.iter().enumerate() {
            assert_eq!(i, revtopo[ix.index()]);
        }
        println!("toposorted adjacency list: {:#?}", &toposorted);
        let (tred, tclos) = petgraph::algo::tred::dag_transitive_reduction_closure(&toposorted);
        println!("tred: {:#?}", &tred);
        println!("tclos: {:#?}", &tclos);
        if tred.node_count() != tclos.node_count() {
            println!("Different node count");
            return false;
        }
        if acyclic.node_count() != tclos.node_count() {
            println!("Different node count from original graph");
            return false;
        }
        // check the closure
        let mut clos_edges: Vec<(_, _)> = tclos.edge_references().map(|i| (i.source(), i.target())).collect();
        clos_edges.sort();
        let mut tred_closure = naive_closure(&tred);
        tred_closure.sort();
        if tred_closure != clos_edges {
            println!("tclos is not the transitive closure of tred");
            return false
        }
        // check the transitive reduction is a transitive reduction
        for i in tred.edge_references() {
            let filtered = EdgeFiltered::from_fn(&tred, |edge| {
                edge.source() !=i.source() || edge.target() != i.target()
            });
            let new = naive_closure_edgecount(&filtered);
            if new >= clos_edges.len() {
                println!("when removing ({} -> {}) the transitive closure does not shrink",
                         i.source().index(), i.target().index());
                return false
            }
        }
        // check that the transitive reduction is included in the original graph
        for i in tred.edge_references() {
            if acyclic.find_edge(toposort[i.source().index()], toposort[i.target().index()]).is_none() {
                println!("tred is not included in the original graph");
                return false
            }
        }
        println!("ok!");
        true
    }
}

quickcheck! {
    fn greedy_fas_remaining_graph_is_acyclic(g: StableDiGraph<(), ()>) -> bool {
        let mut g = g;
        let fas: Vec<EdgeIndex> = greedy_feedback_arc_set(&g).map(|e| e.id()).collect();

        for edge_id in fas {
            g.remove_edge(edge_id);
        }

        !is_cyclic_directed(&g)
    }

    /// Assert that the size of the feedback arc set of a tournament does not exceed
    /// **|E| / 2 - |V| / 6**
    fn greedy_fas_performance_within_bound(t: Tournament<(), ()>) -> bool {
        let Tournament(g) = t;

        let expected_bound = if g.node_count() < 2 {
            0
        } else {
            ((g.edge_count() as f64) / 2.0 - (g.node_count() as f64) / 6.0) as usize
        };

        let fas_size = greedy_feedback_arc_set(&g).count();

        fas_size <= expected_bound
    }
}

fn is_valid_matching<G: NodeIndexable>(m: &Matching<G>) -> bool {
    // A set of edges is a matching if no two edges from the matching share an
    // endpoint.
    for (s1, t1) in m.edges() {
        for (s2, t2) in m.edges() {
            if s1 == s2 && t1 == t2 {
                continue;
            }

            if s1 == s2 || s1 == t2 || t1 == s2 || t1 == t2 {
                // Two edges share an endpoint.
                return false;
            }
        }
    }

    true
}

fn is_maximum_matching<G: NodeIndexable + IntoEdges + IntoNodeIdentifiers + Visitable>(
    g: G,
    m: &Matching<G>,
) -> bool {
    // Berge's lemma: a matching is maximum iff there is no augmenting path (a
    // path that starts and ends in unmatched vertices, and alternates between
    // matched and unmatched edges). Thus if we find an augmenting path, the
    // matching is not maximum.
    //
    // Start with an unmatched node and traverse the graph alternating matched
    // and unmatched edges. If an unmatched node is found, then an augmenting
    // path was found.
    for unmatched in g.node_identifiers().filter(|u| !m.contains_node(*u)) {
        let visited = &mut g.visit_map();
        let mut stack = Vec::new();

        stack.push((unmatched, false));
        while let Some((u, do_matched_edges)) = stack.pop() {
            if visited.visit(u) {
                for e in g.edges(u) {
                    if e.source() == e.target() {
                        // Ignore self-loops.
                        continue;
                    }

                    let is_matched = m.contains_edge(e.source(), e.target());

                    if do_matched_edges && is_matched || !do_matched_edges && !is_matched {
                        stack.push((e.target(), !do_matched_edges));

                        // Found another free node (other than the starting one)
                        // that is unmatched - an augmenting path.
                        if !is_matched && !m.contains_node(e.target()) && e.target() != unmatched {
                            return false;
                        }
                    }
                }
            }
        }
    }

    true
}

fn is_perfect_matching<G: NodeCount + NodeIndexable>(g: G, m: &Matching<G>) -> bool {
    // By definition.
    g.node_count() % 2 == 0 && m.edges().count() == g.node_count() / 2
}

quickcheck! {
    fn matching(g: Graph<(), (), Undirected>) -> bool {
        let m1 = greedy_matching(&g);
        let m2 = maximum_matching(&g);

        assert!(is_valid_matching(&m1), "greedy_matching returned an invalid matching");
        assert!(is_valid_matching(&m2), "maximum_matching returned an invalid matching");
        assert!(is_maximum_matching(&g, &m2), "maximum_matching returned a matching that is not maximum");
        assert_eq!(m1.is_perfect(), is_perfect_matching(&g, &m1), "greedy_matching incorrectly determined whether the matching is perfect");
        assert_eq!(m2.is_perfect(), is_perfect_matching(&g, &m2), "maximum_matching incorrectly determined whether the matching is perfect");

        true
    }

    fn matching_in_stable_graph(g: StableGraph<(), (), Undirected>) -> bool {
        let m1 = greedy_matching(&g);
        let m2 = maximum_matching(&g);

        assert!(is_valid_matching(&m1), "greedy_matching returned an invalid matching");
        assert!(is_valid_matching(&m2), "maximum_matching returned an invalid matching");
        assert!(is_maximum_matching(&g, &m2), "maximum_matching returned a matching that is not maximum");
        assert_eq!(m1.is_perfect(), is_perfect_matching(&g, &m1), "greedy_matching incorrectly determined whether the matching is perfect");
        assert_eq!(m2.is_perfect(), is_perfect_matching(&g, &m2), "maximum_matching incorrectly determined whether the matching is perfect");

        true
    }
}

quickcheck! {
    // The ranks are probabilities,
    // as such they are positive and they should sum up to 1.
    fn test_page_rank_proba(gr: Graph<(), f32>) -> bool {
        if gr.node_count() == 0 {
            return true;
        }
        let tol = 1e-10;
        let ranks: Vec<f64> = page_rank(&gr, 0.85_f64, 5);
        let at_least_one_neg_rank = ranks.iter().any(|rank| *rank < 0.);
        let not_sumup_to_one = (ranks.iter().sum::<f64>() - 1.).abs() > tol;
        if  at_least_one_neg_rank | not_sumup_to_one{
            return false;
        }
        true
    }
}

fn sum_flows<N, F: std::iter::Sum + Copy>(
    gr: &Graph<N, F>,
    flows: &[F],
    node: NodeIndex,
    dir: Direction,
) -> F {
    gr.edges_directed(node, dir)
        .map(|edge| flows[EdgeIndexable::to_index(&gr, edge.id())])
        .sum::<F>()
}

quickcheck! {
    // 1. (Capacity)
    //    The flows should be <= capacities
    // 2. (Flow conservation)
    //    For every internal node (i.e a node different from the
    //    source node and the destination (or sink) node), the sum
    //    of incoming flows (i.e flows of incoming edges) is equal
    //    to the sum of the outgoing flows (i.e flows of outgoing edges).
    // 3. (Maximum flow)
    //    It is equal to the sum of the destination node incoming flows and
    //    also the sum of the outgoing flows of the source node.
    fn test_ford_fulkerson_flows(gr: Graph<usize, u32>) -> bool {
        if gr.node_count() <= 1 || gr.edge_count() == 0 {
            return true;
        }
        let source = NodeIndex::from(0);
        let destination = NodeIndex::from(gr.node_count() as u32 / 2);
        let (max_flow, flows) = ford_fulkerson(&gr, source, destination);
        let capacity_constraint = flows
            .iter()
            .enumerate()
            .all(|(ix, flow)| flow <= gr.edge_weight(EdgeIndexable::from_index(&gr, ix)).unwrap());
        let flow_conservation_constraint = (0..gr.node_count()).all(|ix| {
            let node = NodeIndexable::from_index(&gr, ix);
            if (node != source) && (node != destination){
            sum_flows(&gr, &flows, node, Direction::Outgoing)
                == sum_flows(&gr, &flows, node, Direction::Incoming)
            } else {true}
        });
        let max_flow_constaint = (sum_flows(&gr, &flows, source, Direction::Outgoing) == max_flow)
            && (sum_flows(&gr, &flows, destination, Direction::Incoming) == max_flow);
        return capacity_constraint && flow_conservation_constraint && max_flow_constaint;
    }
}
