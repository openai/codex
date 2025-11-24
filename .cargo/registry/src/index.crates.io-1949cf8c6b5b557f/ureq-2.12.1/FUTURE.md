- Make `AgentBuilder::timeout` be a truly global timeout, including for `connect`.
- Replace `impl From<http::request::Builder> for Request` with `TryFrom` because the conversion is fallible
  (implement in terms of `From<http::request::Parts>`: `builder.body(())?.into_parts().0.into()`);
- Change `Request::send_json` to take a reference to `data` instead of ownership. See [#737](https://github.com/algesten/ureq/issues/737)
