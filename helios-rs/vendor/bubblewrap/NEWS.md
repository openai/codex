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
