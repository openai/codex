# codex-utils-template

Small, strict string templating for prompt and text assets.

Supported syntax:

- `{{ name }}` placeholder interpolation
- `{{{{` for a literal `{{`
- `}}}}` for a literal `}}`

The library is intentionally strict:

- parsing fails on malformed placeholders
- rendering fails on missing values
- rendering fails on duplicate values
- rendering fails on extra values not used by the template

## Example

```rust
use codex_utils_template::Template;
use codex_utils_template::render;

let template = Template::parse(
    "Hello, {{ name }}.\nLiteral braces: {{{{ and }}}}.\nMode: {{ mode }}",
)?;

let rendered = template.render([
    ("name", "Codex"),
    ("mode", "strict"),
])?;

assert_eq!(
    rendered,
    "Hello, Codex.\nLiteral braces: {{ and }}.\nMode: strict"
);

let one_shot = render("Hi {{ who }}!", [("who", "there")])?;
assert_eq!(one_shot, "Hi there!");
# Ok::<(), Box<dyn std::error::Error>>(())
```
