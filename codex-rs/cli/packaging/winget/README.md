WinGet manifests for the Codex CLI

Local testing

- Validate: `winget validate .\manifests\o\OpenAI\Codex\0.57.0`
- Install from local manifests: `winget install --manifest .\manifests\o\OpenAI\Codex\0.57.0`
- Verify: `codex --version` and `where codex`
- Uninstall: `winget uninstall OpenAI.Codex`

Submitting to winget-pkgs

- Ensure URLs and SHA256 match the public GitHub Release for this version.
- Submit with `wingetcreate submit <path>` or copy this tree into a fork of `microsoft/winget-pkgs` under the same path.
Winget manifests

- Templates live under `.github/winget_templates/` and use placeholders:
  - `{{VERSION}}`, `{{X64_SHA256}}`, `{{ARM64_SHA256}}`
- The CI calls a composite action (`.github/actions/winget-submit`) from the release job:
  - Fills the templates using the release version and precomputed SHA256s,
  - Validates the manifests with `winget validate` (submission is separate).

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
