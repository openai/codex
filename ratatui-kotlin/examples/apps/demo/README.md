# Demo example

This is the original demo that was developed for Tui-rs (the library that Ratatui was forked from).

![demo.gif](https://github.com/ratatui/ratatui/blob/images/examples/demo.gif?raw=true)

This example is available for each backend. To run it:

## crossterm

```shell
cargo run -p demo
```

## termion

```shell
cargo run -p demo --no-default-features --features termion
```

## termwiz

```shell
cargo run -p demo --no-default-features --features termwiz
```
