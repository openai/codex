use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::marker::PhantomData;

use crate::prelude::*;

use crate::graph::Node;
use crate::graph::{Edge, IndexType};
use crate::serde_utils::CollectSeqWithLength;
use crate::serde_utils::MappedSequenceVisitor;
use crate::serde_utils::{FromDeserialized, IntoSerializable};
use crate::stable_graph::StableGraph;
use crate::visit::{EdgeIndexable, NodeIndexable};
use crate::EdgeType;

use super::super::serialization::{
    invalid_hole_err, invalid_length_err, invalid_node_err, EdgeProperty,
};

// Serialization representation for StableGraph
// Keep in sync with deserialization and Graph
#[derive(Serialize)]
#[serde(rename = "Graph")]
#[serde(bound(serialize = "N: Serialize, E: Serialize, Ix: IndexType + Serialize"))]
pub struct SerStableGraph<'a, N: 'a, E: 'a, Ix: 'a + IndexType> {
    nodes: Somes<&'a [Node<Option<N>, Ix>]>,
    node_holes: Holes<&'a [Node<Option<N>, Ix>]>,
    edge_property: EdgeProperty,
    #[serde(serialize_with = "ser_stable_graph_edges")]
    edges: &'a [Edge<Option<E>, Ix>],
}

// Deserialization representation for StableGraph
// Keep in sync with serialization and Graph
#[derive(Deserialize)]
#[serde(rename = "Graph")]
#[serde(bound(
    deserialize = "N: Deserialize<'de>, E: Deserialize<'de>, Ix: IndexType + Deserialize<'de>"
))]
pub struct DeserStableGraph<N, E, Ix> {
    #[serde(deserialize_with = "deser_stable_graph_nodes")]
    nodes: Vec<Node<Option<N>, Ix>>,
    #[serde(default = "Vec::new")]
    node_holes: Vec<NodeIndex<Ix>>,
    edge_property: EdgeProperty,
    #[serde(deserialize_with = "deser_stable_graph_edges")]
    edges: Vec<Edge<Option<E>, Ix>>,
}

/// `Somes` are the present node weights N, with known length.
struct Somes<T>(usize, T);

impl<'a, N, Ix> Serialize for Somes<&'a [Node<Option<N>, Ix>]>
where
    N: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq_with_length(
            self.0,
            self.1.iter().filter_map(|node| node.weight.as_ref()),
        )
    }
}

/// Holes are the node indices of vacancies, with known length
struct Holes<T>(usize, T);

impl<'a, N, Ix> Serialize for Holes<&'a [Node<Option<N>, Ix>]>
where
    Ix: Serialize + IndexType,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq_with_length(
            self.0,
            self.1.iter().enumerate().filter_map(|(i, node)| {
                if node.weight.is_none() {
                    Some(NodeIndex::<Ix>::new(i))
                } else {
                    None
                }
            }),
        )
    }
}

fn ser_stable_graph_edges<S, E, Ix>(
    edges: &&[Edge<Option<E>, Ix>],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    E: Serialize,
    Ix: Serialize + IndexType,
{
    serializer.collect_seq_exact(edges.iter().map(|edge| {
        edge.weight
            .as_ref()
            .map(|w| (edge.source(), edge.target(), w))
    }))
}

fn deser_stable_graph_nodes<'de, D, N, Ix>(
    deserializer: D,
) -> Result<Vec<Node<Option<N>, Ix>>, D::Error>
where
    D: Deserializer<'de>,
    N: Deserialize<'de>,
    Ix: IndexType + Deserialize<'de>,
{
    deserializer.deserialize_seq(MappedSequenceVisitor::new(|n| {
        Ok(Node {
            weight: Some(n),
            next: [EdgeIndex::end(); 2],
        })
    }))
}

fn deser_stable_graph_edges<'de, D, N, Ix>(
    deserializer: D,
) -> Result<Vec<Edge<Option<N>, Ix>>, D::Error>
where
    D: Deserializer<'de>,
    N: Deserialize<'de>,
    Ix: IndexType + Deserialize<'de>,
{
    deserializer.deserialize_seq(MappedSequenceVisitor::<
        Option<(NodeIndex<Ix>, NodeIndex<Ix>, N)>,
        _,
        _,
    >::new(|x| {
        if let Some((i, j, w)) = x {
            Ok(Edge {
                weight: Some(w),
                node: [i, j],
                next: [EdgeIndex::end(); 2],
            })
        } else {
            Ok(Edge {
                weight: None,
                node: [NodeIndex::end(); 2],
                next: [EdgeIndex::end(); 2],
            })
        }
    }))
}

