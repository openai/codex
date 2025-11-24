# rust-uname

Name and information about current kernel

## Dashboard

| Linux CI | Test Coverage | Crate | Documentation |
|:--------:|:-------------:|:-----:|:-------------:|
| [![Build Status](https://travis-ci.org/icorderi/rust-uname.svg?branch=master)](https://travis-ci.org/icorderi/rust-uname) | [![Coverage Status](https://coveralls.io/repos/icorderi/rust-uname/badge.svg?branch=master)](https://coveralls.io/r/icorderi/rust-uname?branch=master) | [![Crate](http://meritbadge.herokuapp.com/uname)](https://crates.io/crates/uname) | [![Docs](https://img.shields.io/badge/docs-up--to--date-blue.svg)](https://icorderi.github.io/rust-uname/index.html)

## Basic usage

```rust
extern crate uname;

use uname::uname;

fn main() {
    let info = uname().unwrap();

    // Print the hostname
    println!("{}", info.hostname);
    // Print everything
    println!("{:?}", info);
}
```

Don't forget to check out the [examples](./examples)

## License

Licensed under:

- Apache License, Version 2.0 - [LICENSE-APACHE](LICENSE-APACHE) ([source](http://www.apache.org/licenses/LICENSE-2.0))
- MIT license - ([LICENSE-MIT](LICENSE-MIT) ([source](http://opensource.org/licenses/MIT))

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
