#!/usr/bin/env sh

echo "color: foreground"
cargo run -p anstyle --example dump-style -- --layer fg
echo
echo "color: background"
cargo run -p anstyle --example dump-style -- --layer bg
echo
echo "color: underline"
cargo run -p anstyle --example dump-style -- --layer underline
echo
echo "italic"
cargo run -p anstyle --example dump-style -- --effect italic
echo
echo "bold"
cargo run -p anstyle --example dump-style -- --effect bold
echo
echo "dimmed"
cargo run -p anstyle --example dump-style -- --effect dimmed
echo
echo "underline"
cargo run -p anstyle --example dump-style -- --effect underline
echo
echo "double_underline"
cargo run -p anstyle --example dump-style -- --effect double_underline
echo
echo "curly_underline"
cargo run -p anstyle --example dump-style -- --effect curly_underline
echo
echo "dotted_underline"
cargo run -p anstyle --example dump-style -- --effect dotted_underline
echo
echo "dashed_underline"
cargo run -p anstyle --example dump-style -- --effect dashed_underline
echo
echo "blink"
cargo run -p anstyle --example dump-style -- --effect blink
echo
echo "invert"
cargo run -p anstyle --example dump-style -- --effect invert
echo
echo "hidden"
cargo run -p anstyle --example dump-style -- --effect hidden
echo
echo "strikethrough"
cargo run -p anstyle --example dump-style -- --effect strikethrough
