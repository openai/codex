# nu-ansi-term

> This is a copy of rust-ansi-term but with Colour change to Color and light foreground colors added (90-97) as well as light background colors added (100-107).

This is a library for controlling colors and formatting, such as red bold text or blue underlined text, on ANSI terminals.

## [View the Rustdoc](https://docs.rs/nu_ansi_term/)

## Installation

This crate works with [Cargo](http://crates.io). Add the following to your `Cargo.toml` dependencies section:

```toml
[dependencies]
nu-ansi-term = "0.49"

# optional gnu-legacy mode to have two digit instead of one digit styles
nu-ansi-term = { version="0.49", features=["gnu_legacy"] }
```

## Basic usage

There are three main types in this crate that you need to be concerned with: `AnsiString`, `Style`, and `Color`.

A `Style` holds stylistic information: foreground and background colors, whether the text should be bold, or blinking, or other properties.
The `Color` enum represents the available colors.
And an `AnsiString` is a string paired with a `Style`.

To format a string, call the `paint` method on a `Style` or a `Color`, passing in the string you want to format as the argument.
For example, here’s how to get some red text:

```rust
use nu_ansi_term::Color::Red;

println!("This is in red: {}", Red.paint("a red string"));
```

It’s important to note that the `paint` method does _not_ actually return a string with the ANSI control characters surrounding it.
Instead, it returns an `AnsiString` value that has a `Display` implementation that, when formatted, returns the characters.
This allows strings to be printed with a minimum of `String` allocations being performed behind the scenes.

If you _do_ want to get at the escape codes, then you can convert the `AnsiString` to a string as you would any other `Display` value:

```rust
use nu_ansi_term::Color::Red;

let red_string = Red.paint("a red string").to_string();
```

**Note for Windows 10 users:** On Windows 10, the application must enable ANSI support first:

```rust,ignore
let enabled = nu_ansi_term::enable_ansi_support();
```

## Bold, underline, background, and other styles

For anything more complex than plain foreground color changes, you need to construct `Style` values themselves, rather than beginning with a `Color`.
You can do this by chaining methods based on a new `Style`, created with `Style::new()`.
Each method creates a new style that has that specific property set.
For example:

```rust
use nu_ansi_term::Style;

println!("How about some {} and {}?",
         Style::new().bold().paint("bold"),
         Style::new().underline().paint("underline"));
```

For brevity, these methods have also been implemented for `Color` values, so you can give your styles a foreground color without having to begin with an empty `Style` value:

```rust
use nu_ansi_term::Color::{Blue, Yellow};

println!("Demonstrating {} and {}!",
         Blue.bold().paint("blue bold"),
         Yellow.underline().paint("yellow underline"));

println!("Yellow on blue: {}", Yellow.on(Blue).paint("wow!"));
```

The complete list of styles you can use are:
`bold`, `dimmed`, `italic`, `underline`, `blink`, `reverse`, `hidden`, and `on` for background colors.

In some cases, you may find it easier to change the foreground on an existing `Style` rather than starting from the appropriate `Color`.
You can do this using the `fg` method:

```rust
use nu_ansi_term::Style;
use nu_ansi_term::Color::{Blue, Cyan, Yellow};

println!("Yellow on blue: {}", Style::new().on(Blue).fg(Yellow).paint("yow!"));
println!("Also yellow on blue: {}", Cyan.on(Blue).fg(Yellow).paint("zow!"));
```

You can turn a `Color` into a `Style` with the `normal` method.
This will produce the exact same `AnsiString` as if you just used the `paint` method on the `Color` directly, but it’s useful in certain cases: for example, you may have a method that returns `Styles`, and need to represent both the “red bold” and “red, but not bold” styles with values of the same type. The `Style` struct also has a `Default` implementation if you want to have a style with _nothing_ set.

```rust
use nu_ansi_term::Style;
use nu_ansi_term::Color::Red;

Red.normal().paint("yet another red string");
Style::default().paint("a completely regular string");
```

Sometimes it is desirable to hard-reset a style/color just before
applying a new one. To reset and apply, the `reset_before_style` method can
be used on either `Color` or `Style` structs.

```rust
use nu_ansi_term::Style;

println!("\x1b[33mHow about some {} \x1b[33mand {}?\x1b[0m",
         Style::new().reset_before_style().bold().paint("bold"),
         Style::new().reset_before_style().underline().paint("underline"));
```

## Extended colors

You can access the extended range of 256 colors by using the `Color::Fixed` variant, which takes an argument of the color number to use.
This can be included wherever you would use a `Color`:

```rust
use nu_ansi_term::Color::Fixed;

Fixed(134).paint("A sort of light purple");
Fixed(221).on(Fixed(124)).paint("Mustard in the ketchup");
```

The first sixteen of these values are the same as the normal and bold standard color variants.
There’s nothing stopping you from using these as `Fixed` colors instead, but there’s nothing to be gained by doing so either.

You can also access full 24-bit color by using the `Color::RGB` variant, which takes separate `u8` arguments for red, green, and blue:

```rust
use nu_ansi_term::Color::RGB;

RGB(70, 130, 180).paint("Steel blue");
```

## Combining successive colored strings

The benefit of writing ANSI escape codes to the terminal is that they _stack_: you do not need to end every colored string with a reset code if the text that follows it is of a similar style.
For example, if you want to have some blue text followed by some blue bold text, it’s possible to send the ANSI code for blue, followed by the ANSI code for bold, and finishing with a reset code without having to have an extra one between the two strings.

This crate can optimise the ANSI codes that get printed in situations like this, making life easier for your terminal renderer.
The `AnsiStrings` struct takes a slice of several `AnsiString` values, and will iterate over each of them, printing only the codes for the styles that need to be updated as part of its formatting routine.

The following code snippet uses this to enclose a binary number displayed in red bold text inside some red, but not bold, brackets:

```rust
use nu_ansi_term::Color::Red;
use nu_ansi_term::{AnsiString, AnsiStrings};

let some_value = format!("{:b}", 42);
let strings: &[AnsiString<'static>] = &[
    Red.paint("["),
    Red.bold().paint(some_value),
    Red.paint("]"),
];

println!("Value: {}", AnsiStrings(strings));
```

There are several things to note here.
Firstly, the `paint` method can take _either_ an owned `String` or a borrowed `&str`.
Internally, an `AnsiString` holds a copy-on-write (`Cow`) string value to deal with both owned and borrowed strings at the same time.
This is used here to display a `String`, the result of the `format!` call, using the same mechanism as some statically-available `&str` slices.
Secondly, that the `AnsiStrings` value works in the same way as its singular counterpart, with a `Display` implementation that only performs the formatting when required.

## Byte strings

This library also supports formatting `[u8]` byte strings; this supports applications working with text in an unknown encoding.
`Style` and `Color` support painting `[u8]` values, resulting in an `AnsiByteString`.
This type does not implement `Display`, as it may not contain UTF-8, but it does provide a method `write_to` to write the result to any value that implements `Write`:

```rust
use nu_ansi_term::Color::Green;

Green.paint("user data".as_bytes()).write_to(&mut std::io::stdout()).unwrap();
```

Similarly, the type `AnsiByteStrings` supports writing a list of `AnsiByteString` values with minimal escape sequences:

```rust
use nu_ansi_term::Color::Green;
use nu_ansi_term::AnsiByteStrings;

AnsiByteStrings(&[
    Green.paint("user data 1\n".as_bytes()),
    Green.bold().paint("user data 2\n".as_bytes()),
]).write_to(&mut std::io::stdout()).unwrap();
```
