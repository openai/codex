# Walking Trees with Tree Cursors

You can access every node in a syntax tree using the `TSNode` APIs [described earlier][retrieving nodes], but if you need
to access a large number of nodes, the fastest way to do so is with a _tree cursor_. A cursor is a stateful object that
allows you to walk a syntax tree with maximum efficiency.

```admonish note
The given input node is considered the root of the cursor, and the cursor cannot walk outside this node.
Going to the parent or any sibling of the root node will always return `false`.

This has no unexpected effects if the given input node is the actual `root` node of the tree, but is something to keep in
mind when using cursors constructed with a node that is not the `root` node.
```

You can initialize a cursor from any node:

```c
TSTreeCursor ts_tree_cursor_new(TSNode);
```

You can move the cursor around the tree:

```c
bool ts_tree_cursor_goto_first_child(TSTreeCursor *);
bool ts_tree_cursor_goto_next_sibling(TSTreeCursor *);
bool ts_tree_cursor_goto_parent(TSTreeCursor *);
```

These methods return `true` if the cursor successfully moved and `false` if there was no node to move to.

You can always retrieve the cursor's current node, as well as the [field name][node-field-names] that is associated with
the current node.

```c
TSNode ts_tree_cursor_current_node(const TSTreeCursor *);
const char *ts_tree_cursor_current_field_name(const TSTreeCursor *);
TSFieldId ts_tree_cursor_current_field_id(const TSTreeCursor *);
```

[retrieving nodes]: ./2-basic-parsing.md#retrieving-nodes
[node-field-names]: ./2-basic-parsing.md#node-field-names
