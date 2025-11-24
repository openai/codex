# 0.3.6

* Fix soundness bugs (#109)
* Bump MSRV to 1.65 (#109)

# 0.3.5

* Update to syn 2.0 (#93)
* Bump MSRV to 1.56 (#97)

# 0.3.4

* Improve support for `#[track_caller]` (#72)
* Reduce unsafe code (#77)

# 0.3.3

* Fix a bug where `yield` and `?` cannot be used on the same line (#66)

# 0.3.2

* Expand `yield` in internal macro calls (#57)

# 0.3.1

* Support reexporting (#46)
* Allow yielding `!Unpin` values (#50)
* Implement `Stream::size_hint` method on `AsyncStream` (#40)
* Documentation improvements

# 0.3.0

* Remove proc-macro-hack (#30)
