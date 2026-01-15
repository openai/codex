## Contributing

This project is under active development and the code will likely change pretty significantly.

**At the moment, we are generally accepting external contributions only for bugs fixes.**

If you want to add a new feature or change the behavior of an existing one, please open an issue proposing the feature or upvote an existing enhancement request. We will generally prioritize new features based on community feedback. New features must compose well with existing and upcoming features and fit into our roadmap. They must also be implemented consistently across all Codex surfaces (CLI, IDE extension, web, etc.).

If you want to contribute a bug fix, please open a bug report first - or verify that there is an existing bug report that discusses the issue. All bug fix PRs should include a link to a bug report.

**New contributions that don't go through this process may be closed** if they aren't aligned with our current roadmap or conflict with other priorities/upcoming features.

### Development workflow

- Create a _topic branch_ from `main` - e.g. `feat/interactive-prompt`.
- Keep your changes focused. Multiple unrelated fixes should be opened as separate PRs.
- Ensure your change is free of lint warnings and test failures.

### Writing high-impact code changes

1. **Start with an issue.** Open a new one or comment on an existing discussion so we can agree on the solution before code is written.
2. **Add or update tests.** A bug fix should generally come with test coverage that fails before your change and passes afterwards. 100% coverage is not required, but aim for meaningful assertions.
3. **Document behavior.** If your change affects user-facing behavior, update the README, inline help (`codex --help`), or relevant example projects.
4. **Keep commits atomic.** Each commit should compile and the tests should pass. This makes reviews and potential rollbacks easier.

### Opening a pull request

- Fill in the PR template (or include similar information) - **What? Why? How?**
- Include a link to a bug report or enhancement request in the issue tracker
- Run **all** checks locally. Use the root `just` helpers so you stay consistent with the rest of the workspace: `just fmt`, `just fix -p <crate>` for the crate you touched, and the relevant tests (e.g., `cargo test -p codex-tui` or `just test` if you need a full sweep). CI failures that could have been caught locally slow down the process.
- Make sure your branch is up-to-date with `main` and that you have resolved merge conflicts.
- Mark the PR as **Ready for review** only when you believe it is in a merge-able state.

### Review process

1. One maintainer will be assigned as a primary reviewer.
2. If your PR adds a new feature that was not previously discussed and approved, we may close your PR (see [Contributing](#contributing)).
3. We may ask for changes. Please do not take this personally. We value the work, but we also value consistency and long-term maintainability.
4. When there is consensus that the PR meets the bar, a maintainer will squash-and-merge.

### Community values

- **Be kind and inclusive.** Treat others with respect; we follow the [Contributor Covenant](https://www.contributor-covenant.org/).
- **Assume good intent.** Written communication is hard - err on the side of generosity.
- **Teach & learn.** If you spot something confusing, open an issue or PR with improvements.

### Getting help

If you run into problems setting up the project, would like feedback on an idea, or just want to say _hi_ - please open a Discussion topic or jump into the relevant issue. We are happy to help.

Together we can make Codex CLI an incredible tool. **Happy hacking!** :rocket:

### Contributor license agreement (CLA)

All contributors **must** accept the CLA. The process is lightweight:

1. Open your pull request.
2. Paste the following comment (or reply `recheck` if you've signed before):

   ```text
   I have read the CLA Document and I hereby sign the CLA
   ```

3. The CLA-Assistant bot records your signature in the repo and marks the status check as passed.

No special Git commands, email attachments, or commit footers required.

### Security & responsible AI

Have you discovered a vulnerability or have concerns about model output? Please e-mail **security@openai.com** and we will respond promptly.
