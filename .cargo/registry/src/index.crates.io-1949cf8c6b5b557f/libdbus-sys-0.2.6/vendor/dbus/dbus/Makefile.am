
dbusdatadir=$(datadir)/dbus-1

AM_CPPFLAGS = \
	$(CODE_COVERAGE_CPPFLAGS) \
	-I$(top_builddir) \
	-I$(top_srcdir) \
	$(DBUS_STATIC_BUILD_CPPFLAGS) \
	$(SYSTEMD_CFLAGS) \
	$(VALGRIND_CFLAGS) \
	-DDBUS_COMPILATION \
	-DDBUS_MACHINE_UUID_FILE=\""$(localstatedir)/lib/dbus/machine-id"\" \
	-DDBUS_SYSTEM_CONFIG_FILE=\""$(dbusdatadir)/system.conf"\" \
	-DDBUS_SESSION_CONFIG_FILE=\""$(dbusdatadir)/session.conf"\" \
	-DDBUS_RUNSTATEDIR=\""$(runstatedir)"\" \
	$(NULL)

AM_CFLAGS = \
	$(CODE_COVERAGE_CFLAGS) \
	$(SANITIZE_CFLAGS) \
	$(NULL)

if HAVE_VISIBILITY
if !DBUS_WIN
AM_CFLAGS += $(CFLAG_VISIBILITY)
AM_CPPFLAGS += \
	-DDBUS_EXPORT='__attribute__((__visibility__("default")))' \
	-DDBUS_PRIVATE_EXPORT='__attribute__((__visibility__("default")))' \
	$(NULL)
endif
endif

# if assertions are enabled, improve backtraces
AM_LDFLAGS = @R_DYNAMIC_LDFLAG@

dbusincludedir=$(includedir)/dbus-1.0/dbus
dbusarchincludedir=$(libdir)/dbus-1.0/include/dbus

lib_LTLIBRARIES=libdbus-1.la

# Initialize to empty so that we can append later
nodist_libdbus_1_la_SOURCES =

#
# Deal with W32 .def and version-info.rc stuff
#
if DBUS_WIN

SUFFIXES = .rc

.rc.lo:
	$(LIBTOOL) $(AM_V_lt) --tag=RC $(AM_LIBTOOLFLAGS) $(LIBTOOLFLAGS) --mode=compile $(RC) $< -o $@

endif


#
# Platform-dependent sources:
#
if DBUS_WIN
DBUS_LIB_arch_sources = 			\
	dbus-server-win.c 			\
	dbus-server-win.h			\
	$(NULL)

nodist_libdbus_1_la_SOURCES += \
	versioninfo.rc \
	$(NULL)

if DBUS_WINCE
wince_source = dbus-sysdeps-wince-glue.h dbus-sysdeps-wince-glue.c
else
wince_source =
endif

DBUS_SHARED_arch_sources = 			\
	$(wince_source)				\
	dbus-file-win.c 			\
	dbus-pipe-win.c 			\
	dbus-sockets-win.h			\
	dbus-sysdeps-win.c 			\
	dbus-sysdeps-win.h			\
	dbus-sysdeps-thread-win.c		\
	dbus-transport-win.c			\
	dbus-transport-win.h

DBUS_UTIL_arch_sources =			\
	dbus-sysdeps-util-win.c

if ENABLE_TRADITIONAL_ACTIVATION
DBUS_UTIL_arch_sources += dbus-spawn-win.c
endif

else

if DBUS_ENABLE_LAUNCHD
launchd_source = dbus-server-launchd.h dbus-server-launchd.c
else
launchd_source =
endif

DBUS_LIB_arch_sources = 			\
	dbus-uuidgen.c				\
	dbus-uuidgen.h				\
	dbus-server-unix.c 			\
	dbus-server-unix.h

