# Query Syntax

A _query_ consists of one or more _patterns_, where each pattern is an [S-expression][s-exp] that matches a certain set of
nodes in a syntax tree. The expression to match a given node consists of a pair of parentheses containing two things: the
node's type, and optionally, a series of other S-expressions that match the node's children. For example, this pattern would
match any `binary_expression` node whose children are both `number_literal` nodes:

```query
(binary_expression (number_literal) (number_literal))
```

Children can also be omitted. For example, this would match any `binary_expression` where at least _one_ of child is a
`string_literal` node:

```query
(binary_expression (string_literal))
```

## Fields

In general, it's a good idea to make patterns more specific by specifying [field names][node-field-names] associated with
child nodes. You do this by prefixing a child pattern with a field name followed by a colon. For example, this pattern would
match an `assignment_expression` node where the `left` child is a `member_expression` whose `object` is a `call_expression`.

```query
(assignment_expression
  left: (member_expression
    object: (call_expression)))
```

## Negated Fields

You can also constrain a pattern so that it only matches nodes that _lack_ a certain field. To do this, add a field name
prefixed by a `!` within the parent pattern. For example, this pattern would match a class declaration with no type parameters:

```query
(class_declaration
  name: (identifier) @class_name
  !type_parameters)
```

## Anonymous Nodes

The parenthesized syntax for writing nodes only applies to [named nodes][named-vs-anonymous-nodes]. To match specific anonymous
nodes, you write their name between double quotes. For example, this pattern would match any `binary_expression` where the
operator is `!=` and the right side is `null`:

```query
(binary_expression
  operator: "!="
  right: (null))
```

## Special Nodes

### The Wildcard Node

A wildcard node is represented with an underscore (`_`), it matches any node.
This is similar to `.` in regular expressions.
There are two types, `(_)` will match any named node,
and `_` will match any named or anonymous node.

For example, this pattern would match any node inside a call:

```query
(call (_) @call.inner)
```

### The `ERROR` Node

When the parser encounters text it does not recognize, it represents this node
as `(ERROR)` in the syntax tree. These error nodes can be queried just like
normal nodes:

```scheme
(ERROR) @error-node
```

### The `MISSING` Node

If the parser is able to recover from erroneous text by inserting a missing token and then reducing, it will insert that
missing node in the final tree so long as that tree has the lowest error cost. These missing nodes appear as seemingly normal
nodes in the tree, but they are zero tokens wide, and are internally represented as a property of the actual terminal node
that was inserted, instead of being its own kind of node, like the `ERROR` node. These special missing nodes can be queried
using `(MISSING)`:

```scheme
(MISSING) @missing-node
```

This is useful when attempting to detect all syntax errors in a given parse tree, since these missing node are not captured
by `(ERROR)` queries. Specific missing node types can also be queried:

```scheme
(MISSING identifier) @missing-identifier
(MISSING ";") @missing-semicolon
```

[node-field-names]: ../2-basic-parsing.md#node-field-names
[named-vs-anonymous-nodes]: ../2-basic-parsing.md#named-vs-anonymous-nodes
[s-exp]: https://en.wikipedia.org/wiki/S-expression
