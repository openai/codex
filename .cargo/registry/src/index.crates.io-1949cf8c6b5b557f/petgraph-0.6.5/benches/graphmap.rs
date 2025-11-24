#![feature(test)]
#![cfg(feature = "rayon")]

extern crate petgraph;
extern crate test;

use petgraph::prelude::*;
use rayon::iter::ParallelIterator;
use std::hash::BuildHasher;
use test::Bencher;

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct MyStruct {
    u: String,
    v: String,
    w: String,
}

fn test_nodes() -> Vec<MyStruct> {
    let mut nodes = vec![];
    for i in 0..2500 {
        nodes.push(MyStruct {
            u: format!("X {}", i),
            v: format!("Y {} Y", i),
            w: format!("{}Z", i),
        });
    }

    nodes
}

fn test_graph<H: BuildHasher + Default>(
    data: &Vec<MyStruct>,
) -> GraphMap<&MyStruct, usize, Directed, H> {
    let mut gr = GraphMap::new();

    for i in 0..2500 {
        gr.add_node(&data[i]);
    }

    for i in 0..1_000 {
        for j in 999..2000 {
            gr.add_edge(&data[i], &data[j], i * j);
        }
    }

    gr
}

macro_rules! test_case_with_hasher {
    ($name:ident, $hasher:path) => {
        #[bench]
        fn $name(bench: &mut Bencher) {
            let data = test_nodes();
            let gr = test_graph::<$hasher>(&data);
            bench.iter(|| {
                let mut sources = vec![];
                for n in gr.nodes() {
                    for (src, _, e) in gr.edges_directed(n, Direction::Outgoing) {
                        if *e == 500 {
                            sources.push(src.clone());
                        }
                    }
                }
            });
        }
    };
}

test_case_with_hasher!(graphmap_serial_bench, std::hash::RandomState);
test_case_with_hasher!(graphmap_serial_bench_fxhash, fxhash::FxBuildHasher);
test_case_with_hasher!(graphmap_serial_bench_ahash, ahash::RandomState);

#[bench]
fn graphmap_parallel_bench(bench: &mut Bencher) {
    let data = test_nodes();
    let gr = test_graph::<std::hash::RandomState>(&data);
    bench.iter(|| {
        let sources: Vec<MyStruct> = gr
            .par_nodes()
            .map(|n| {
                let mut sources = vec![];
                for (src, _, e) in gr.edges_directed(n, Direction::Outgoing) {
                    if *e == 500 {
                        sources.push(src.clone());
                    }
                }

                sources
            })
            .flatten()
            .collect();
    });
}
