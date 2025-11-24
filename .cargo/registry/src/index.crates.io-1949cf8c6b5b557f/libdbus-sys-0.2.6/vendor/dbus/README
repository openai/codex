Sections in this file describe:
 - introduction and overview
 - low-level vs. high-level API
 - version numbers
 - options to the configure script
 - ABI stability policy

Introduction
===

D-Bus is a simple system for interprocess communication and coordination.

The "and coordination" part is important; D-Bus provides a bus daemon that does things like:
 - notify applications when other apps exit
 - start services on demand
 - support single-instance applications

See http://www.freedesktop.org/software/dbus/ for lots of documentation, 
mailing lists, etc.

See also the file CONTRIBUTING.md for notes of interest to developers
working on D-Bus.

If you're considering D-Bus for use in a project, you should be aware
that D-Bus was designed for a couple of specific use cases, a "system
bus" and a "desktop session bus." These are documented in more detail
in the D-Bus specification and FAQ available on the web site.

If your use-case isn't one of these, D-Bus may still be useful, but
only by accident; so you should evaluate carefully whether D-Bus makes
sense for your project.

Security
==

If you find a security vulnerability that is not known to the public,
please report it privately to dbus-security@lists.freedesktop.org
or by reporting a Gitlab issue at
https://gitlab.freedesktop.org/dbus/dbus/issues/new and marking it
as "confidential".

On Unix systems, the system bus (dbus-daemon --system) is designed
to be a security boundary between users with different privileges.

On Unix systems, the session bus (dbus-daemon --session) is designed
to be used by a single user, and only accessible by that user.

We do not currently consider D-Bus on Windows to be security-supported,
and we do not recommend allowing untrusted users to access Windows
D-Bus via TCP.

Note: low-level API vs. high-level binding APIs
===

A core concept of the D-Bus implementation is that "libdbus" is
intended to be a low-level API. Most programmers are intended to use
the bindings to GLib, Qt, Python, Mono, Java, or whatever. These
bindings have varying levels of completeness and are maintained as
separate projects from the main D-Bus package. The main D-Bus package
contains the low-level libdbus, the bus daemon, and a few command-line
tools such as dbus-launch.

If you use the low-level API directly, you're signing up for some
pain. Think of the low-level API as analogous to Xlib or GDI, and the
high-level API as analogous to Qt/GTK+/HTML.

Version numbers
===

D-Bus uses the common "Linux kernel" versioning system, where
even-numbered minor versions are stable and odd-numbered minor
versions are development snapshots.

So for example, development snapshots: 1.1.1, 1.1.2, 1.1.3, 1.3.4
Stable versions: 1.0, 1.0.1, 1.0.2, 1.2.1, 1.2.3

All pre-1.0 versions were development snapshots.

Development snapshots make no ABI stability guarantees for new ABI
introduced since the last stable release. Development snapshots are
likely to have more bugs than stable releases, obviously.

Configuration 
===

dbus could be build by using autotools or cmake. 

When using autotools the configure step is initiated by running ./configure 
with or without additional configuration flags. dbus requires GNU Make
(on BSD systems, this is typically called gmake) or a "make" implementation
with compatible extensions.

When using cmake the configure step is initiated by running the cmake 
program with or without additional configuration flags. 

Configuration flags
===

When using autotools, run "./configure --help" to see the possible
configuration options and environment variables.

When using cmake, inspect README.cmake to see the possible
configuration options and environment variables.
    
API/ABI Policy
===

Now that D-Bus has reached version 1.0, the objective is that all
applications dynamically linked to libdbus will continue working
indefinitely with the most recent system and session bus daemons.

 - The protocol will never be broken again; any message bus should 
   work with any client forever. However, extensions are possible
   where the protocol is extensible.

 - If the library API is modified incompatibly, we will rename it 
   as in http://ometer.com/parallel.html - in other words, 
   it will always be possible to compile against and use the older 
   API, and apps will always get the API they expect.

Interfaces can and probably will be _added_. This means both new
functions and types in libdbus, and new methods exported to
applications by the bus daemon.

The above policy is intended to make D-Bus as API-stable as other
widely-used libraries (such as GTK+, Qt, Xlib, or your favorite
example). If you have questions or concerns they are very welcome on
the D-Bus mailing list.

NOTE ABOUT DEVELOPMENT SNAPSHOTS AND VERSIONING

Odd-numbered minor releases (1.1.x, 1.3.x, 2.1.x, etc. -
major.minor.micro) are devel snapshots for testing, and any new ABI
they introduce relative to the last stable version is subject to
change during the development cycle.

Any ABI found in a stable release, however, is frozen.

ABI will not be added in a stable series if we can help it. i.e. the
ABI of 1.2.0 and 1.2.5 you can expect to be the same, while the ABI of
1.4.x may add more stuff not found in 1.2.x.

NOTE ABOUT STATIC LINKING

We are not yet firmly freezing all runtime dependencies of the libdbus
library. For example, the library may read certain files as part of
its implementation, and these files may move around between versions.

As a result, we don't yet recommend statically linking to
libdbus. Also, reimplementations of the protocol from scratch might
have to work to stay in sync with how libdbus behaves.

To lock things down and declare static linking and reimplementation to
be safe, we'd like to see all the internal dependencies of libdbus
(for example, files read) well-documented in the specification, and
we'd like to have a high degree of confidence that these dependencies
are supportable over the long term and extensible where required.

NOTE ABOUT HIGH-LEVEL BINDINGS

Note that the high-level bindings are _separate projects_ from the
main D-Bus package, and have their own release cycles, levels of
maturity, and ABI stability policies. Please consult the documentation
for your binding.

Bootstrapping D-Bus on new platforms
===

A full build of dbus, with all regression tests enabled and run, depends
on GLib. A full build of GLib, with all regression tests enabled and run,
depends on dbus.

To break this cycle, don't enable full test coverage (for at least one
of those projects) during bootstrapping. You can rebuild with full test
coverage after you have built both dbus and GLib at least once.
