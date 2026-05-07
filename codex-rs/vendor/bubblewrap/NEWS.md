bubblewrap 0.11.2
=================

Released: 2026-04-23

Bug fixes:

  * In setuid mode, don't run the low-privileged parts parts of the setup
    as dumpable, as that allows it to be ptraced which can lead to problems.
    This is CVE-2026-41163, and was reported by François Diakhate.

Enhancements:

  * New build option `-Dsupport_setuid`, which if set to false (which
    is the default) disables the support for setuid. Binaries built
    with this will refuse to run if made setuid. We recommend building
    normal bubblewrap binaries like this, which allows you to safely
    ignore any security issues that only affect setuid mode.

bubblewrap 0.11.1
=================

Released: 2026-03-21

Bug fixes:

  * Reset disposition of `SIGCHLD`, restoring normal subprocess management
    if bwrap was run from a process that was ignoring that signal,
    such as Erlang or volumeicon (#705, Joel Pelaez Jorge)

  * Don't ignore `--userns 0`, `--userns2 0` or `--pidns 0` if used
    (#731, Daniel Cazares).
    Note that using a fd number ≥ 3 for these purposes is still
    preferred, to avoid confusion with the stdin, stdout, stderr
    that will be inherited by the command inside the container.

  * Fix grammar in an error message (#694, J. Neuschäfer)

  * Fix a broken link in the documentation (#729, Aaron Brooks)

Internal changes:

  * Enable user namespaces in Github Actions configuration, fixing a CI
    regression with newer Ubuntu (#728, Joel Pelaez Jorge)

  * Clarify comments (#737, Simon McVittie)

bubblewrap 0.11.0
=================

Released: 2024-10-30

Dependencies:

  * Remove the Autotools build system. Meson ≥ 0.49.0 is now required
    at build-time. (#625, Hugo Osvaldo Barrera)

  * For users of bash-completion, bash-completion ≥ 2.10 is recommended.
    With older bash-completion, bubblewrap might install completions
    outside its `${prefix}` unless overridden with `-Dbash_completion_dir=…`.

Enhancements:

  * New `--overlay`, `--tmp-overlay`, `--ro-overlay` and `--overlay-src`
    options allow creation of overlay mounts.
    This feature is not available when bubblewrap is installed setuid.
    (#412, #663; Ryan Hendrickson, William Manley, Simon McVittie)

  * New `--level-prefix` option produces output that can be parsed by
    tools like `logger --prio-prefix` and `systemd-cat --level-prefix=1`
    (#646, Simon McVittie)

Bug fixes:

  * Handle `EINTR` when doing I/O on files or sockets (#657, Simon McVittie)

  * Don't make assumptions about alignment of socket control message data
    (#637, Simon McVittie)

  * Silence some Meson deprecation warnings (#647, @Sertonix)

  * Update URLs in documentation to https (#566, @TotalCaesar659)

  * Improve tests' compatibility with busybox (#627, @Sertonix)

  * Improve compatibility with Meson < 1.3.0 (#664, Simon McVittie)

Internal changes:

  * Consistently use `<stdbool.h>` for booleans (#660, Simon McVittie)

  * Avoid `-Wshadow` compiler warnings (#661, Simon McVittie)

  * Update Github Actions configuration (#658, Simon McVittie)

----

See also <https://github.com/containers/bubblewrap/releases>
