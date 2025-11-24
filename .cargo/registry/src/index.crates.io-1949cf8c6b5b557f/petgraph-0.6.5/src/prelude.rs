//! Commonly used items.
//!
//! ```
//! use petgraph::prelude::*;
//! ```

#[doc(no_inline)]
pub use crate::graph::{DiGraph, EdgeIndex, Graph, NodeIndex, UnGraph};
#[cfg(feature = "graphmap")]
#[doc(no_inline)]
pub use crate::graphmap::{DiGraphMap, GraphMap, UnGraphMap};
#[doc(no_inline)]
#[cfg(feature = "stable_graph")]
pub use crate::stable_graph::{StableDiGraph, StableGraph, StableUnGraph};
#[doc(no_inline)]
pub use crate::visit::{Bfs, Dfs, DfsPostOrder};
#[doc(no_inline)]
pub use crate::{Directed, Direction, Incoming, Outgoing, Undirected};

#[doc(no_inline)]
pub use crate::visit::EdgeRef;
