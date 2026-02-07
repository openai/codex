# codex-process-hardening

This crate provides `pre_main_hardening()`, which is designed to be called pre-`main()` (using `#[ctor::ctor]`) to perform various process hardening steps, such as

- disabling core dumps
- disabling ptrace attach on Linux and macOS
- removing dangerous environment variables such as `LD_PRELOAD` and `DYLD_*`

To opt out in dev or test environments, set the following in your
`~/.codex/config.toml`:

```toml
[security]
process_hardening_disable = true
```
