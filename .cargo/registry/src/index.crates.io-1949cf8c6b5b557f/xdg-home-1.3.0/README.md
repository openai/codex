# xdg-home

Gets the user's home directory as per [XDG Base Directory Specification][xdg].

This is almost the same as [`home`] (and [`dirs`]) crate, except it honors `HOME` environment
variable on the Windows platform as well, which is conformant to the XDG Base Directory
Specification.

Use it where the XDG Base Directory Specification is applicable, such as in [D-Bus] code.

## Example

```rust
use xdg_home::home_dir;

let home = home_dir().unwrap();
assert!(home.is_absolute());
assert!(home.exists());
println!("Home directory: {}", home.display());
```

[xdg]: https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
[`home`]: https://crates.io/crates/home
[`dirs`]: https://crates.io/crates/dirs
[D-Bus]: https://dbus.freedesktop.org/doc/dbus-specification.html
