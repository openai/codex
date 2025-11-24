# Dunce (de-UNC)

In Windows the regular paths (`C:\foo`) are supported by all programs,
but have lots of bizarre restrictions for backwards compatibility with MS-DOS.
There are also Windows NT UNC paths (`\\?\C:\foo`), which are more robust and with fewer gotchas,
but are rarely supported by Windows programs — even Microsoft's own!

This crate converts Windows UNC paths to the MS-DOS-compatible format whenever possible,
but leaves UNC paths as-is when they can't be unambiguously expressed in a simpler way.
This allows legacy programs to access all paths they can possibly access,
and doesn't break any paths for UNC-aware programs.

For example, `\\?\C:\Windows` will be converted to `C:\Windows`, but `\\?\C:\COM` will be
left as-is, because it contains a reserved filename.

In Rust the worst UNC offender is the `fs::canonicalize()` function. This crate provides
a drop-in replacement for it that returns paths you'd expect.

On non-Windows platforms these functions leave paths unmodified, so it's safe to use them
unconditionally for all platforms.

This crate's handling of UNC paths is safer than just unconditionally stripping the `\\` prefix,
because naively stripped UNC paths with hostnames change to relative directory paths. There are
other normalization rules, special characters, and length limits that could change meaning
of the path.

Parsing is based on <https://msdn.microsoft.com/en-us/library/windows/desktop/aa365247(v=vs.85).aspx>.
