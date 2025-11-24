Version 0.6.5 (2024-05-06)
==========================

- Add rayon support for ``GraphMap`` (`#573`_, `#615`_)
- Add ``Topo::with_initials`` method (`#585`_)
- Add logo to the project (`#598`_)
- Add Ford-Fulkerson algorithm (`#640`_)
- Update ``itertools`` to 0.12.1 (`#628`_)
- Update ``GraphMap`` to allow custom hash functions (`#623`_)
- Fix documentation (`#630`_)
- Fix clippy warnings (`#627`_)
- (internal) Fix remove old ``copyclone`` macro (`#601`_)
- (internal) Move minimum spanning tree into own module (`#624`_)

.. _`#573`: https://github.com/petgraph/petgraph/pull/573
.. _`#615`: https://github.com/petgraph/petgraph/pull/615
.. _`#585`: https://github.com/petgraph/petgraph/pull/585
.. _`#598`: https://github.com/petgraph/petgraph/pull/598
.. _`#640`: https://github.com/petgraph/petgraph/pull/640
.. _`#628`: https://github.com/petgraph/petgraph/pull/628
.. _`#623`: https://github.com/petgraph/petgraph/pull/623
.. _`#630`: https://github.com/petgraph/petgraph/pull/630
.. _`#627`: https://github.com/petgraph/petgraph/pull/627
.. _`#601`: https://github.com/petgraph/petgraph/pull/601
.. _`#624`: https://github.com/petgraph/petgraph/pull/624

Version 0.6.4 (2023-08-21)
==========================

- Update ``indexmap`` to 2.0.0 (`#568`_)
- Fix typos (`#544`_)

.. _`#544`: https://github.com/petgraph/petgraph/pull/544
.. _`#568`: https://github.com/petgraph/petgraph/pull/568

Version 0.6.3 (2023-02-07)
==========================

- Added an iterator over subgraph isomorphisms (`#500`_)
- Added serde support on ``GraphMap`` (`#496`_)
- Added ``reverse`` method for ``StableGraph`` (`#533`_)
- Added ``edges_connecting`` iterator for ``StableGraph`` (`#521`_)
- Fix Floyd-Warshall algorithm behaviour on undirected graphs (`487`_)
- Fix IntoEdgesDirected implementation for NodeFiltered when direction is Incoming (`476`_)
- Fix cardinality check in subgraph isomorphism (`472`_)
- Fix UB in ``MatrixGraph`` (`#505`_)

.. _`#472`: https://github.com/petgraph/petgraph/issues/472
.. _`#476`: https://github.com/petgraph/petgraph/issues/476
.. _`#487`: https://github.com/petgraph/petgraph/issues/487
.. _`#496`: https://github.com/petgraph/petgraph/issues/496
.. _`#500`: https://github.com/petgraph/petgraph/issues/500
.. _`#505`: https://github.com/petgraph/petgraph/issues/505
.. _`#521`: https://github.com/petgraph/petgraph/issues/521
.. _`#533`: https://github.com/petgraph/petgraph/issues/533

Version 0.6.2 (2022-05-28)
==========================

- Loosed the strict version dependency set in `493`_, to allow users to use newer versions of indexmap  (`495`_).

.. _`#495`: https://github.com/petgraph/petgraph/issues/493

Version 0.6.1 (2022-05-22)
==========================

- Added clarifications on Graph docs (`491`_).
- Fix build errors on rust 1.41 (`493`_).

.. _`#491`: https://github.com/petgraph/petgraph/issues/491
.. _`#493`: https://github.com/petgraph/petgraph/issues/493

Version 0.6.0 (2021-07-04)
==========================

Breaking changes
----------------

- MSRV is now 1.41 (`#444`_).
- Removed the ``NodeCompactIndexable`` trait impl for ``MatrixGraph`` (`#429`_).
- The ``IntoEdges::edges`` implementations are now required return edges with the passed node as source (`#433`_).

New features
------------

