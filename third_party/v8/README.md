# `rusty_v8` Consumer Artifacts

Codex publishes its own `rusty_v8` archive/binding pairs for Cargo release,
package, and CI builds. The exact producer input is
`third_party/v8/artifacts.toml`. It pins:

- the crates.io `v8` wrapper version;
- the V8 version native to that wrapper;
- the selected V8 engine version and denoland/v8 commit;
- an ordered, versioned Codex patch recipe;
- the complete release artifact identity.

The current identity is
`rusty-v8-v149.2.0-v8-14.9.207.35-recipe-1`. The selected engine revision is
the one used by Chromium `149.0.7827.201`.

`.github/scripts/materialize_rusty_v8.py` creates every published artifact
source tree. It checks out the wrapper tag, replaces its V8 submodule with the
manifest commit, applies every recipe patch using `git apply --check --index`,
updates Chromium dependencies, and verifies their exact revisions. Patch
application does not permit fuzz.

Local Cargo builds still use upstream `rusty_v8` prebuilts by default.
`.github/actions/setup-rusty-v8` and the package scripts use the manifest's
artifact identity to download Codex assets and set
`RUSTY_V8_ARCHIVE`/`RUSTY_V8_SRC_BINDING_PATH`.

## Updating the artifact manifest

Independent engine updates are initially limited to the wrapper's V8 patch
line: major, minor, and build must match `wrapper_v8_version`. For example,
`14.9.207.2` may move to `14.9.207.35`, but not `14.9.208.1`. Bump the
wrapper when generated bindings or runtime tests do not pass.

For an engine patch update:

1. update `v8_version` and `v8_source_commit`;
2. create a new recipe directory and increment `patch_recipe` whenever the
   ordered patch contents change;
3. update `artifact_identity` to
   `rusty-v8-v<wrapper>-v8-<engine>-recipe-<recipe>`;
4. keep the V8 archive pin in `MODULE.bazel` aligned;
5. run the helper, materializer, V8, and code-mode tests;
6. publish a candidate branch and require `v8-canary` to pass.

For a wrapper update, also update `codex-rs/Cargo.lock`,
`wrapper_version`, and `wrapper_v8_version`. A wrapper bump is required if
the patch-level engine update does not compile, link, or pass runtime tests.

Validate the checked-in pins with:

```bash
python3 .github/scripts/rusty_v8_bazel.py check-artifact-manifest
python3 .github/scripts/rusty_v8_bazel.py check-module-bazel
```

After the candidate is green, create the tag printed by:

```bash
python3 .github/scripts/rusty_v8_bazel.py artifact-identity
```

Creating or pushing a topic branch does not create this tag.

## Published artifacts

`.github/workflows/rusty-v8-release.yml` builds both release and
pointer-compression/sandbox profiles from the materialized tree for:

- x86_64 and aarch64 Darwin;
- x86_64 and aarch64 GNU Linux;
- x86_64 and aarch64 musl Linux;
- x86_64 and aarch64 Windows MSVC.

Release assets use:

- `librusty_v8_release_<target>.a.gz` on Darwin and Linux;
- `rusty_v8_release_<target>.lib.gz` on Windows MSVC;
- `src_binding_release_<target>.rs`;
- `rusty_v8_release_<target>.sha256`.

Sandbox assets replace `release` with `ptrcomp_sandbox_release`. Each
checksum file covers exactly its archive and generated binding.

Every architecture smoke-links the staged pair against the unmodified crates.io
wrapper used by the Codex workspace. Native jobs run both `codex-v8-poc` and
`codex-code-mode-host` tests. Windows ARM64 is cross-built on Windows x64 and
then tested on a native Windows ARM64 runner.

We use explicit archive and binding URLs instead of `RUSTY_V8_MIRROR` because
the upstream wrapper assumes a `v<crate_version>` tag layout. Codex identities
also include the engine and recipe versions.

## Bazel consumers

Bazel consumer builds remain a separate validation and consumption path.
Darwin, GNU Linux, musl Linux, and Windows GNU use the source pin in
`MODULE.bazel`; Windows MSVC Bazel selectors still use upstream prebuilts
until the Bazel graph has an MSVC C++ toolchain.

The consumer-facing selectors are:

- `//third_party/v8:rusty_v8_archive_for_target`
- `//third_party/v8:rusty_v8_binding_for_target`

The Bazel graph pins the libc++, libc++abi, and llvm-libc revisions used by the
wrapper and builds with `--config=rusty-v8-upstream-libcxx`. `v8-canary`
keeps this Bazel matrix alongside the materialized-source artifact matrix so
the two consumers cannot drift silently.
