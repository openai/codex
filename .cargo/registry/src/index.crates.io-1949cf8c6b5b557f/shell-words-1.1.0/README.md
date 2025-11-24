# shell-words

Process command line according to parsing rules of Unix shell.

## Usage

Add this to Cargo.toml:
```toml
[dependencies]
shell-words = "1.0.0"
```

Add this to your crate:
```rust
extern crate shell_words;
```

## Examples

### Split

Compiling C source code into an executable as in default build rule found in GNU Make:

```rust
extern crate shell_words;

use std::env::var;
use std::process::Command;

fn main() {
    let cc = var("CC").unwrap_or_else(|_| "cc".to_owned());

    let cflags = var("CFLAGS").unwrap_or_else(|_| String::new());
    let cflags = shell_words::split(&cflags).expect("failed to parse CFLAGS");

    let cppflags = var("CPPFLAGS").unwrap_or_else(|_| String::new());
    let cppflags = shell_words::split(&cppflags).expect("failed to parse CPPFLAGS");

    Command::new(cc)
        .args(cflags)
        .args(cppflags)
        .args(&["-c", "a.c", "-o", "a.out"])
        .spawn()
        .expect("failed to start subprocess")
        .wait()
        .expect("failed to wait for subprocess");
}
```

### Join

Logging executed commands in format that can be readily copied and pasted to a shell:

```rust
extern crate shell_words;

fn main() {
    let argv = &["python", "-c", "print('Hello world!')"];

    println!("Executing: {}", shell_words::join(argv));

    std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .spawn()
        .expect("failed to start subprocess")
        .wait()
        .expect("failed to wait for subprocess");
}
```

## Bugs

Please report any issues at https://github.com/tmiasko/shell-words/issues.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
