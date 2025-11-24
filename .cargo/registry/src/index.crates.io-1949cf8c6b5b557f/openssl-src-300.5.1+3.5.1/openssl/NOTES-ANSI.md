Notes on ANSI C
===============

When building for pure ANSI C (C89/C90), you must configure with at least
the following configuration settings:

-   `no-asm`

    There are cases of `asm()` calls in our C source, which isn't supported
    in pure ANSI C.

-   `no-secure-memory`

    The secure memory calls aren't supported with ANSI C.

-   `-D_XOPEN_SOURCE=1`

    This macro enables the use of the following types, functions and global
    variables:

    -   `timezone`

-   `-D_POSIX_C_SOURCE=200809L`

    This macro enables the use of the following types, functions and global
    variables:

    -   `ssize_t`
    -   `strdup()`

It's arguable that with gcc and clang, all of these issues are removed when
defining the macro `_DEFAULT_SOURCE`.  However, that effectively sets the C
language level to C99, which isn't ANSI C.