- Multiple documentation improvements (`#360`_, `#383`_, `#426`_, `#433`_, `#437`_, `#443`_, `#450`_).
- Added an ``immediately_dominated_by`` method to the dominators result (`#337`_).
- Added ``adj::List``, a new append-only graph type using a simple adjacency list with no node-weights (`#263`_).
- Added ``dag_to_toposorted_adjacency_list`` and ``dag_transitive_reduction_closure`` algorithms to transitively reduce an acyclic graph (`#263`_).
- Made the ``is_isomorphic`` algorithm generic on both graph types (`#369`_).
- Implement Debug and Clone for all the iterators (`#418`_).
- Implement multiple mising traits on graph implementations and adapters (`#405`_, `#429`_).
- Add an EdgeIndexable public trait (`#402`_).
- Added immutable ``node_weights`` and ``edge_weights`` methods for ``Graph`` and ``StableGraph`` (`#363`_).

New algorithms
--------------

- Added a k-shortest-path implementation (`#328`_).
- Added a generic graph complement implementation (`#371`_).
- Added a maximum matching implementation (`#400`_).
- Added a Floyd-Warshall shortest path algorithm (`#377`_).
- Added a greedy feedback arc set algorithm (`#386`_).
- Added a `find_negative_cycle` algorithm (`#434`_).

Performance
-----------

- Reuse the internal state in ``tarjan_scc`` (`#313`_)
- Reduce memory usage in ``tarjan_scc`` (`#413`_).
- Added tighter size hints to all iterators (`#380`_).
- Optimized ``petgraph::dot`` a bit (`#424`_).
- Optimized StableGraph de-serialization with holes (`#395`_).

Bug fixes
---------

- Fixed A* not producing optimal solutions with inconsistent heuristics (`#379`_).
- Fixed a stacked borrow violation (`#404`_).
- Fixed a panic in ``StableGraph::extend_with_edges`` (`#415`_).
- Fixed multiple bugs in the matrix graph implementation (`#427`_).
- Fixed ``GraphMap::remove_node`` not removing some edges (`#432`_).
- Fixed all clippy warnings (`#440`_, `#449`_).

Other changes
-------------

- Now using github actions as CI (`#391`_).
- Replace matchs on `Option<T>` with `map` (`#381`_).
- Added benchmarks for ``tarjan_scc`` (`#421`_).

.. _`#263`: https://github.com/petgraph/petgraph/issues/263
.. _`#313`: https://github.com/petgraph/petgraph/issues/313
.. _`#328`: https://github.com/petgraph/petgraph/issues/328
.. _`#337`: https://github.com/petgraph/petgraph/issues/337
.. _`#360`: https://github.com/petgraph/petgraph/issues/360
.. _`#363`: https://github.com/petgraph/petgraph/issues/363
.. _`#369`: https://github.com/petgraph/petgraph/issues/369
.. _`#371`: https://github.com/petgraph/petgraph/issues/371
.. _`#377`: https://github.com/petgraph/petgraph/issues/377
.. _`#379`: https://github.com/petgraph/petgraph/issues/378
.. _`#380`: https://github.com/petgraph/petgraph/issues/380
.. _`#381`: https://github.com/petgraph/petgraph/issues/381
.. _`#383`: https://github.com/petgraph/petgraph/issues/383
.. _`#386`: https://github.com/petgraph/petgraph/issues/386
.. _`#391`: https://github.com/petgraph/petgraph/issues/391
.. _`#395`: https://github.com/petgraph/petgraph/issues/395
.. _`#400`: https://github.com/petgraph/petgraph/issues/400
.. _`#402`: https://github.com/petgraph/petgraph/issues/402
.. _`#404`: https://github.com/petgraph/petgraph/issues/404
.. _`#405`: https://github.com/petgraph/petgraph/issues/405
.. _`#413`: https://github.com/petgraph/petgraph/issues/413
.. _`#415`: https://github.com/petgraph/petgraph/issues/415
.. _`#418`: https://github.com/petgraph/petgraph/issues/418
.. _`#421`: https://github.com/petgraph/petgraph/issues/421
.. _`#424`: https://github.com/petgraph/petgraph/issues/424
.. _`#426`: https://github.com/petgraph/petgraph/issues/426
.. _`#427`: https://github.com/petgraph/petgraph/issues/427
.. _`#429`: https://github.com/petgraph/petgraph/issues/429
.. _`#432`: https://github.com/petgraph/petgraph/issues/432
.. _`#433`: https://github.com/petgraph/petgraph/issues/433
.. _`#434`: https://github.com/petgraph/petgraph/issues/434
.. _`#437`: https://github.com/petgraph/petgraph/issues/437
.. _`#440`: https://github.com/petgraph/petgraph/issues/440
.. _`#443`: https://github.com/petgraph/petgraph/issues/443
.. _`#444`: https://github.com/petgraph/petgraph/issues/444
.. _`#449`: https://github.com/petgraph/petgraph/issues/449
.. _`#450`: https://github.com/petgraph/petgraph/issues/450


