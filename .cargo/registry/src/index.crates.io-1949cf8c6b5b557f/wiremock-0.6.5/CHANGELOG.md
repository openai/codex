# Changelog

# Next

# `0.5.8`

- Features:
    - Functions from `&Request` to `ResponseTemplate` can now be passed to `MockBuilder::respond_with`. You do not have to write a struct with a `Respond` implementation for simple manipulation of request data! (by [@RoGryza])

# `0.5.7`

- Fixes:
    - `MockGuard` is now marked as `must_use`, ensuring that a compiler warning is raised if the guard for a scoped mock is not bound to a variable.

# `0.5.6`

- Features:
    - Added support for **scoped** `Mock`s! 
      Using `MockServer::register_as_scoped` or `Mock::mount_as_scoped` you can now register `Mock`s that go "out of scope" when the returned RAII guard (`MockGuard`) is dropped.
      Scoped `Mock`s are recommended for usage in test helper functions to ensure proper isolation - check the documentation for more details! (by [@LukeMathWalker])

# `0.5.5`

- Miscellaneous:
    - Added the `http` module to re-export the types from `http-types` that are part of `wiremock`'s public API (by [@LukeMathWalker]).

# `0.5.4`

- Fixes:
    - Handle URI authority properly in the mock server (by [@Tuetuopay])

# `0.5.3`

- Miscellaneous:
    - Remove `textwrap` from the dependency tree (by [@apiraino])

# `0.5.2`

- Features:
    - Support multi-valued headers (by [@beltram])
- Miscellaneous:
    - Improve README (by [@apiraino])

# `0.5.1`

- Features:
    - Capture the port in `hyper`'s `Request` into `wiremock::Request::url`  (by [@beltram])

# `0.5.0`

- Breaking changes:
    - Removed `MockServer::start_on`.  
      Use `MockServer::builder` and `MockServerBuilder::listener` to configure your `MockServer` to start on a pre-determined port (by [@LukeMathWalker]).
    - `MockServer::verify` is now asynchronous (by [@LukeMathWalker]).
- Features:
    - Added request recording to `MockServer`, enabled by default.  
      The recorded requests are used to display more meaningful error messages when assertions are not satisfied and can be retrieved using `MockServer::received_requests` ([by @LukeMathWalker]).
    - Added `MockServerBuilder` to customise the configuration of a `MockServer`.  
      It can be used to bind a `MockServer` to an existing `TcpListener` as well as disabling request recording (by [@LukeMathWalker]).
    - Added `matchers::body_json_schema` to verify the structure of the body attached to an incoming request (by [@Michael-J-Ward]).

[@Michael-J-Ward]: https://github.com/Michael-J-Ward
[@LukeMathWalker]: https://github.com/LukeMathWalker
[@beltram]: https://github.com/beltram
[@apiraino]: https://github.com/apiraino
[@Tuetuopay]: https://github.com/Tuetuopay
[@RoGryza]: https://github.com/RoGryza