DBUS_SHARED_arch_sources = 			\
	$(launchd_source)			\
	dbus-file-unix.c 			\
	dbus-pipe-unix.c 			\
	dbus-sysdeps-unix.c 			\
	dbus-sysdeps-unix.h			\
	dbus-sysdeps-pthread.c			\
	dbus-transport-unix.c			\
	dbus-transport-unix.h			\
	dbus-userdb.c				\
	dbus-userdb.h 				\
	$(NULL)

DBUS_UTIL_arch_sources = 			\
	dbus-sysdeps-util-unix.c		\
	dbus-userdb-util.c			\
	$(NULL)

if ENABLE_TRADITIONAL_ACTIVATION
DBUS_UTIL_arch_sources += dbus-spawn-unix.c
endif
endif

if DBUS_HAVE_LINUX_EPOLL
DBUS_UTIL_arch_sources += dbus-pollable-set-epoll.c
endif

dbusinclude_HEADERS=				\
	dbus.h					\
	dbus-address.h				\
	dbus-bus.h				\
	dbus-connection.h			\
	dbus-errors.h				\
	dbus-macros.h				\
	dbus-memory.h				\
	dbus-message.h				\
	dbus-misc.h				\
	dbus-pending-call.h			\
	dbus-protocol.h				\
	dbus-server.h				\
	dbus-shared.h				\
	dbus-signature.h			\
	dbus-syntax.h				\
	dbus-threads.h				\
	dbus-types.h


nodist_dbusarchinclude_HEADERS=			\
	dbus-arch-deps.h

### source code that goes in the installed client library
### and is specific to library functionality
DBUS_LIB_SOURCES=				\
	dbus-address.c				\
	dbus-auth.c				\
	dbus-auth.h				\
	dbus-bus.c				\
	dbus-connection.c			\
	dbus-connection-internal.h		\
	dbus-credentials.c			\
	dbus-credentials.h			\
	dbus-errors.c				\
	dbus-keyring.c				\
	dbus-keyring.h				\
	dbus-marshal-header.c			\
	dbus-marshal-header.h			\
	dbus-marshal-byteswap.c			\
	dbus-marshal-byteswap.h			\
	dbus-marshal-recursive.c		\
	dbus-marshal-recursive.h		\
	dbus-marshal-validate.c			\
	dbus-marshal-validate.h			\
	dbus-message.c				\
	dbus-message-internal.h			\
	dbus-message-private.h			\
	dbus-misc.c				\
	dbus-nonce.h				\
	dbus-nonce.c				\
	dbus-object-tree.c			\
	dbus-object-tree.h			\
	dbus-pending-call.c			\
	dbus-pending-call-internal.h		\
	dbus-resources.c			\
	dbus-resources.h			\
	dbus-server.c				\
	dbus-server-debug-pipe.c		\
	dbus-server-debug-pipe.h		\
	dbus-server-protected.h			\
	dbus-server-socket.c			\
	dbus-server-socket.h			\
	$(DBUS_LIB_arch_sources)		\
	dbus-sha.c				\
	dbus-sha.h				\
	dbus-signature.c			\
	dbus-syntax.c				\
	dbus-timeout.c				\
	dbus-timeout.h				\
	dbus-threads-internal.h			\
	dbus-threads.c				\
	dbus-transport.c			\
	dbus-transport.h			\
	dbus-transport-protected.h		\
	dbus-transport-socket.c			\
	dbus-transport-socket.h			\
	dbus-watch.c				\
	dbus-watch.h