Version 0.5.1 (2020-05-23)
==========================

- Implement ``Default`` for traversals.
- Export ``EdgesConnecting`` publicly.
- Implement ``is_bipartite_graph``.
- Add ``FilterNode`` implementation for ``FixedBitSet`` and ``HashSet``.
- Implement ``node_weights_mut`` and ``edge_weights_mut`` for ``StableGraph``.
- Add configurable functions for adding attributes to dotfile features.

Version 0.5.0 (2019-12-25)
==========================

Breaking changes
----------------

- The iterative DFS implementation, ``Dfs``, now marks nodes visited when
  they are pushed onto the stack, not when they're popped off. This may
  require changes to callers that use ``Dfs::from_parts`` or manipulate
  its internals.
- The ``IntoEdgesDirected`` trait now has a stricter contract for
  undirected graphs. Custom implementations of this trait may have to be
  updated. See the `trait documentation`__ for more.

Other changes
-------------

- Upgrade to Rust 2018 edition
- Fix clippy warnings and unify code formatting
- Improved and enhanced documentation
- Update dependencies including modern quickcheck
- Numerous bugfixes and refactorings
- Added ``MatrixGraph`` implementation

__ https://docs.rs/petgraph/0.5/petgraph/visit/trait.IntoEdgesDirected.html

Version 0.4.13 (2018-08-26)
===========================

- Fix clippy warnings by @jonasbb
- Add docs for ``Csr`` by @ksadorf
- Fix conflict with new stable method ``find_map`` in new Rust

Version 0.4.12 (2018-03-26)
===========================

- Newtype ``Time`` now also implements ``Hash``
- Documentation updates for ``Frozen``.

Version 0.4.11 (2018-01-07)
===========================

- Fix ``petgraph::graph::NodeReferences`` to be publicly visible
- Small doc typo and code style files by @shepmaster and @waywardmonkeys
- Fix a future compat warning with pointer casts

Version 0.4.10 (2017-08-15)
===========================

- Add graph trait ``IntoEdgesDirected``
- Update dependencies

Version 0.4.9 (2017-10-02)
==========================

