# Contributing

Thanks for considering helping this project.  There are many
ways you can help: using the library and reporting bugs,
reporting usability issues, making additions and improvements
to the library, documentation and finding security bugs.

## Reporting bugs

Please file a github issue.  Include as much information as
possible.  Suspected protocol bugs are easier debugged with
a pcap or reproduction steps.

Feel free to file github issues to get help, or ask a question.

## Code changes

Some ideas and guidelines for contributions:

- For large features, file an issue prior to starting work.
  This means everyone can see what is in progress prior to a PR.
- Feel free to submit a PR even if the work is not totally finished,
  for feedback or to hand-over.
- Prefer not to reference github issue or PR numbers in commits.
- Try to keep code formatting commits separate from functional commits.
- I run `cargo outdated` prior to major releases; but PRs to update specific
  dependencies are welcome.

## Security bugs

Please report security bugs by filing a github issue, or by
email to jbp@jbp.io if you want to disclose privately.  I'll then:

- Prepare a fix and regression tests.
- Backport the fix and make a patch release for most recent release.
- Submit an advisory to [rustsec/advisory-db](https://github.com/RustSec/advisory-db).
- Refer to the advisory on the main README.md and release notes.

## Testing

- Features involving additions to the public API should have (at least)
  API-level tests (see [`tests/api.rs`](tests/api.rs)).
- Protocol additions should have some coverage -- consider enabling
  corresponding tests in the bogo suite, or writing some adhoc tests.

PRs which cause test failures or a significant coverage decrease
are unlikely to be accepted.

## Licensing

Contributions are made under [rustls-native-certs's licenses](LICENSE).
