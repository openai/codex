Test data was taken from the Go distribution, which was in turn taken from the
testregex test suite:

  http://web.archive.org/web/20150925124103/http://www2.research.att.com/~astopen/testregex/testregex.html

Unfortunately, the original web site now appears dead, but the test data lives
on.

The LICENSE in this directory corresponds to the LICENSE that the data was
originally released under.

The tests themselves were modified for RE2/Go (and marked as such). A
couple were modified further by me (Andrew Gallant) and marked with 'Rust'.

After some number of years, these tests were transformed into a TOML format
using the 'regex-cli generate fowler' command. To re-generate the
TOML files, run the following from the root of this repository:

  regex-cli generate fowler tests/data/fowler tests/data/fowler/dat/*.dat

This assumes that you have 'regex-cli' installed. See 'regex-cli/README.md'
from the root of the repository for more information.

This brings the Fowler tests into a more "sensible" structured format in which
other tests can be written such that they aren't write-only.
