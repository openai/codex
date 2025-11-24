Contributing to ``petgraph``
============================

Hi! We'd love to have your contributions! If you want help or mentorship, reach
out to us in a GitHub issue, or ping ``bluss`` in `#rust on irc.mozilla.org`_
and introduce yourself.

.. _`\#rust on irc.mozilla.org`: irc://irc.mozilla.org#rust

* `Building`_

* `Testing`_

* `Pull Requests`_

  * `Bug Fixes`_

  * `Performance Improvements`_

  * `Implementing New Algorithms`_

* `Where We Need Help`_

* `Team`_

Building
--------

::

    $ cargo build

Testing
-------

::

    $ cargo test --features all

Pull Requests
-------------

All pull requests are reviewed by a team_ member before merging.

Additionally, different kinds of pull requests have different requirements.

Bug Fixes
.........

We love getting bug fixes!

Make sure to include a regression test, so that we can be sure that we never
accidentally re-introduce the bug again.

Performance Improvements
........................

You made an algorithm faster? Awesome.

When submitting performance improvement, include the following:

* A new ``#[bench]`` function that exercises this code path, if one doesn't
  already exist

* Before and after ``cargo bench`` scores, optionally formatted using
  `cargo-benchcmp`_

.. _`cargo-benchcmp`: https://github.com/BurntSushi/cargo-benchcmp

Implementing New Algorithms
...........................

Implementing new graph algorithms is encouraged!

If you're going to implement a new algorithm, make sure that you do the
following:

* Add a ``quickcheck`` property test for the new algorithm

* Add a ``benchmark`` test for measuring performance of the new algorithm

* Document what the algorithm does and in what situations it should be used

* Document the big-O running time of the algorithm

* Include links to relevant reading materials, such as a paper or Wikipedia

* Make the algorithm work with generic graphs, constraining the generic graph
  type parameter with our existing graph traits, like ``Visitable``, or with new
  graph traits

Any team_ member can review a pull request implementing a new algorithm, but the
final decision whether or not the algorithm is appropriate for inclusion in the
``petgraph`` crate is left to ``@bluss``.

Additionally, assuming that the new algorithm is merged into ``petgraph``, you
are *strongly* encouraged to join the ``petgraph`` team_! *You* are the best
person to review any future bug fixes, performance improvements, and whatever
other changes that affect this new algorithm.

Where We Need Help
------------------

* Issues labeled `"help wanted"`_ are issues where we could use a little help
  from you.

* Issues Labeled `"mentored"`_ are issues that don't really involve any more
  investigation, just implementation. We've outlined what needs to be done, and
  a team_ member has volunteered to help whoever claims the issue implement it,
  and get the implementation merged.

.. _`"help wanted"`:
   https://github.com/bluss/petgraph/issues?q=is%3Aopen+is%3Aissue+label%3A%22help+wanted%22

.. _`"mentored"`:
   https://github.com/bluss/petgraph/issues?q=is%3Aopen+is%3Aissue+label%3A%22mentored%22

Team
----

The ``petgraph`` team consists of:

* ``@bluss``
* ``@fitzgen``

We need more team members to help spread out reviewing and maintenance
responsibilities — want to join us? `Drop a comment in this issue!`_

.. _`Drop a comment in this issue!`: https://github.com/bluss/petgraph/issues/TODO
