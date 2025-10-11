# Release attestations

Every Codex CLI release ships with Sigstore-backed [SLSA v1 build-provenance attestations](https://docs.github.com/actions/security-guides/using-artifact-attestations-to-establish-provenance-for-builds). Our release workflow (`.github/workflows/rust-release.yml`) signs each artifact with [`actions/attest-build-provenance`](https://github.com/actions/attest-build-provenance) and publishes matching `codex-<target>.sigstore.json` bundles alongside the binaries.

Download the artifact you plan to run plus its `.sigstore.json` bundle, then verify the provenance with either tool below.

## Verify with gh CLI (recommended)

To verify with GitHub's `gh` CLI, run this command, replacing `{FILE}` with the name of the archive you downloaded (e.g., `codex-x86_64-unknown-linux-musl.zst`):

```bash
gh attestation verify --repo openai/codex {FILE}
```

For advanced or offline workflows, refer to the [GitHub CLI attestation docs](https://cli.github.com/manual/gh_attestation_verify)


## Verify with cosign

To verify with [`cosign`](https://github.com/sigstore/cosign), download the `.sigstore.json` corresponding to the release target, replacing `{FILE}` to match name of the archive you downloaded (e.g., `codex-x86_64-unknown-linux-musl.zst`):

```bash
cosign verify-blob-attestation codex-{FILE}.zst \
  --bundle codex-{FILE}.sigstore.json \
  --certificate-identity-regexp https://github.com/openai/codex/.github/workflows/rust-release.yml \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

For advanced or offline workflows, the [`cosign` user guide](https://docs.sigstore.dev/cosign/overview/).
