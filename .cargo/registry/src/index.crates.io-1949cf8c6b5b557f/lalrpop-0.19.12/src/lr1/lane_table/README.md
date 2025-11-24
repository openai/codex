This module contains code for LR(1) construction based on a paper by
Pager and Chen, "The Lane Table Method Of Constructing LR(1) Parsers",
published in APPLC '12. Unfortunately, that paper is quite compact --
only 8 pages! -- which doesn't leave much room for examples and
explanation.  This README is my attempt to explain the idea, or at
least how I chose to implement it in LALRPOP, which may or may not be
faithful to the original algorithm. Naturally it also serves as a
guide to the code.

### First example grammar: G0

We will be working through two example grammars. The first I call G0
-- it is a reduced version of what the paper calls G1. It is
interesting because it does not require splitting any states, and so
we wind up with the same number of states as in LR0. Put another way,
it is an LALR(1) grammar.

#### Grammar G0

```
G0 = X "c"
   | Y "d"
X  = "e" X
   | "e"
Y  = "e" Y
   | "e"
```

#### Step 1: Construct an LR(0) state machine

We begin by constructing an LR(0) state machine. The LR(0) states for
G0 are as follows:

```
S0 = G0 = (*) X "c"
   | G0 = (*) Y "d"
   | X = (*) "e" X
   | X = (*) "e"
   | Y = (*) "e" Y
   | Y = (*) "e"

S1 = X = "e" (*) X
   | X = "e" (*)
   | X = (*) "e"
   | X = (*) "e" "X"
   | Y = "e" (*) Y
   | Y = "e" (*)
   | Y = (*) "e"
   | Y = (*) "e" Y

S2 = X = "e" X (*)

S3 = G0 = X (*) "c"

S4 = Y = "e" Y (*)

S5 = G0 = Y (*) "d"

S6 = G0 = X "c" (*)

S7 = G0 = Y "d" (*)
```

We can also consider *edges* between the states as follows,
with the label being the symbol that is pushed onto the stack:

```
S0 -"e"-> S1
S1 -"e"-> S1
S1 --X--> S2
S0 --X--> S3
S1 --Y--> S4
S0 --Y--> S5
S3 -"c"-> S6
S5 -"d"-> S7
```

Note that state S1 is "inconsistent", in that it has conflicting
actions.

#### Step 2: Convert LR(0) states into LR(0-1) states.

The term LR(0-1), but basically the idea is that the lookahead in a
LR(0-1) state can be either a set of terminals (as in LR(1)) or *none*
(as in LR(0)). You can also think of it alternatively as adding a
special "wildcard" symbol `_` to the grammar; in our actual code, we
represent this with `TokenSet::all()`. We will thus denote the
inconsistent state after transformation as follows, where each line
has the "wildcard" lookahead:

```
S1 = X = "e" (*) X   [_]
   | X = "e" (*)     [_]
   | X = (*) "e"     [_]
   | X = (*) "e" "X" [_]
   | Y = "e" (*) Y   [_]
   | Y = "e" (*)     [_]
   | Y = (*) "e"     [_]
   | Y = (*) "e" Y   [_]
```

Naturally, the state is still inconsistent.

#### Step 3: Resolve inconsistencies.

In the next step, we iterate over all of our LR(0-1) states. In this
example, we will not need to create new states, but in future examples
we will. The iteration thus consists of a queue and some code like
this:

```rust
let mut queue = Queue::new();
queue.extend(/* all states */);
while let Some(s) = queue.pop_front() {
    if /* s is an inconsistent state */ {
        resolve_inconsistencies(s, &mut queue);
    }
}
```

##### Step 3a: Build the lane table.

To resolve an inconsistent state, we first construct a **lane
table**. This is done by the code in the `lane` module (the `table`
module maintains the data structure). It works by structing at each
conflict and tracing **backwards**. Let's start with the final table
we will get for the state S1 and then we will work our way back to how
it is constructed. First, let's identify the conflicting actions from
S1 and give them indices:

```
S1 = X = (*) "e"     [_] // C0 -- shift "e"
   | X = "e" (*)     [_] // C1 -- reduce `X = "e" (*)`
   | X = (*) "e" "X" [_] // C0 -- shift "e"
   | X = "e" (*) X   [_]
   | Y = (*) "e"     [_] // C0 -- shift "e"
   | Y = "e" (*)     [_] // C2 -- reduce `Y = "e" (*)`
   | Y = (*) "e" Y   [_] // C0 -- shift "e"
   | Y = "e" (*) Y   [_]
```

Several of the items can cause "Confliction Action 0" (C0), which is
to shift an `"e"`. These are all mutually compatible. However, there
are also two incompatible actions: C1 and C2, both reductions. In
fact, we'll find that we look back at state S0, these 'conflicting'
actions all occur with distinct lookahead. The purpose of the lane
table is to summarize that information. The lane table we will up
constructing for these conflicting actions is as follows:

```
| State | C0    | C1    | C2    | Successors |
| S0    |       | ["c"] | ["d"] | {S1}       |
| S1    | ["e"] | []    | []    | {S1}       |
```

