# codex-process-hardening

This crate provides `pre_main_hardening()`, which is designed to be called pre-`main()` (using `#[ctor::ctor]`) to perform various process hardening steps, such as

- disabling core dumps
- disabling ptrace attach on Linux and macOS
- removing dangerous or noisy environment variables such as `LD_PRELOAD`,
  `DYLD_*`, and macOS malloc stack-logging controls
