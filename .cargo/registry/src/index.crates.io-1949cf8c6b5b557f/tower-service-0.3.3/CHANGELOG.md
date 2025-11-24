# 0.3.2

- **all**: new functions const when possible ([#760])
- **documentation**: Clarify subtlety around cloning and readiness in the `Service` docs. ([#622])
- **documentation**: service: Call inner.poll_ready() in docs when cloning inner ([#679])

[#760]: https://github.com/tower-rs/tower/pull/760
[#622]: https://github.com/tower-rs/tower/pull/662
[#679]: https://github.com/tower-rs/tower/pull/679

# 0.3.1 (November 29, 2019)

- Improve example in `Service` docs. ([#510])

[#510]: https://github.com/tower-rs/tower/pull/510

# 0.3.0 (November 29, 2019)

- Update to `futures 0.3`.
- Update documentation for `std::future::Future`.

# 0.3.0-alpha.2 (September 30, 2019)

- Documentation fixes.

# 0.3.0-alpha.1 (Aug 20, 2019)

* Switch to `std::future::Future`

# 0.2.0 (Dec 12, 2018)

* Change `Service`'s `Request` associated type to be a generic instead.
  * Before:

    ```rust
    impl Service for Client {
        type Request = HttpRequest;
        type Response = HttpResponse;
        // ...
    }
    ```
  * After:

    ```rust
    impl Service<HttpRequest> for Client {
        type Response = HttpResponse;
        // ...
    }
    ```
* Remove `NewService`, use `tower_util::MakeService` instead.
* Remove `Service::ready` and `Ready`, use `tower_util::ServiceExt` instead.

# 0.1.0 (Aug 9, 2018)

* Initial release
