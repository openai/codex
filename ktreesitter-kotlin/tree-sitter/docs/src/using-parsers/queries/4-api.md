# The Query API

Create a query by specifying a string containing one or more patterns:

```c
TSQuery *ts_query_new(
  const TSLanguage *language,
  const char *source,
  uint32_t source_len,
  uint32_t *error_offset,
  TSQueryError *error_type
);
```

If there is an error in the query, then the `error_offset` argument will be set to the byte offset of the error, and the
`error_type` argument will be set to a value that indicates the type of error:

```c
typedef enum {
  TSQueryErrorNone = 0,
  TSQueryErrorSyntax,
  TSQueryErrorNodeType,
  TSQueryErrorField,
  TSQueryErrorCapture,
} TSQueryError;
```

The `TSQuery` value is immutable and can be safely shared between threads. To execute the query, create a `TSQueryCursor`,
which carries the state needed for processing the queries. The query cursor should not be shared between threads, but can
be reused for many query executions.

```c
TSQueryCursor *ts_query_cursor_new(void);
```

You can then execute the query on a given syntax node:

```c
void ts_query_cursor_exec(TSQueryCursor *, const TSQuery *, TSNode);
```

You can then iterate over the matches:

```c
typedef struct {
  TSNode node;
  uint32_t index;
} TSQueryCapture;

typedef struct {
  uint32_t id;
  uint16_t pattern_index;
  uint16_t capture_count;
  const TSQueryCapture *captures;
} TSQueryMatch;

bool ts_query_cursor_next_match(TSQueryCursor *, TSQueryMatch *match);
```

This function will return `false` when there are no more matches. Otherwise, it will populate the `match` with data about
which pattern matched and which nodes were captured.