### source code that goes in the installed client library
### AND is generic utility functionality used by the
### daemon or test programs (all symbols in here should
### be underscore-prefixed)
DBUS_SHARED_SOURCES=				\
	dbus-dataslot.c				\
	dbus-dataslot.h				\
	dbus-file.c                 \
	dbus-file.h                 \
	dbus-hash.c				\
	dbus-hash.h				\
	dbus-internals.c			\
	dbus-internals.h			\
	dbus-list.c				\
	dbus-list.h				\
	dbus-marshal-basic.c			\
	dbus-marshal-basic.h			\
	dbus-memory.c				\
	dbus-mempool.c				\
	dbus-mempool.h				\
	dbus-pipe.c                 \
	dbus-pipe.h                 \
	dbus-string.c				\
	dbus-string.h				\
	dbus-string-private.h			\
	$(DBUS_SHARED_arch_sources)		\
	dbus-sysdeps.c				\
	dbus-sysdeps.h				\
	dbus-valgrind-internal.h

if DBUS_ENABLE_EMBEDDED_TESTS
DBUS_SHARED_SOURCES += \
	dbus-test-tap.c \
	dbus-test-tap.h \
	$(NULL)
# else they are included in the test library instead
endif

### source code that is generic utility functionality used
### by the bus daemon or test apps, but is NOT included
### in the D-Bus client library (all symbols in here
### should be underscore-prefixed but don't really need
### to be unless they move to DBUS_SHARED_SOURCES later)
DBUS_UTIL_SOURCES=				\
	dbus-asv-util.c				\
	dbus-asv-util.h				\
	dbus-mainloop.c				\
	dbus-mainloop.h				\
	dbus-message-util.c			\
	dbus-shell.c				\
	dbus-shell.h				\
	$(DBUS_UTIL_arch_sources)		\
	dbus-pollable-set.h			\
	dbus-pollable-set.c			\
	dbus-pollable-set-poll.c			\
	dbus-spawn.h				\
	dbus-string-util.c			\
	dbus-sysdeps-util.c			\
	dbus-test.h

libdbus_1_la_SOURCES=				\
	$(DBUS_LIB_SOURCES)			\
	$(DBUS_SHARED_SOURCES)

libdbus_internal_la_SOURCES=			\
	$(DBUS_UTIL_SOURCES)

BUILT_SOURCES=$(nodist_dbusarchinclude_HEADERS)
EXTRA_DIST=dbus-arch-deps.h.in

noinst_LTLIBRARIES=libdbus-internal.la

libdbus_1_la_CPPFLAGS = \
	$(AM_CPPFLAGS) \
	-Ddbus_1_EXPORTS \
	$(NULL)

if HAVE_LD_VERSION_SCRIPT
SYMBOL_EXPORT_LDFLAGS=-Wl,--version-script=Version
else
SYMBOL_EXPORT_LDFLAGS=
endif

libdbus_1_la_LIBADD = \
	$(CODE_COVERAGE_LIBS) \
	$(LIBDBUS_LIBS) \
	$(NULL)
libdbus_1_la_LDFLAGS = \
	$(AM_LDFLAGS) \
	-version-info $(LT_CURRENT):$(LT_REVISION):$(LT_AGE) \
	$(SYMBOL_EXPORT_LDFLAGS) \
	-no-undefined \
	$(NULL)

libdbus_internal_la_LIBADD = \
	$(CODE_COVERAGE_LIBS) \
	$(LIBDBUS_LIBS) \
	libdbus-1.la \
	$(NULL)

if DBUS_WIN
AM_CXXFLAGS = \
	$(CODE_COVERAGE_CXXFLAGS) \
	$(NULL)

# This must be a separate convenience library, otherwise libtool notices
# that libdbus-1 might contain C++, links it with g++ and links in libstdc++,
# even on Unix where in fact it doesn't contain any C++. For Windows, where
# this code is used, we don't actually need libstdc++.
noinst_LTLIBRARIES += libdbus-init-win.la
libdbus_init_win_la_SOURCES = dbus-init-win.cpp
libdbus_1_la_LIBADD += libdbus-init-win.la
endif

noinst_PROGRAMS =

# Add rules for code-coverage testing, as defined by AX_CODE_COVERAGE
include $(top_srcdir)/aminclude_static.am

clean-local:
	$(AM_V_at)rm -fr ./.dbus-keyrings
