## Debugging handler type errors

For a function to be used as a handler it must implement the [`Handler`] trait.
axum provides blanket implementations for functions that:

- Are `async fn`s.
- Take no more than 16 arguments that all implement `Send`.
  - All except the last argument implement [`FromRequestParts`].
  - The last argument implements [`FromRequest`].
- Returns something that implements [`IntoResponse`].
- If a closure is used it must implement `Clone + Send` and be
`'static`.
- Returns a future that is `Send`. The most common way to accidentally make a
future `!Send` is to hold a `!Send` type across an await.

Unfortunately Rust gives poor error messages if you try to use a function
that doesn't quite match what's required by [`Handler`].

You might get an error like this:

```not_rust
error[E0277]: the trait bound `fn(bool) -> impl Future {handler}: Handler<_, _>` is not satisfied
   --> src/main.rs:13:44
    |
13  |     let app = Router::new().route("/", get(handler));
    |                                            ^^^^^^^ the trait `Handler<_, _>` is not implemented for `fn(bool) -> impl Future {handler}`
    |
   ::: axum/src/handler/mod.rs:116:8
    |
116 |     H: Handler<T, B>,
    |        ------------- required by this bound in `axum::routing::get`
```

This error doesn't tell you _why_ your function doesn't implement
[`Handler`]. It's possible to improve the error with the [`debug_handler`]
proc-macro from the [axum-macros] crate.

[axum-macros]: https://docs.rs/axum-macros
[`debug_handler`]: https://docs.rs/axum-macros/latest/axum_macros/attr.debug_handler.html
