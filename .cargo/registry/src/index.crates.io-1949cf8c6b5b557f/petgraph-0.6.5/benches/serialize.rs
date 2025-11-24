#![feature(test)]

#[cfg(feature = "serde-1")]
mod serialize {
    extern crate petgraph;
    extern crate test;

    use petgraph::prelude::*;
    use rand::Rng;
    use test::Bencher;

    const NUM_NODES: usize = 1_000_000;
    const NUM_EDGES: usize = 100_000;
    const NUM_HOLES: usize = 1_000_000;

    fn make_stable_graph() -> StableGraph<u32, u32> {
        let mut g = StableGraph::with_capacity(NUM_NODES + NUM_HOLES, NUM_EDGES);
        let indices: Vec<_> = (0..NUM_NODES + NUM_HOLES)
            .map(|i| g.add_node(i as u32))
            .collect();

        let mut rng = rand::thread_rng();
        g.extend_with_edges((0..NUM_EDGES).map(|_| {
            let first = rng.gen_range(0, NUM_NODES + NUM_HOLES);
            let second = rng.gen_range(0, NUM_NODES + NUM_HOLES - 1);
            let second = second + (second >= first) as usize;
            let weight: u32 = rng.gen();
            (indices[first], indices[second], weight)
        }));

        // Remove nodes to make the structure a bit more interesting
        while g.node_count() > NUM_NODES {
            let idx = rng.gen_range(0, indices.len());
            g.remove_node(indices[idx]);
        }

        g
    }

    #[bench]
    fn serialize_stable_graph(bench: &mut Bencher) {
        let graph = make_stable_graph();
        bench.iter(|| bincode::serialize(&graph).unwrap());
    }

    #[bench]
    fn deserialize_stable_graph(bench: &mut Bencher) {
        let graph = make_stable_graph();
        let data = bincode::serialize(&graph).unwrap();
        bench.iter(|| {
            let graph2: StableGraph<u32, u32> = bincode::deserialize(&data).unwrap();
            graph2
        });
    }
}