impl<'a, N, E, Ty, Ix> IntoSerializable for &'a StableGraph<N, E, Ty, Ix>
where
    Ix: IndexType,
    Ty: EdgeType,
{
    type Output = SerStableGraph<'a, N, E, Ix>;
    fn into_serializable(self) -> Self::Output {
        let nodes = &self.raw_nodes()[..self.node_bound()];
        let node_count = self.node_count();
        let hole_count = nodes.len() - node_count;
        let edges = &self.raw_edges()[..self.edge_bound()];
        SerStableGraph {
            nodes: Somes(node_count, nodes),
            node_holes: Holes(hole_count, nodes),
            edges: edges,
            edge_property: EdgeProperty::from(PhantomData::<Ty>),
        }
    }
}

/// Requires crate feature `"serde-1"`
impl<N, E, Ty, Ix> Serialize for StableGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType + Serialize,
    N: Serialize,
    E: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.into_serializable().serialize(serializer)
    }
}

impl<'a, N, E, Ty, Ix> FromDeserialized for StableGraph<N, E, Ty, Ix>
where
    Ix: IndexType,
    Ty: EdgeType,
{
    type Input = DeserStableGraph<N, E, Ix>;
    fn from_deserialized<E2>(input: Self::Input) -> Result<Self, E2>
    where
        E2: Error,
    {
        let ty = PhantomData::<Ty>::from_deserialized(input.edge_property)?;
        let node_holes = input.node_holes;
        let edges = input.edges;
        if edges.len() >= <Ix as IndexType>::max().index() {
            Err(invalid_length_err::<Ix, _>("edge", edges.len()))?
        }

        let total_nodes = input.nodes.len() + node_holes.len();
        let mut nodes = Vec::with_capacity(total_nodes);

        let mut compact_nodes = input.nodes.into_iter();
        let mut node_pos = 0;
        for hole_pos in node_holes.iter() {
            let hole_pos = hole_pos.index();
            if !(node_pos..total_nodes).contains(&hole_pos) {
                return Err(invalid_hole_err(hole_pos));
            }
            nodes.extend(compact_nodes.by_ref().take(hole_pos - node_pos));
            nodes.push(Node {
                weight: None,
                next: [EdgeIndex::end(); 2],
            });
            node_pos = hole_pos + 1;
            debug_assert_eq!(nodes.len(), node_pos);
        }
        nodes.extend(compact_nodes);

        if nodes.len() >= <Ix as IndexType>::max().index() {
            Err(invalid_length_err::<Ix, _>("node", nodes.len()))?
        }

        let node_bound = nodes.len();
        let mut sgr = StableGraph {
            g: Graph {
                nodes: nodes,
                edges: edges,
                ty: ty,
            },
            node_count: 0,
            edge_count: 0,
            free_edge: EdgeIndex::end(),
            free_node: NodeIndex::end(),
        };
        sgr.link_edges()
            .map_err(|i| invalid_node_err(i.index(), node_bound))?;
        Ok(sgr)
    }
}

/// Requires crate feature `"serde-1"`
impl<'de, N, E, Ty, Ix> Deserialize<'de> for StableGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType + Deserialize<'de>,
    N: Deserialize<'de>,
    E: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_deserialized(DeserStableGraph::deserialize(deserializer)?)
    }
}

#[test]
fn test_from_deserialized_with_holes() {
    use crate::graph::node_index;
    use crate::stable_graph::StableUnGraph;
    use itertools::assert_equal;
    use serde::de::value::Error as SerdeError;

    let input = DeserStableGraph::<_, (), u32> {
        nodes: vec![
            Node {
                weight: Some(1),
                next: [EdgeIndex::end(); 2],
            },
            Node {
                weight: Some(4),
                next: [EdgeIndex::end(); 2],
            },
            Node {
                weight: Some(5),
                next: [EdgeIndex::end(); 2],
            },
        ],
        node_holes: vec![node_index(0), node_index(2), node_index(3), node_index(6)],
        edges: vec![],
        edge_property: EdgeProperty::Undirected,
    };
    let graph = StableUnGraph::from_deserialized::<SerdeError>(input).unwrap();

    assert_eq!(graph.node_count(), 3);
    assert_equal(
        graph.raw_nodes().iter().map(|n| n.weight.as_ref().cloned()),
        vec![None, Some(1), None, None, Some(4), Some(5), None],
    );
}
