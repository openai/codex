<a name="0.2.0"></a>
## 0.2.0 (2024-07-01)


#### Changes

*   Update to `regex-automata` v0.4 (#5) ([92217ada](https://github.com/hawkw/matchers/commit/92217ada16bb2c873e2ac6e49f4e74ec93a23b22))


<a name="0.1.0"></a>
## 0.1.0 (2021-02-25)


#### Features

* **Pattern:**  add `Pattern::new_anchored` (#2) ([81f4c1c0](https://github.com/hawkw/matchers/commit/81f4c1c0ece4908d2e62607bb8a5690646404dec))



<a name="0.0.1"></a>
## 0.0.1 (2019-08-02)


#### Features

*   add `FromStr` impl for `Pattern` ([86a33462](https://github.com/hawkw/matchers/commit/86a33462d80c48cf3d534da0759d2b2ea5ddce5f))
*   add `Clone` and `Debug` impls ([9092305d](https://github.com/hawkw/matchers/commit/9092305dde81b69f1051b138b94f653124da662f))
*   support matching on `io::Read` ([121efb9b](https://github.com/hawkw/matchers/commit/121efb9b093b817d6c67d11a5b5508079091ab3d))

#### Bug Fixes

*   only short-circuit when known not to match ([71544493](https://github.com/hawkw/matchers/commit/71544493613049009f33f4885a1287eee0f21aa4))
