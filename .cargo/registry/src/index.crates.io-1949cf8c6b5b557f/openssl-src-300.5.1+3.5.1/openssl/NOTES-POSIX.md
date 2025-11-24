Notes on POSIX
==============

There are few instances where OpenSSL requires a POSIX C library, at least
version 1-2008, or compatible enough functionality.

There are exceptions, though, for platforms that do not have a POSIX
library, or where there are quirks that need working around.  A notable
platform is Windows, where POSIX functionality may be available, but where
the function names are prefixed with an underscore, and where some POSIX
types are not present (such as `ssize_t`).

Platforms that do have a POSIX library may still not have them accessible
unless the following macros are defined:

    _POSIX_C_SOURCE=200809L
    _XOPEN_SOURCE=1

This is, for example, the case when building with gcc or clang and using the
flag `-ansi`.
