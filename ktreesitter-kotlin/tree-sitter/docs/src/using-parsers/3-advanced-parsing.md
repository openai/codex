# Advanced Parsing

## Editing

In applications like text editors, you often need to re-parse a file after its source code has changed. Tree-sitter is designed
to support this use case efficiently. There are two steps required. First, you must _edit_ the syntax tree, which adjusts
the ranges of its nodes so that they stay in sync with the code.

```c
typedef struct {
  uint32_t start_byte;
  uint32_t old_end_byte;
  uint32_t new_end_byte;
  TSPoint start_point;
  TSPoint old_end_point;
  TSPoint new_end_point;
} TSInputEdit;

void ts_tree_edit(TSTree *, const TSInputEdit *);
```

Then, you can call `ts_parser_parse` again, passing in the old tree. This will create a new tree that internally shares structure
with the old tree.

When you edit a syntax tree, the positions of its nodes will change. If you have stored any `TSNode` instances outside of
the `TSTree`, you must update their positions separately, using the same `TSInput` value, in order to update their
cached positions.

```c
void ts_node_edit(TSNode *, const TSInputEdit *);
```

This `ts_node_edit` function is _only_ needed in the case where you have retrieved `TSNode` instances _before_ editing the
tree, and then _after_ editing the tree, you want to continue to use those specific node instances. Often, you'll just want
to re-fetch nodes from the edited tree, in which case `ts_node_edit` is not needed.

## Multi-language Documents

Sometimes, different parts of a file may be written in different languages. For example, templating languages like [EJS][ejs]
and [ERB][erb] allow you to generate HTML by writing a mixture of HTML and another language like JavaScript or Ruby.

Tree-sitter handles these types of documents by allowing you to create a syntax tree based on the text in certain
_ranges_ of a file.

```c
typedef struct {
  TSPoint start_point;
  TSPoint end_point;
  uint32_t start_byte;
  uint32_t end_byte;
} TSRange;

void ts_parser_set_included_ranges(
  TSParser *self,
  const TSRange *ranges,
  uint32_t range_count
);
```

For example, consider this ERB document:

```erb
<ul>
  <% people.each do |person| %>
    <li><%= person.name %></li>
  <% end %>
</ul>
```

Conceptually, it can be represented by three syntax trees with overlapping ranges: an ERB syntax tree, a Ruby syntax tree,
and an HTML syntax tree. You could generate these syntax trees with the following code:

```c
#include <string.h>
#include <tree_sitter/api.h>

// These functions are each implemented in their own repo.
const TSLanguage *tree_sitter_embedded_template(void);
const TSLanguage *tree_sitter_html(void);
const TSLanguage *tree_sitter_ruby(void);

int main(int argc, const char **argv) {
  const char *text = argv[1];
  unsigned len = strlen(text);

  // Parse the entire text as ERB.
  TSParser *parser = ts_parser_new();
  ts_parser_set_language(parser, tree_sitter_embedded_template());
  TSTree *erb_tree = ts_parser_parse_string(parser, NULL, text, len);
  TSNode erb_root_node = ts_tree_root_node(erb_tree);

  // In the ERB syntax tree, find the ranges of the `content` nodes,
  // which represent the underlying HTML, and the `code` nodes, which
  // represent the interpolated Ruby.
  TSRange html_ranges[10];
  TSRange ruby_ranges[10];
  unsigned html_range_count = 0;
  unsigned ruby_range_count = 0;
  unsigned child_count = ts_node_child_count(erb_root_node);

  for (unsigned i = 0; i < child_count; i++) {
    TSNode node = ts_node_child(erb_root_node, i);
    if (strcmp(ts_node_type(node), "content") == 0) {
      html_ranges[html_range_count++] = (TSRange) {
        ts_node_start_point(node),
        ts_node_end_point(node),
        ts_node_start_byte(node),
        ts_node_end_byte(node),
      };
    } else {
      TSNode code_node = ts_node_named_child(node, 0);
      ruby_ranges[ruby_range_count++] = (TSRange) {
        ts_node_start_point(code_node),
        ts_node_end_point(code_node),
        ts_node_start_byte(code_node),
        ts_node_end_byte(code_node),
      };
    }
  }

  // Use the HTML ranges to parse the HTML.
  ts_parser_set_language(parser, tree_sitter_html());
  ts_parser_set_included_ranges(parser, html_ranges, html_range_count);
  TSTree *html_tree = ts_parser_parse_string(parser, NULL, text, len);
  TSNode html_root_node = ts_tree_root_node(html_tree);

  // Use the Ruby ranges to parse the Ruby.
  ts_parser_set_language(parser, tree_sitter_ruby());
  ts_parser_set_included_ranges(parser, ruby_ranges, ruby_range_count);
  TSTree *ruby_tree = ts_parser_parse_string(parser, NULL, text, len);
  TSNode ruby_root_node = ts_tree_root_node(ruby_tree);

  // Print all three trees.
  char *erb_sexp = ts_node_string(erb_root_node);
  char *html_sexp = ts_node_string(html_root_node);
  char *ruby_sexp = ts_node_string(ruby_root_node);
  printf("ERB: %s\n", erb_sexp);
  printf("HTML: %s\n", html_sexp);
  printf("Ruby: %s\n", ruby_sexp);
  return 0;
}
```

This API allows for great flexibility in how languages can be composed. Tree-sitter is not responsible for mediating the
interactions between languages. Instead, you are free to do that using arbitrary application-specific logic.

## Concurrency

Tree-sitter supports multi-threaded use cases by making syntax trees very cheap to copy.

```c
TSTree *ts_tree_copy(const TSTree *);
```

Internally, copying a syntax tree just entails incrementing an atomic reference count. Conceptually, it provides you a new
tree which you can freely query, edit, reparse, or delete on a new thread while continuing to use the original tree on a
different thread.

```admonish danger
Individual `TSTree` instances are _not_ thread safe; you must copy a tree if you want to use it on multiple threads simultaneously.
```

[ejs]: https://ejs.co
[erb]: https://ruby-doc.org/stdlib-2.5.1/libdoc/erb/rdoc/ERB.html