Here the idea is that the lane table summarizes the lookahead
information contributed by each state. Note that for the *shift* the
state S1 already has enough lookahead information: we only shift when
we see the terminal we need next ("e"). But state C1 and C2, the lookahead
actually came from S0, which is a predecessor state.

As I said earlier, the algorithm for constructing the table works by
looking at the conflicting item and walking backwards. So let's
illustrate with conflict C1. We have the conflicting item `X = "e"
(*)`, and we are basically looking to find its lookahead. We know
that somewhere in the distant past of our state machine there must be
an item like

    Foo = ...a (*) X ...b
    
that led us here. We want to find that item, so we can derive the
lookahead from `...b` (whatever symbols come after `X`).

To do this, we will walk the graph. Our state at any point in time
will be the pair of a state and an item in that state. To start out,
then, we have `(S1, X = "e" (*))`, which is the conflict C1. Because
the `(*)` is not at the "front" of this item, we have to figure out
where this `"e"` came from on our stack, so we look for predecessors
of the state S1 which have an item like `X = (*) e`. This leads us to
S0 and also S1. So we can push two states in our search: `(S0, X = (*)
"e")` and `(S1, X = (*) "e")`. Let's consider each in turn.

The next state is then `(S0, X = (*) "e")`. Here the `(*)` lies at the
front of the item, so we search **the same state** S0 for items that
would have led to this state via an *epsilon move*.  This basically
means an item like `Foo = ... (*) X ...` -- i.e., where the `(*)`
appears directly before the nonterminal `X`. In our case, we will find
`G0 = (*) X "c"`. This is great, because it tells us some lookahead
("c", in particular), and hence we can stop our search. We add to the
table the entry that the state S0 contributes lookahead "c" to the
conflict C1.  In some cases, we might find something like `Foo =
... (*) X` instead, where the `X` we are looking for appears at the
end. In that case, we have to restart our search, but looking for the
lookahead for `Foo`.

The next state in our case is `(S1, X = (*) e)`. Again the `(*)` lies
at the beginning and hence we search for things in the state S1 where
`X` is the next symbol. We find `X = "e" (*) X`. This is not as good
as last time, because there are no symbols appearing after X in this
item, so it does not contribute any lookahead. We therefore can't stop
our search yet, but we push the state `(S1, X = "e" (*) X)` -- this
corresponds to the `Foo` state I mentioned at the end of the last
paragraph, except that in this case `Foo` is the same nonterminal `X`
we started with.

Looking at `(S1, X = "e" (*) X)`, we again have the `(*)` in the
middle of the item, so we move it left, searching for predecessors
with the item `X = (*) e X`. We will (again) find S0 and S1 have such
items. In the case of S0, we will (again) find the context "c", which
we dutifully add to the table (this has no effect, since it is already
present). In the case of S1, we will (again) wind up at the state
`(S1, X = "e" (*) X)`.  Since we've already visited this state, we
stop our search, it will not lead to new context.

At this point, our table column for C1 is complete. We can repeat the
process for C2, which plays out in an analogous way.

##### Step 3b: Update the lookahead

Looking at the lane table we built, we can union the context sets in
any particular column. We see that the context sets for each
conflicting action are pairwise disjoint. Therefore, we can simply
update each reduce action in our state with those lookaheads in mind,
and hence render it consistent:

```
S1 = X = (*) "e"     [_]
   | X = "e" (*)     ["c"] // lookahead from C1
   | X = (*) "e" "X" [_]
   | X = "e" (*) X   [_]
   | Y = (*) "e"     [_]
   | Y = "e" (*)     ["d"] // lookahead from C2
   | Y = (*) "e" Y   [_]
   | Y = "e" (*) Y   [_]
```

This is of course also what the LALR(1) state would look like (though
it would include context for the other items, though that doesn't play
into the final machine execution).

At this point we've covered enough to handle the grammar G0.  Let's
turn to a more complex grammar, grammar G1, and then we'll come back
to cover the remaining steps.

### Second example: the grammar G1

G1 is a (typo corrected) version of the grammar from the paper. This
grammar is not LALR(1) and hence it is more interesting, because it
requires splitting states.

#### Grammar G1

```
G1 = "a" X "d"
   | "a" Y "c"
   | "b" X "c"
   | "b" Y "d"
X  = "e" X
   | "e"
Y  = "e" Y
   | "e"
```

The key point of this grammar is that when we see `... "e" "c"` and we
wish to know whether to reduce to `X` or `Y`, we don't have enough
information. We need to know what is in the `...`, because `"a" "e"
"c"` means we reduce `"e"` to `Y` and `"b" "e" "c"` means we reduce to
`X`. In terms of our *state machine*, this corresponds to *splitting*
the states responsible for X and Y based on earlier context.

Let's look at a subset of the LR(0) states for G1:

