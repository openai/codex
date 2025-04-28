# codex-core

This crate implements the business logic for Codex. It is designed to be used by the various Codex UIs written in Rust.

Though for non-Rust UIs, we are also working to define a _protocol_ for talking to Codex. See:

- [Specification](../docs/protocol_v1.md)
- [Rust types](./src/protocol.rs)

You can use the `proto` subcommand using the executable in the [`cli` crate](../cli) to speak the protocol using newline-delimited-JSON over stdin/stdout.
