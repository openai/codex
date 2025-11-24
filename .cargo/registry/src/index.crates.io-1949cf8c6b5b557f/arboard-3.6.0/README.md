# Arboard (Arthur's Clipboard)

[![Latest version](https://img.shields.io/crates/v/arboard?color=mediumvioletred)](https://crates.io/crates/arboard)
[![Documentation](https://docs.rs/arboard/badge.svg)](https://docs.rs/arboard)
![MSRV](https://img.shields.io/badge/rustc-1.71.0+-blue.svg)

## General

This is a cross-platform library for interacting with the clipboard. It allows
to copy and paste both text and image data in a platform independent way on
Linux, Mac, and Windows.

Please note that this is not an official 1Password product. Feature requests will be considered like any other volunteer-based crate.

## GNU/Linux

### Backend Support

By default, `arboard`'s backend on Linux supports X11 (or XWayland implementations) and uses
that for managing the various Linux clipboard variants. This supports the majority of desktop 
environments that exist in the wild today. `arboard` will use the `Clipboard` selection by default,
but the [LinuxClipboardKind](https://docs.rs/arboard/latest/arboard/enum.LinuxClipboardKind.html)
selector lets you operate on the `Primary` or `Secondary` clipboard selections (if supported).

However, Wayland is becoming the majority default as of 2025 (with some distributions)
even considering the removal of X by default. To support Wayland correctly, `arboard` users
should enable the `wayland-data-control` feature. If enabled, it will be prioritized over the X clipboard.

Wayland support is not enabled by default because it may be counterintuitive 
to some users: it relies on the data-control extension protocols,
which _are not_ support all Wayland compositors. You can check compositor support on `wayland.app`:
- [ext-data-control-v1](https://wayland.app/protocols/ext-data-control-v1)
- [wlr-data-control-unstable-v1](https://wayland.app/protocols/wlr-data-control-unstable-v1)

If you or a user's desktop doesn't support these protocols, `arboard` won't function in a pure
Wayland environment. It is recommended to enable `XWayland` for these cases. If your app runs inside
an isolated sandbox, such as Flatpak or Snap, you'll need to expose the X11 socket to the application
_in addition_ to the Wayland communication interface.

### Clipboard Ownership

Some apps and users may notice that sometimes values copied to a Linux clipboard with this crate vanish
before anyone gets the chance to paste, or just aren't available when you expect them to be. The root behind
these problems is _selection ownership_.

X11 and Wayland put the responsibility for answering paste requests and serving data on the application
which originally copied it onto the clipboard. This usually means the app using `arboard`. Nothing you copy
to the clipboard is sent anywhere to start. It stays inside `arboard` until something else on the system requests it,
which is very different to how the clipboard works on other platforms like macOS or Windows.

Note that `arboard` may attempt to warn you about these conditions when compiled in debug mode, to improve the debugging
experience. Even if you don't see these warnings, you should double check the lifetime of the `Clipboard` in your code.

In some cases, an environment may have a clipboard manager installed. These services monitor the clipboard contents and
do their best to retain a copy when needed to smooth over clipboard ownership changes. A clipboar manager can make contents
available even after a process previously owning it exits.

In order to keep the contents around longer, make sure that you don't `Drop` your `Clipboard` object right away or
terminate the copying process too fast. This is why, at times, adding a call to `sleep()` near the set operation
makes it behave more reliabily: the background thread `arboard` uses for serving clipboard contents has more time to run
and let other apps (including clipboard managers) make requests for the contents. However `sleep` isn't the recommended approach.

If your application is exiting, you must make sure there is a clipboard manager running on the system. If nothing is listening for 
the clipboard ownership transfer, or made a copy previously, the data will be lost. Note that this isn't a complete 
guarantee as races are possible if your program's main thread is exiting. If you would like to fully synchronize the clipboard "paste"
before exiting, you can use the [wait](https://docs.rs/arboard/latest/arboard/trait.SetExtLinux.html#tymethod.wait) method when setting
contents on the clipboard. This will block the calling thread until another app has requested, and then received, the data.

If your application is longer-running (ie a GUI, TUI, etc), it is highly recommended that you either store the `Clipboard` object in some 
long-lived data structure (like app context, etc) or utilize `wait` method mentioned above, and/or threading to make sure another 
app can request the clipboard data later.

We welcome suggestions to improve on the above issues in ways that don't degrade other use cases.

## Example

```rust
use arboard::Clipboard;

fn main() {
    let mut clipboard = Clipboard::new().unwrap();
    println!("Clipboard text was: {}", clipboard.get_text().unwrap());

    let the_string = "Hello, world!";
    clipboard.set_text(the_string).unwrap();
    println!("But now the clipboard text should be: \"{}\"", the_string);
}
```

## Credits

This crate is a combined effort by 1Password staff and `@ArturKovacs`, the crate's past
maintainer.

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE.txt">Apache License, Version
2.0</a> or <a href="LICENSE-MIT.txt">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>

#### History: Yet another clipboard crate

This crate started out as a fork of `rust-clipboard`. The reason for forking is due to the former
crate not being maintained any longer. At this point, `arboard`'s backends and public APIs have diverged
a lot.

`arboard`'s original maintainer noted that "I don't know why this is happening but while it is, we might 
as well just start naming the clipboard crates after ourselves. This one is arboard which stands for Artur's clipboard.".
