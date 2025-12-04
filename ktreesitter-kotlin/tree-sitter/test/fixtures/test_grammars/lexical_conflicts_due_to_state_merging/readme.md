This grammar has two tokens, `regex` and `/`, which conflict: when a `/` character is encountered, the lexer can't tell if it is part of a `/` token or a `regex` by looking ahead only one character. But because these tokens are never valid in the same position, this doesn't cause any problem.

When merging similar parse states in order to reduce the size of the parse table, it is important that we avoid merging states in a way that causes these two tokens to both appear as valid lookahead symbols in a given state.

If we weren't careful, this grammar would cause that to happen, because a `regex` is valid in this state:

```
(if (1) /\w+/)
       ^
```

and a `/` is valid in this state:


```
((1) / 2)
    ^
```

And these two states would otherwise be candidates for merging, because they both contain only the action `reduce(parenthesized, 3)`.