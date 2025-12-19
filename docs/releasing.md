## Releasing Codexel (npm)

Codexel is published as `@ixe1/codexel` with prebuilt native binaries bundled in
`codex-cli/vendor/`. Publishing is handled by GitHub Actions.

### Release checklist

- Update `codex-cli/package.json` to the release version (no `-dev` suffix).
- Merge the release commit to `main`.
- Create and push a tag:
  - Stable: `codexel-vX.Y.Z`
  - Pre-release: `codexel-vX.Y.Z-alpha.N` or `codexel-vX.Y.Z-beta.N`

### What the workflow does

The `npm-publish-codexel` workflow:

- Builds the `codexel` binary for all supported targets.
- Assembles `codex-cli/vendor/<target>/codex/codexel(.exe)`.
- Packs an npm tarball and runs a smoke test (`codexel --help`).
- Creates a GitHub Release containing the per-target binaries and npm tarball.
- Publishes `@ixe1/codexel` using npm Trusted Publishing (OIDC).

### One-time setup

Before the first publish, configure npm Trusted Publishing for `@ixe1/codexel`
to trust this repository and the `npm-publish-codexel` workflow in the npm UI.