```
S0 = G0 = (*) "a" X "d"
   | G0 = (*) "a" Y "c"
   | G0 = (*) "b" X "c"
   | G0 = (*) "b" X "d"
   
S1 = G0 = "a" (*) X "d"
   | G0 = "a" (*) Y "c"
   | X = (*) "e" X
   | X = (*) "e"
   | Y = (*) "e" Y
   | Y = (*) "e"

S2 = G0 = "b" (*) X "c"
   | G0 = "b" (*) Y "d"
   | X = (*) "e" X
   | X = (*) "e"
   | Y = (*) "e" Y
   | Y = (*) "e"

S3 = X = "e" (*) X
   | X = "e" (*)      // C1 -- can reduce
   | X = (*) "e"      // C0 -- can shift "e"
   | X = (*) "e" "X"  // C0 -- can shift "e"
   | Y = "e" (*) Y
   | Y = "e" (*)      // C2 -- can reduce
   | Y = (*) "e"      // C0 -- can shift "e"
   | Y = (*) "e" Y    // C0 -- can shift "e"
```

Here we can see the problem. The state S3 is inconsistent. But it is
reachable from both S1 and S2. If we come from S1, then we can have (e.g.)
`X "d"`, but if we come from S2, we expect `X "c"`.

Let's walk through our algorithm again. I'll start with step 3a.

### Step 3a: Build the lane table.

The lane table for state S3 will look like this:

```
| State | C0    | C1    | C2    | Successors |
| S1    |       | ["d"] | ["c"] | {S3}       |
| S2    |       | ["c"] | ["d"] | {S3}       |
| S3    | ["e"] | []    | []    | {S3}       |
```

Now if we union each column, we see that both C1 and C2 wind up with
lookahead `{"c", "d"}`. This is our problem. We have to isolate things
better. Therefore, step 3b ("update lookahead") does not apply. Instead
we attempt step 3c.

### Step 3c: Isolate lanes

This part of the algorithm is only loosely described in the paper, but
I think it works as follows. We will employ a union-find data
structure. With each set, we will record a "context set", which
records for each conflict the set of lookahead tokens (e.g.,
`{C1:{"d"}}`).

A context set tells us how to map the lookahead to an action;
therefire, to be self-consistent, the lookaheads for each conflict
must be mutually disjoint. In other words, `{C1:{"d"}, C2:{"c"}}` is
valid, and says to do C1 if we see a "d" and C2 if we see a "c". But
`{C1:{"d"}, C2:{"d"}}` is not, because there are two actions.

Initially, each state in the lane table is mapped to itself, and the
conflict set is derived from its column in the lane table:

```
S1 = {C1:d, C2:c}
S2 = {C1:c, C2:d}
S3 = {C0:e}
```

We designate "beachhead" states as those states in the table that are
not reachable from another state in the table (i.e., using the
successors). In this case, those are the states S1 and S2. We will be
doing a DFS through the table and we want to use those as the starting
points.

(Question: is there always at least one beachhead state? Seems like
there must be.)

So we begin by iterating over the beachhead states.

```rust
for beachhead in beachheads { ... }
```

When we visit a state X, we will examine each of its successors Y. We
consider whether the context set for Y can be merged with the context
set for X. So, in our case, X will be S1 to start and Y will be S3.
In this case, the context set can be merged, and hence we union S1, S3
and wind up with the following union-find state:

```
S1,S3 = {C0:e, C1:d, C2:c}
S2    = {C1:c, C2:d}
```

(Note that this union is just for the purpose of tracking context; it
doesn't imply that S1 and S3 are the 'same states' or anything like
that.)

Next we examine the edge S3 -> S3. Here the contexts are already
merged and everything is happy, so we stop. (We already visited S3,
after all.)

This finishes our first beachhead, so we proceed to the next edge, S2
-> S3. Here we find that we **cannot** union the context: it would
produce an inconsistent state. So what we do is we **clone** S3 to
make a new state, S3', with the initial setup corresponding to the row
for S3 from the lane table:

```
S1,S3 = {C0:e, C1:d, C2:c}
S2    = {C1:c, C2:d}
S3'   = {C0:e}
```

This also involves updating our LR(0-1) state set to have a new state
S3'. All edges from S2 that led to S3 now lead to S3'; the outgoing
edges from S3' remain unchanged. (At least to start.)

Therefore, the edge `S2 -> S3` is now `S2 -> S3'`. We can now merge
the conflicts:

```
S1,S3  = {C0:e, C1:d, C2:c}
S2,S3' = {C0:e, C1:c, C2:d}
```

Now we examine the outgoing edge S3' -> S3. We cannot merge these
conflicts, so we search (greedily, I guess) for a clone of S3 where we
can merge the conflicts. We find one in S3', and hence we redirect the
S3 edge to S3' and we are done. (I think the actual search we want is
to make first look for a clone of S3 that is using literally the same
context as us (i.e., same root node), as in this case. If that is not
found, *then* we search for one with a mergeable context. If *that*
fails, then we clone a new state.)

The final state thus has two copies of S3, one for the path from S1,
and one for the path from S2, which gives us enough context to
proceed.
