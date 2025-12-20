## Contributing

Codexel is a community-maintained fork of upstream OpenAI Codex CLI. Contributions are welcome, especially:

- Bug fixes and security hardening
- UX improvements (TUI, plan mode, approvals/sandbox ergonomics)
- Documentation fixes and examples
- Tests and CI reliability

For larger features or behavior changes, open an issue first so we can agree on scope and direction. If the issue reproduces in upstream Codex CLI too, it may be a better fit as an upstream PR (or at least worth reporting upstream as well).

### Development workflow

- Create a _topic branch_ from `main` - e.g. `feat/interactive-prompt`.
- Keep your changes focused. Multiple unrelated fixes should be opened as separate PRs.
- Ensure your change is free of lint warnings and test failures. Prefer the repo `just` helpers where possible (see `docs/install.md`).

### Changelog (Codexel fork)

- The changelog tracks Codexel-only changes (commits not in `upstream/main`).
- Refresh generated Details blocks with `scripts/gen-changelog.ps1` (Windows) or
  `bash scripts/gen-changelog.sh` (macOS/Linux).
- Use `--check` in CI to ensure the changelog is up to date.
- When cutting a release, pin the release commit and upstream baseline in
  `CHANGELOG.md`, then update the generated range for that release section.
- Rollback is just reverting `CHANGELOG.md`, `cliff.toml`, and the generator
  scripts if the changelog workflow needs to be removed.

### Writing high-impact code changes

1. **Start with an issue.** Open a new one or comment on an existing discussion so we can agree on the solution before code is written.
2. **Add or update tests.** Every new feature or bug-fix should come with test coverage that fails before your change and passes afterwards. 100% coverage is not required, but aim for meaningful assertions.
3. **Document behavior.** If your change affects user-facing behavior, update the README, inline help (`codexel --help`), or relevant docs under `docs/`.
4. **Keep commits atomic.** Each commit should compile and the tests should pass. This makes reviews and potential rollbacks easier.

### Opening a pull request

- Fill in the PR template (or include similar information) - **What? Why? How?**
- Include a link to a bug report or enhancement request in the issue tracker
- Run the relevant checks locally. Use the root `just` helpers so you stay consistent with the rest of the workspace: `just fmt`, `just fix -p <crate>` for the crate you touched, and the relevant tests (e.g., `cargo test -p codex-tui`).
- Make sure your branch is up-to-date with `main` and that you have resolved merge conflicts.
- Mark the PR as **Ready for review** only when you believe it is in a merge-able state.

### Review process

1. One maintainer will be assigned as a primary reviewer.
2. If your PR adds a new feature that was not previously discussed and approved, we may choose to close your PR (see [Contributing](#contributing)).
3. We may ask for changes - please do not take this personally. We value the work, but we also value consistency and long-term maintainability.
4. When there is consensus that the PR meets the bar, a maintainer will squash-and-merge.

### Community values

- **Be kind and inclusive.** Treat others with respect; we follow the [Contributor Covenant](https://www.contributor-covenant.org/).
- **Assume good intent.** Written communication is hard - err on the side of generosity.
- **Teach & learn.** If you spot something confusing, open an issue or PR with improvements.

### Getting help

If you run into problems setting up the project or want feedback on an idea, please open a Discussion or jump into the relevant issue.

### Security & responsible AI

If you discover a security issue, prefer opening a GitHub Security Advisory for this repository. If that's not possible, open an issue with minimal details and we’ll follow up.
