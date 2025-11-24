In axum a "handler" is an async function that accepts zero or more
["extractors"](crate::extract) as arguments and returns something that
can be converted [into a response](crate::response).

Handlers are where your application logic lives and axum applications are built
by routing between handlers.

[`debug_handler`]: https://docs.rs/axum-macros/latest/axum_macros/attr.debug_handler.html
