The `serde_core` crate contains Serde's trait definitions with **no support for
#\[derive()\]**.

In crates that derive an implementation of `Serialize` or `Deserialize`, you
must depend on the [`serde`] crate, not `serde_core`.

[`serde`]: https://crates.io/crates/serde

In crates that handwrite implementations of Serde traits, or only use them as
trait bounds, depending on `serde_core` is permitted. But `serde` re-exports all
of these traits and can be used for this use case too. If in doubt, disregard
`serde_core` and always use `serde`.

Crates that depend on `serde_core` instead of `serde` are able to compile in
parallel with `serde_derive` even when `serde`'s "derive" feature is turned on,
as shown in the following build timings.

<br>

| When `serde_json` depends on `serde` |
|---|
| <img src="https://github.com/user-attachments/assets/78dc179c-6ab1-4059-928c-1474b0d9d0bb"> |

<br>

| When `serde_json` depends on `serde_core` |
|---|
| <img src="https://github.com/user-attachments/assets/6b6cff5e-3e45-4ac7-9db1-d99ee8b9f5f7"> |
