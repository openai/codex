# Contributing to dbus

The guidelines in this file are the ideals; it's better to send a
not-fully-following-guidelines patch than no patch at all, though.  We
can always polish it up.

## Source code and issue tracking

Source code and issue tracking for the D-Bus specification and its
reference implementation 'dbus' are provided by freedesktop.org Gitlab:
<https://gitlab.freedesktop.org/dbus/dbus>.

## Reporting security vulnerabilities

If you find a security vulnerability that is not known to the public,
please report it privately to dbus-security@lists.freedesktop.org
or by reporting a Gitlab issue at
https://gitlab.freedesktop.org/dbus/dbus/issues/new and marking it
as "confidential".
For appropriate patches, please create a "confidential" merge request,
see [Sending a merge request](#sending-a-merge-request] for details.

## Mailing list

The D-Bus mailing list is dbus@lists.freedesktop.org; discussion of
protocol enhancements, new implementations, etc. should go there.

## Code of Conduct

As a freedesktop.org project, dbus follows the Contributor Covenant,
found at: https://www.freedesktop.org/wiki/CodeOfConduct

Please conduct yourself in a respectful and civilised manner when
interacting with community members on mailing lists, IRC, or bug
trackers. The community represents the project as a whole, and abusive
or bullying behaviour is not tolerated by the project.

## Development

D-Bus uses Git as its version control system. The main repository is
hosted on freedesktop.org Gitlab. To clone D-Bus, execute one of the
following commands:

    git clone https://gitlab.freedesktop.org/dbus/dbus.git
    git clone git@gitlab.freedesktop.org:dbus/dbus.git

The second form is the one that allows pushing, but it also requires
an SSH account on the server. The first form allows anonymous
checkouts.

### Branches

D-Bus development happens in multiple branches in parallel. The main
branches are the current stable branch, with an even minor number (like
1.0, 1.2 and 1.4), and the next development branch, with the next odd
number. At the time of writing, the stable branch is dbus 1.12.x and
the development branch is dbus 1.13.x, leading to a new 1.14.x stable
branch in future.

Stable branches are named after the version number itself (`dbus-1.2`,
`dbus-1.4`), whereas the development branch is simply known as
`master`.

New features, enhancements, minor bug fixes, and bug fixes that are
unusually intrusive should always be based on the `master` branch.

Fixes for significant bugs should be developed on the `master` branch
and cherry-picked to the most recent stable branch.

Depending on the release cycles of various Linux distributions, some
older stable branches might continue to receive fixes for security
vulnerabilities (and sometimes major non-security bugs) for a time.
These are announced on the D-Bus mailing list.

Old development branches are not supported at all, and will not receive
any bug fixes - not even for security vulnerabilities. Please do not
use a development branch like 1.13.x in your OS distribution, unless
you can guarantee that you will upgrade to the next stable branch such
as 1.14.x when it becomes available.

### Commits

If you are making changes that you wish to be incorporated upstream,
please do as small commits to your local git tree that are individually
correct, so there is a good history of your changes.

The first line of the commit message should be a single sentence that
describes the change, optionally with a prefix that identifies the
area of the code that is affected.

The body of the commit message should describe what the patch changes
and why, and also note any particular side effects. This shouldn't be
empty on most of the cases. It shouldn't take a lot of effort to write a
commit message for an obvious change, so an empty commit message body is
only acceptable if the questions "What?" and "Why?" are already answered
on the one-line summary.

The lines of the commit message should have at most 76 characters,
to cope with the way git log presents them.

See [notes on commit messages](https://who-t.blogspot.com/2009/12/on-commit-messages.html),
[A Note About Git Commit Messages](https://tbaggery.com/2008/04/19/a-note-about-git-commit-messages.html)
or [How to Write a Git Commit Message](https://chris.beams.io/posts/git-commit/)
for recommended reading on writing high-quality commit messages.

Your patches should also include a Signed-off-by line with your name and
email address, indicating that your contribution follows the [Developer's
Certificate of Origin](https://developercertificate.org/). If you're
not the patch's original author, you should also gather S-o-b's by
them (and/or whomever gave the patch to you.) The significance of this
is that it certifies that you created the patch, that it was created
under an appropriate open source license, or provided to you under those
terms. This lets us indicate a chain of responsibility for the copyright
status of the code.

We won't reject patches that lack S-o-b, but it is strongly recommended.

### Sending a merge request

When you consider your changes to be ready for merging to mainline:

* create a personal fork of <https://gitlab.freedesktop.org/dbus/dbus>
  on freedesktop.org Gitlab
* push your changes to your personal fork as a branch
* create a merge request at
  <https://gitlab.freedesktop.org/dbus/dbus/merge_requests>
* Merge requests for "confidential" issues must also be created as
  "confidential", see <https://docs.gitlab.com/ee/user/project/merge_requests/confidential.html>
  for details.

### Security guidelines

Most of D-Bus is security sensitive.  Guidelines related to that:

 - avoid `memcpy()`, `sprintf()`, `strlen()`, `snprintf()`, `strlcat()`,
   `strstr()`, `strtok()`, or any of this stuff. Use `DBusString`.
   If `DBusString` doesn't have the feature you need, add it
   to `DBusString`.

   There are some exceptions, for example
   if your strings are just used to index a hash table
   and you don't do any parsing/modification of them, perhaps
   `DBusString` is wasteful and wouldn't help much. But definitely
   if you're doing any parsing, reallocation, etc. use `DBusString`.

 - do not include system headers outside of `dbus-memory.c`,
   `dbus-sysdeps.c`, and other places where they are already
   included. This gives us one place to audit all external
   dependencies on features in libc, etc.

 - do not use libc features that are "complicated"
   and may contain security holes. For example, you probably shouldn't
   try to use `regcomp()` to compile an untrusted regular expression.
   Regular expressions are just too complicated, and there are many
   different libc implementations out there.

 - we need to design the message bus daemon (and any similar features)
   to use limited privileges, run in a chroot jail, and so on.

http://vsftpd.beasts.org/ has other good security suggestions.

### Coding Style

 - Please follow the coding style and indentation of nearby code.

 - C code uses GNU coding conventions (approximately "gnu" style in
   Emacs), with GLib-like extensions (e.g. lining up function arguments).

 - Write docs for all non-static functions and structs and so on. try
   `doxygen Doxyfile` prior to commit and try not to cause any new
   warnings.

 - All external interfaces (network protocols, file formats, etc.)
   should have documented specifications sufficient to allow an
   alternative implementation to be written. Our implementation should
   be strict about specification compliance (should not for example
   heuristically parse a file and accept not-well-formed
   data). Avoiding heuristics is also important for security reasons;
   if it looks funny, ignore it (or exit, or disconnect).

### Licensing

Please match the existing licensing (a dual-license: AFL-2.1 or GPL-2+,
recipient's choice). Entirely new modules can be placed under a more
permissive license: to avoid license proliferation, our preferred
permissive license is the variant of the MIT/X11 license used by the
Expat XML library (for example see the top of tools/ci-build.sh).

### Build systems

The primary build system for dbus uses the GNU Autotools suite (Autoconf,
Automake and Libtool). This build system is strongly recommended for
Unix OS integrators. It can also be used to compile dbus for Windows
using the mingw-w64 compiler suite, either by cross-compiling on a Unix
system or by using an environment like MSYS2 on Windows.

There is also a CMake build system. This is primarily there to make it
easier to build dbus on Windows, using either a MSYS2/mingw environment
or the MSVC compiler from Microsoft Visual Studio. It can also be used
on a GNU/Linux system, but this is not recommended for OS integrators.

Changes contributed to dbus must not break the build for either of these
build systems. It is OK for the CMake build system to support fewer
options, support fewer operating systems, have less test coverage or
build fewer non-essential programs, but it must continue to work on at
least GNU/Linux and Windows.

### Environment variables

These are some of the environment variables that are used by the D-Bus
client library.

* `DBUS_VERBOSE=1`

  Turns on printing verbose messages. This only works if D-Bus has been
  compiled with `--enable-verbose-mode`.

* `DBUS_MALLOC_FAIL_NTH=n`

  Can be set to a number, causing every *n*th call to `dbus_alloc` or
  `dbus_realloc` to fail. This only works if D-Bus has been compiled with
  `--enable-embedded-tests`.

* `DBUS_MALLOC_FAIL_GREATER_THAN=n`

  Can be set to a number, causing every call to `dbus_alloc` or
  `dbus_realloc` to fail if the number of bytes to be allocated is greater
  than the specified number. This only works if D-Bus has been compiled with
  `--enable-embedded-tests`.

* `DBUS_TEST_MALLOC_FAILURES=n`

  Many of the D-Bus tests will run over and over, once for each `malloc`
  involved in the test. Each run will fail a different `malloc`, plus some
  number of `malloc`s following that malloc (because a fair number of bugs
  only happen if two or more `malloc`s fail in a row, e.g. error recovery
  that itself involves `malloc`).  This environment variable sets the
  number of consecutive `malloc`s to fail.

  Here's why you care: If set to 0, then the `malloc` checking is skipped,
  which makes the test suite a lot faster. Just run with this
  environment variable unset before you commit.

### Tests

Please try to write test coverage for all new functionality.
We have two broad categories of tests.

The *modular tests* are enabled by configuring with
`--enable-modular-tests`. These mostly use GLib's GTest framework,
and are standalone programs that do not affect the contents of the
production dbus library and programs. Most of them can be installed
alongside the library and programs by configuring with
`--enable-installed-tests`.

The *embedded tests* are enabled by configuring with
`--enable-embedded-tests`. Unlike the modular tests, enabling the
embedded tests adds special code to libdbus and dbus-daemon, some of
which may harm performance or security. A production version of dbus
that will be included in an operating system should never have the
embedded tests enabled.

If possible, new test coverage should be provided via modular tests,
preferably using GLib's GTest framework. `test/dbus-daemon.c` is a good
example.

## Information for maintainers

This section is not directly relevant to infrequent contributors.

### Releasing

See maint/release-checklist.md.

### Code reviews

The commit rules are approximately these:

 - Fixes that don't affect API or protocol can be committed
   if any one qualified reviewer other than patch author
   reviews and approves

 - For fixes that do affect API or protocol, two people
   in the reviewer group have to review and approve the commit.

 - If there's a live unresolved controversy about a change,
   don't commit it while the argument is still raging.

 - At their discretion, members of the reviewer group may also commit
   branches/patches under these conditions:

   - the branch does not add or change API, ABI or wire-protocol

   - the branch solves a known problem and is covered by the regression tests

   - there are no objections from the rest of the review group within
     a week of the merge request being opened

   - the committer gets a positive review on the merge request from someone
     they consider qualified to review the change (e.g. a colleague with D-Bus
     experience; not necessarily a member of the reviewer group)

 - Regardless of reviews, to commit a patch:

    - `make check` must pass
    - the test suite must be extended to cover the new code
      as much as reasonably feasible (see Tests above)
    - the patch has to follow the portability, security, and
      style guidelines
    - the patch should as much as reasonable do one thing,
      not many unrelated changes

   No reviewer should approve a patch without these attributes, and
   failure on these points is grounds for reverting the patch.

The reviewer group that can approve patches consists of the members
of <https://gitlab.freedesktop.org/dbus/dbus/project_members> with
"Maintainer" or "Owner" status.