- Fix ``bellman_ford`` to work correctly with undirected graphs (#152) by
  @carrutstick
- Performance improvements for ``Graph, Stablegraph``'s ``.map()``.

Version 0.4.8 (2017-09-20)
==========================

- ``StableGraph`` learned new methods nearing parity with ``Graph``.  Note
  that the ``StableGraph`` methods preserve index stability even in the batch
  removal methods like ``filter_map`` and ``retain_edges``.

  + Added ``.filter_map()``, which maps associated node and edge data
  + Added ``.retain_edges()``, ``.edge_indices()`` and ``.clear_edges()``

- Existing ``Graph`` iterators gained some trait impls:

  + ``.node_indices(), .edge_indices()`` are ``ExactSizeIterator``
  + ``.node_references()`` is now
    ``DoubleEndedIterator + ExactSizeIterator``.
  + ``.edge_references()`` is now ``ExactSizeIterator``.

- Implemented ``From<StableGraph>`` for ``Graph``.

Version 0.4.7 (2017-09-16)
==========================

- New algorithm by @jmcomets: A* search algorithm in ``petgraph::algo::astar``
- One ``StableGraph`` bug fix whose patch was supposed to be in the previous
  version:

  + ``add_edge(m, n, _)`` now properly always panics if nodes m or n don't
    exist in the graph.

Version 0.4.6 (2017-09-12)
==========================

- New optional crate feature: ``"serde-1"``, which enables serialization
  for ``Graph`` and ``StableGraph`` using serde.
- Add methods ``new``, ``add_node`` to ``Csr`` by @jmcomets
- Add indexing with ``[]`` by node index, ``NodeCompactIndexable`` for
  ``Csr`` by @jmcomets
- Amend doc for ``GraphMap::into_graph`` (it has a case where it can panic)
- Add implementation of ``From<Graph>`` for ``StableGraph``.
- Add implementation of ``IntoNodeReferences`` for ``&StableGraph``.
- Add method ``StableGraph::map`` that maps associated data
- Add method ``StableGraph::find_edge_undirected``
- Many ``StableGraph`` bug fixes involving node vacancies (holes left by
  deletions):

  + ``neighbors(n)`` and similar neighbor and edge iterator methods now
    handle n being a vacancy properly. (This produces an empty iterator.)
  + ``find_edge(m, n)`` now handles m being a vacancy correctly too
  + ``StableGraph::node_bound`` was fixed for empty graphs and returns 0

- Add implementation of ``DoubleEndedIterator`` to ``Graph, StableGraph``'s
  edge references iterators.
- Debug output for ``Graph`` now shows node and edge count. ``Graph, StableGraph``
  show nothing for the edges list if it's empty (no label).
- ``Arbitrary`` implementation for ``StableGraph`` now can produce graphs with
  vacancies (used by quickcheck)

Version 0.4.5 (2017-06-16)
==========================

- Fix ``max`` ambiguity error with current rust nightly by @daboross (#153)

Version 0.4.4 (2017-03-14)
==========================

- Add ``GraphMap::all_edges_mut()`` iterator by @Binero
- Add ``StableGraph::retain_nodes`` by @Rupsbant
- Add ``StableGraph::index_twice_mut`` by @christolliday

Version 0.4.3 (2017-01-21)
==========================

- Add crate categories

Version 0.4.2 (2017-01-06)
==========================

- Move the ``visit.rs`` file due to changed rules for a module’s directory
  ownership in Rust, resolving a future compat warning.
- The error types ``Cycle, NegativeCycle`` now implement ``PartialEq``.

Version 0.4.1 (2016-10-26)
==========================

- Add new algorithm ``simple_fast`` for computing dominators in a control-flow
  graph.

Version 0.4.0 (2016-10-17)
==========================

Breaking changes in ``Graph``
-----------------------------

- ``Graph::edges`` and the other edges methods now return an iterator of
  edge references

Other breaking changes
----------------------

- ``toposort`` now returns an error if the graph had a cycle.
- ``is_cyclic_directed`` no longer takes a dfs space argument. It is
  now recursive.
- ``scc`` was renamed to ``kosaraju_scc``.
- ``min_spanning_tree`` now returns an iterator that needs to be
  made into a specific graph type deliberately.
- ``dijkstra`` now uses the ``IntoEdges`` trait.
- ``NodeIndexable`` changed its method signatures.
- ``IntoExternals`` was removed, and many other smaller adjustments
  in graph traits. ``NodeId`` must now implement ``PartialEq``, for example.
- ``DfsIter, BfsIter`` were removed in favour of a more general approach
  with the ``Walker`` trait and its iterator conversion.

New features
------------

- New graph traits, for example ``IntoEdges`` which returns
  an iterator of edge references. Everything implements the graph traits
  much more consistently.
- Traits for associated data access and building graphs: ``DataMap``,
  ``Build, Create, FromElements``.
- Graph adaptors: ``EdgeFiltered``. ``Filtered`` was renamed to ``NodeFiltered``.
- New algorithms: bellman-ford
- New graph: compressed sparse row (``Csr``).
- ``GraphMap`` implements ``NodeIndexable``.
- ``Dot`` was generalized

Version 0.3.2 (2016-10-11)
==========================

  - Add ``depth_first_search``, a recursive dfs visitor that emits discovery,
    finishing and edge classification events.
  - Add graph adaptor ``Filtered``.
  - impl ``Debug, NodeIndexable`` for ``Reversed``.

Version 0.3.1 (2016-10-05)
==========================

- Add ``.edges(), .edges_directed()`` to ``StableGraph``. Note that these
  differ from ``Graph``, because this is the signature they will all use
  in the future.
- Add ``.update_edge()`` to ``StableGraph``.
- Add reexports of common items in ``stable_graph`` module (for example
  ``NodeIndex``).
- Minor performance improvements to graph iteration
- Improved docs for ``visit`` module.

Version 0.3.0 (2016-10-03)
==========================

- Overhaul all graph visitor traits so that they use the ``IntoIterator``
  style. This makes them composable.

  - Multiple graph algorithms use new visitor traits.
  - **Help is welcome to port more algorithms (and create new graph traits in
    the process)!**

- ``GraphMap`` can now have directed edges. ``GraphMap::new`` is now generic
  in the edge type. ``DiGraphMap`` and ``UnGraphMap`` are new type aliases.
- Add type aliases ``DiGraph, UnGraph, StableDiGraph, StableUnGraph``
- ``GraphMap`` is based on the indexmap crate. Deterministic iteration
  order, faster iteration, no side tables needed to convert to ``Graph``.
- Improved docs for a lot of types and functions.
- Add graph visitor ``DfsPostOrder``
- ``Dfs`` gained new methods ``from_parts`` and ``reset``.
- New algo ``has_path_connecting``.
- New algo ``tarjan_scc``, a second scc implementation.
- Document traversal order in ``Dfs, DfsPostOrder, scc, tarjan_scc``.
- Optional graph visitor workspace reuse in ``has_path_connecting``,
  ``is_cyclic_directed, toposort``.
- Improved ``Debug`` formatting for ``Graph, StableGraph``.
- Add a prelude module
- ``GraphMap`` now has a method ``.into_graph()`` that makes a ``Graph``.
- ``Graph::retain_nodes, retain_edges`` now expose the self graph only
  as wrapped in ``Frozen``, so that weights can be mutated but the
  graph structure not.
- Enable ``StableGraph`` by default
- Add method ``Graph::contains_edge``.
- Renamed ``EdgeDirection`` → ``Direction``.
- Remove ``SubTopo``.
- Require Rust 1.12 or later

Version 0.2.10 (2016-07-27)
===========================

- Fix compilation with rust nightly

Version 0.2.9 (2016-10-01)
==========================

- Fix a bug in SubTopo (#81)

Version 0.2.8 (2016-09-12)
==========================

- Add Graph methods reserve_nodes, reserve_edges, reserve_exact_nodes,
  reserve_exact_edges, shrink_to_fit_edges, shrink_to_fit_nodes, shrink_to_fit

Version 0.2.7 (2016-04-22)
==========================

- Update URLs

Version 0.2.6 (2016-04-20)
==========================

- Fix warning about type parameter defaults (no functional change)

Version 0.2.5 (2016-04-10)
==========================

- Add SubTopo, a topo walker for the subgraph reachable from a starting point.
- Add condensation, which forms the graph of a graph’s strongly connected
  components.

Version 0.2.4 (2016-04-05)
==========================

- Fix an algorithm error in scc (#61). This time we have a test that
  crosschecks the result of the algorithm vs another implementation, for
  greater confidence in its correctness.

Version 0.2.3 (2016-02-22)
==========================

- Require Rust 1.6: Due to changes in how rust uses type parameter defaults.
- Implement Graph::clone_from.

Version 0.2.2 (2015-12-14)
==========================

- Require Rust 1.5
- ``Dot`` passes on the alternate flag to node and edge label formatting
- Add ``Clone`` impl for some iterators
- Document edge iteration order for ``Graph::neighbors``
- Add *experimental feature* ``StableGraph``, using feature flag ``stable_graph``

Version 0.2.1 (2015-12-06)
==========================

- Add algorithm ``is_isomorphic_matching``

Version 0.2.0 (2015-12-03)
==========================

New Features
------------

- Add Graph::neighbors().detach() to step edges without borrowing.
  This is more general than, and replaces now deprecated
  walk_edges_directed. (#39)
- Implement Default for Graph, GraphMap
- Add method EdgeDirection::opposite()

Breaking changes
----------------

- Graph::neighbors() for undirected graphs and Graph::neighbors_undirected
  for any graph now visit self loop edges once, not twice. (#31)
- Renamed Graph::without_edges to Graph::externals
- Removed Graph::edges_both
- GraphMap::add_edge now returns ``Option<E>``
- Element type of ``GraphMap<N, E>::all_edges()`` changed to ``(N, N, &E)``

Minor breaking changes
----------------------

- IntoWeightedEdge changed a type parameter to associated type
- IndexType is now an unsafe trait
- Removed IndexType::{one, zero}, use method new instead.
- Removed MinScored
- Ptr moved to the graphmap module.
- Directed, Undirected are now void enums.
- Fields of graphmap::Edges are now private (#19)

Version 0.1.18 (2015-11-30)
===========================

- Fix bug on calling GraphMap::add_edge with existing edge (#35)

Version 0.1.17 (2015-11-25)
===========================

- Add Graph::capacity(), GraphMap::capacity()
- Fix bug in Graph::reverse()
- Graph and GraphMap have `quickcheck::Arbitrary` implementations,
  if optional feature `check` is enabled.

Version 0.1.16 (2015-11-25)
===========================

- Add Graph::node_indices(), Graph::edge_indices()
- Add Graph::retain_nodes(), Graph::retain_edges()
- Add Graph::extend_with_edges(), Graph::from_edges()
- Add functions petgraph::graph::{edge_index, node_index};
- Add GraphMap::extend(), GraphMap::from_edges()
- Add petgraph::dot::Dot for simple graphviz dot output

Version 0.1.15 (2015-11-20)
===========================

- Add Graph::clear_edges()
- Add Graph::edge_endpoints()
- Add Graph::map() and Graph::filter_map()

Version 0.1.14 (2015-11-19)
===========================

- Add new topological order visitor Topo
- New graph traits NeighborsDirected, Externals, Revisitable

Version 0.1.13 (2015-11-11)
===========================

- Add iterator GraphMap::all_edges

Version 0.1.12 (2015-11-07)
===========================

- Fix an algorithm error in scc (#14)

Version 0.1.11 (2015-08-16)
===========================

- Update for well-formedness warnings (Rust RFC 1214), adding
  new lifetime bounds on NeighborIter and Dfs, impact should be minimal.

Version 0.1.10 (2015-06-22)
===========================

- Fix bug in WalkEdges::next_neighbor()

Version 0.1.9 (2015-06-17)
==========================

- Fix Dfs/Bfs for a rustc bugfix that disallowed them
- Add method next_neighbor() to WalkEdges

Version 0.1.8 (2015-06-08)
==========================

- Add Graph::walk_edges_directed()
- Add Graph::index_twice_mut()

Version 0.1.7 (2015-06-08)
==========================

- Add Graph::edges_directed()

Version 0.1.6 (2015-06-04)
==========================

- Add Graph::node_weights_mut and Graph::edge_weights_mut

Version 0.1.4 (2015-05-20)
==========================

- Add back DfsIter, BfsIter
