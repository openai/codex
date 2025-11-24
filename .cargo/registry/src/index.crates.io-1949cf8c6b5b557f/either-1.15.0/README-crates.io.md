The enum `Either` with variants `Left` and `Right` is a general purpose
sum type with two cases.

Either has methods that are similar to Option and Result, and it also implements
traits like `Iterator`.

Includes macros `try_left!()` and `try_right!()` to use for
short-circuiting logic, similar to how the `?` operator is used with `Result`.
Note that `Either` is general purpose. For describing success or error, use the
regular `Result`.
