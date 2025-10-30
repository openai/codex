## @just-every/code v0.4.6
This release polishes the build pipeline and publishes the refreshed CLI package.

### Changes
- Build: keep release notes version in sync during `build-fast` to stop false release failures.
- Build: drop the release notes gate so `build-fast` runs cleanly in CI.
- CLI: publish the v0.4.6 package metadata for all platform bundles.

### Install
```
npm install -g @just-every/code@latest
code
```

Compare: https://github.com/just-every/code/compare/v0.4.5...v0.4.6
