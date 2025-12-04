This language has function calls similar to Ruby's, with no parentheses required, and optional blocks.

There is a shift/reduce conflict here:

```
foo bar { baz }
       ^
```

The possible actions are:

1. `reduce(expression, 1)` - `bar` is an expression being passed to the `foo` function.
2. `shift` - `bar` is a function being called with the block `{ baz }`

The grammars `precedence_on_single_child_negative` and `precedence_on_single_child_positive` show possible resolutions to this conflict.
