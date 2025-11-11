WinGet manifests for the Codex CLI

Local testing

- Validate: `wingetcreate validate .\\cli\\packaging\\winget\\manifests\\o\\OpenAI\\Codex\\0.57.0`
- Install from local manifests: `winget install --manifest .\\cli\\packaging\\winget\\manifests\\o\\OpenAI\\Codex\\0.57.0`
- Verify: `codex --version` and `where codex`
- Uninstall: `winget uninstall OpenAI.Codex`

Submitting to winget-pkgs

- Ensure URLs and SHA256 match the public GitHub Release for this version.
- Submit with `wingetcreate submit <path>` or copy this tree into a fork of `microsoft/winget-pkgs` under the same path.
Winget manifests

- Templates live under `cli/packaging/winget/template/` and use placeholders:
  - `{{VERSION}}`, `{{X64_SHA256}}`, `{{ARM64_SHA256}}`
- The CI workflow `.github/workflows/winget-submit.yml`:
  - Derives the version from the release tag (strips `rust-v`),
  - Downloads the raw Windows EXEs from the release,
  - Computes SHA256s and fills the templates,
  - Validates and submits a PR to `microsoft/winget-pkgs` using `wingetcreate`.

Setup

- Ensure releases include raw Windows assets:
  - `codex-x86_64-pc-windows-msvc.exe`
  - `codex-aarch64-pc-windows-msvc.exe`
- Add a repo secret `WINGET_PUBLISH_PAT` with `repo` (or `public_repo`) scope for PR submission.

Local test

- Build a versioned manifest set:
  - Replace placeholders in the files under `template/` and stage under `manifests/o/OpenAI/Codex/<VERSION>/`.
- Validate:
  - `wingetcreate validate manifests/o/OpenAI/Codex/<VERSION>`
- Install locally:
  - `winget install --manifest manifests/o/OpenAI/Codex/<VERSION>`

