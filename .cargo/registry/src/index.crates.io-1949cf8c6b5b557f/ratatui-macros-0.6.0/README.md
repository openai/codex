# Ratatui Macros

[![Crates.io badge]][ratatui_macros crate] [![License badge]](./LICENSE)
[![Docs.rs badge]][API Docs] [![CI Badge]][CI Status]
[![Crate Downloads badge]][ratatui_macros crate]

`ratatui-macros` is a Rust crate that provides easy-to-use macros for simplifying boilerplate
associated with creating UI using [Ratatui].

This is an experimental playground for us to explore macros that would be useful to have in Ratatui
proper.

## Features

- Constraint-based Layouts: Easily define layout constraints such as fixed, percentage, minimum, and
  maximum sizes, as well as ratios.
- Directional Layouts: Specify layouts as either horizontal or vertical with simple macro commands.
- Span and Line macros: Make it easier to create spans and lines with styling.

## Getting Started

To use `ratatui-macros` in your Rust project, add it as a dependency in your `Cargo.toml`:

```shell
cargo add ratatui-macros
```

Then, import the macros in your Rust file:

```rust
use ratatui_macros::{
    constraint,
    constraints,
    horizontal,
    vertical,
    span,
    line,
};
```

### Layout

If you are new to Ratatui, check out the [Layout concepts] article on the Ratatui website before proceeding.

Use the `constraints!` macro to define layout constraints:

```rust
use ratatui::prelude::*;
use ratatui_macros::constraints;

assert_eq!(
    constraints![==50, ==30%, >=3, <=1, ==1/2, *=1],
    [
        Constraint::Length(50),
        Constraint::Percentage(30),
        Constraint::Min(3),
        Constraint::Max(1),
        Constraint::Ratio(1, 2),
        Constraint::Fill(1),
    ]
)
```

```rust
use ratatui::prelude::*;
use ratatui_macros::constraints;

assert_eq!(
    constraints![==1/4; 4],
    [
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ]
)
```

Use the `constraint!` macro to define individual constraints:

```rust
use ratatui::prelude::*;
use ratatui_macros::constraint;

assert_eq!(
    constraint!(==50),
    Constraint::Length(50),
)
```

Create vertical and horizontal layouts using the `vertical!` and `horizontal!` macros:

```rust
use ratatui::prelude::*;
use ratatui_macros::{vertical, horizontal};

let area = Rect { x: 0, y: 0, width: 10, height: 10 };

let [main, bottom] = vertical![==100%, >=3].areas(area);

assert_eq!(bottom.y, 7);
assert_eq!(bottom.height, 3);

let [left, main, right] = horizontal![>=3, ==100%, >=3].areas(area);

assert_eq!(left.width, 3);
assert_eq!(right.width, 3);
```

## Spans

The `span!` macro create raw and styled `Span`s. They each take a format string and arguments.
`span!` accepts as the first parameter any value that can be converted to a `Style` followed by a
`;` followed by the format string and arguments.

```rust
use ratatui::prelude::*;
use ratatui_macros::span;

let name = "world!";
let raw_greeting = span!("hello {name}");
let styled_greeting = span!(Style::new().green(); "hello {name}");
let styled_greeting = span!(Color::Green; "hello {name}");
let styled_greeting = span!(Modifier::BOLD; "hello {name}");
```

## Line

The `line!` macro creates a `Line` that contains a sequence of spans. It is similar to the `vec!`
macro.

```rust
use ratatui::prelude::*;
use ratatui_macros::line;

let name = "world!";
let line = line!["hello", format!("{name}")];
let line = line!["bye"; 2];
```

## Text

The `text!` macro creates a `Text` that contains a sequence of lines. It is similar to the `vec!`
macro.

```rust
use ratatui::prelude::*;
use ratatui_macros::{span, line, text};

let name = "world!";
let text = text!["hello", format!("{name}")];
let text = text!["bye"; 2];
```

It is even possible to use `span!` and `line!` in the `text!` macro:

```rust
use ratatui::prelude::*;
use ratatui_macros::{span, line, text};
let name = "Bye!!!";
let text = text![line!["hello", "world".bold()], span!(Modifier::BOLD; "{name}")];
```

## Row

The `row!` macro creates a `Row` that contains a sequence of `Cell`. It is similar to the `vec!`
macro. A `Row` represents a sequence of `Cell`s in a single row of a table.

```rust
use ratatui::prelude::*;
use ratatui_macros::row;

let rows = [
    row!["hello", "world"],
    row!["goodbye", "world"],
];
```

It is even possible to use `span!`, `line!` and `text!` in the `row!` macro:

```rust
use ratatui::prelude::*;
use ratatui_macros::{span, line, text, row};
let name = "Bye!!!";
let text = row![text![line!["hello", "world".bold()]], span!(Modifier::BOLD; "{name}")];
```

## Contributing

Contributions to `ratatui-macros` are welcome! Whether it's submitting a bug report, a feature
request, or a pull request, all forms of contributions are valued and appreciated.

[Crates.io badge]: https://img.shields.io/crates/v/ratatui-macros?logo=rust&style=flat-square
[License badge]: https://img.shields.io/crates/l/ratatui-macros
[CI Badge]:
  https://img.shields.io/github/actions/workflow/status/ratatui-org/ratatui-macros/ci.yml?logo=github&style=flat-square
[Docs.rs badge]: https://img.shields.io/docsrs/ratatui-macros?logo=rust&style=flat-square
[Crate Downloads badge]: https://img.shields.io/crates/d/ratatui-macros?logo=rust&style=flat-square
[ratatui_macros crate]: https://crates.io/crates/ratatui-macros
[API Docs]: https://docs.rs/ratatui-macros
[CI Status]: https://github.com/kdheepak/ratatui-macros/actions
[Ratatui]: https://github.com/ratatui-org/ratatui
[Layout concepts]: https://ratatui.rs/concepts/layout
