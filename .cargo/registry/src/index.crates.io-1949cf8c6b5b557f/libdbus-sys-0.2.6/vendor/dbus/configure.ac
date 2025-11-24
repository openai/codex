dnl -*- mode: m4 -*-
AC_PREREQ([2.63])

m4_define([dbus_major_version], [1])
m4_define([dbus_minor_version], [14])
m4_define([dbus_micro_version], [4])
m4_define([dbus_version],
          [dbus_major_version.dbus_minor_version.dbus_micro_version])
AC_INIT([dbus], [dbus_version], [https://gitlab.freedesktop.org/dbus/dbus/issues], [dbus])

AC_CONFIG_AUX_DIR([build-aux])

m4_pattern_forbid([^AX_(CHECK_ENABLE_DEBUG|CODE_COVERAGE|COMPILER_FLAGS|COMPILER_FLAGS_(CFLAGS|CXXFLAGS|LDFLAGS))\b],
  [Unexpanded AX_ macro found. Please install GNU autoconf-archive])

AC_CANONICAL_HOST

AC_CONFIG_HEADERS([config.h])
AC_CONFIG_MACRO_DIR([m4])

AM_INIT_AUTOMAKE([1.13 tar-ustar no-dist-gzip dist-xz -Wno-portability subdir-objects foreign])

GETTEXT_PACKAGE=dbus-1
AC_SUBST(GETTEXT_PACKAGE)
AC_DEFINE_UNQUOTED(GETTEXT_PACKAGE,"$GETTEXT_PACKAGE",[The name of the gettext domain])

# By default, rebuild autotools files on demand; only use ./missing if the
# user says --disable-maintainer-mode (some distributions like to do this)
AM_MAINTAINER_MODE([enable])

m4_ifdef([AM_SILENT_RULES],[AM_SILENT_RULES([yes])])

AC_DEFINE_UNQUOTED(DBUS_DAEMON_NAME,"dbus-daemon",[Name of executable])

# libtool versioning - this applies to libdbus
#
# See http://sources.redhat.com/autobook/autobook/autobook_91.html#SEC91 for details
#

## increment if the interface has additions, changes, removals.
LT_CURRENT=35

## increment any time the source changes; set to
##  0 if you increment CURRENT
LT_REVISION=1

## increment if any interfaces have been added; set to 0
## if any interfaces have been changed or removed. removal has
## precedence over adding, so set to 0 if both happened.
LT_AGE=32

AC_SUBST(LT_CURRENT)
AC_SUBST(LT_REVISION)
AC_SUBST(LT_AGE)
SOVERSION=`expr ${LT_CURRENT} - ${LT_AGE}`
AC_SUBST([SOVERSION])

DBUS_MAJOR_VERSION=dbus_major_version
DBUS_MINOR_VERSION=dbus_minor_version
DBUS_MICRO_VERSION=dbus_micro_version
DBUS_VERSION=dbus_major_version.dbus_minor_version.dbus_micro_version

AC_SUBST(DBUS_MAJOR_VERSION)
AC_SUBST(DBUS_MINOR_VERSION)
AC_SUBST(DBUS_MICRO_VERSION)
AC_SUBST(DBUS_VERSION)

dnl
dnl Build configuration
dnl

dnl This must come first: other options use this to set their defaults. Don't
dnl enable developer mode on production builds.
AC_ARG_ENABLE([developer],
  [AS_HELP_STRING([--enable-developer],
    [set defaults to be appropriate for a D-Bus developer instead of a distribution/end-user])],
  [enable_developer=$enableval],
  [enable_developer=no])

dnl 'disable_developer' is the negation of 'enable_developer'. If
dnl 'enable-developer' is set to 'no' (the default), the value of
dnl 'disable_developer' is set to 'yes', and vice versa. It's used
dnl for macros that require the opposite of 'enable_developer', such
dnl as several AX_ macros.
dnl See https://bugs.freedesktop.org/show_bug.cgi?id=97357
AS_IF([test "x$enable_developer" = "xyes"],[
    disable_developer=no
  ],[
    disable_developer=yes
  ])

# The debugging check must run before the compiler tests. Other command-line
# options also use it to set their defaults. We disable debugging by default,
# except for developer builds.
AX_CHECK_ENABLE_DEBUG([$enable_developer])

AC_PROG_CC
AM_PROG_CC_C_O
AC_PROG_CXX
AC_USE_SYSTEM_EXTENSIONS
AC_SYS_LARGEFILE
AC_ISC_POSIX
AC_HEADER_STDC
AC_C_INLINE
AM_PROG_LIBTOOL
AC_PROG_MKDIR_P
PKG_PROG_PKG_CONFIG

# TAP test driver support
AC_PROG_AWK
AC_REQUIRE_AUX_FILE([tap-driver.sh])

# This must come before we set up compiler warnings because it assumes
# non-use of -Werror=missing-prototypes
gl_VISIBILITY
AM_CONDITIONAL([HAVE_VISIBILITY], [test "x$HAVE_VISIBILITY" = x1])

# Initialize libtool
LT_INIT([win32-dll])
LT_LANG([Windows Resource])

# Set some internal variables depending on the platform for later use.
dbus_win=no
dbus_cygwin=no
dbus_unix=no
case "${host}" in
    *-mingw32ce*)
        dbus_win=yes
        dbus_wince=yes
        ;;
    *-mingw32*)
        dbus_win=yes
        ;;
    *-cygwin*)
        dbus_cygwin=yes
        dbus_unix=yes
        ;;
    *)
        dbus_unix=yes
       ;;
esac

# Special defines for certain platforms
if test "$dbus_win" = yes; then
    AC_DEFINE(DBUS_WIN,1,[Defined if we run on a W32 API based system])
    # Yes, on Windows it really does work like this.
    # http://support.microsoft.com/kb/111855
    AC_DEFINE(FD_SETSIZE,8192,[The maximum number of connections that can be handled at once])
    BUILD_TIMESTAMP=`date --iso-8601=minutes`
    AC_SUBST(BUILD_TIMESTAMP)
    # Assume DBUS_VERSION is always three numbers
    BUILD_FILEVERSION=`echo "$DBUS_VERSION" | sed -e 's/\./,/g'`,0
    AC_SUBST(BUILD_FILEVERSION)
    # In the CMake build system we generate multiple files, versioninfo-*.rc, with a
    # different "internal name" and "original filename", for embedding in multiple
    # executables. In the Autotools build system, we currently only generate
    # versioninfo.rc and embed it in libdbus-1-${SOVERSION}.dll.
    AC_SUBST([DBUS_VER_INTERNAL_NAME], [libdbus-1-${SOVERSION}])
    AC_SUBST([DBUS_VER_ORIGINAL_NAME], [libdbus-1-${SOVERSION}.dll])
    AC_SUBST([DBUS_VER_FILE_TYPE], [VFT_DLL])
    AS_IF([test -z "$RC"],
        [AC_MSG_ERROR([An implementation of windres is required])])
    if test "$dbus_wince" = yes; then
      AC_DEFINE(DBUS_WINCE,1,[Defined if we run on a W32 CE API based system])
      AC_DEFINE(_WIN32_WCE, 0x0502, [Defined to get newer W32 CE APIs])
    else
      AC_DEFINE([_WIN32_WINNT], [0x0600],
        [Define to the minimum supported Windows version (0x0600 is Vista)])
    fi
else
    AC_DEFINE(DBUS_UNIX,1,[Defined if we run on a Unix-based system])
fi
if test "$dbus_cygwin" = yes; then
    AC_DEFINE(DBUS_CYGWIN,1,[Defined if we run on a cygwin API based system])
fi

# For best security, assume that all non-Windows platforms can do
# credentials-passing.
AS_IF([test "$dbus_win" = yes],
    [DBUS_SESSION_CONF_MAYBE_AUTH_EXTERNAL="<!--<auth>EXTERNAL</auth>-->"],
    [DBUS_SESSION_CONF_MAYBE_AUTH_EXTERNAL="<auth>EXTERNAL</auth>"])
AC_SUBST([DBUS_SESSION_CONF_MAYBE_AUTH_EXTERNAL])

AM_CONDITIONAL(DBUS_WIN, test "$dbus_win" = yes)
AM_CONDITIONAL(DBUS_WINCE, test "$dbus_wince" = yes)
AM_CONDITIONAL(DBUS_UNIX, test "$dbus_unix" = yes)
AM_CONDITIONAL(DBUS_CYGWIN, test "$dbus_cygwin" = yes)

DBUS_STATIC_BUILD_CPPFLAGS=
if test "x$enable_shared" = xno; then
    # On Windows, linking against the static library requires special effort
    # to turn off DLL import/export processing. We normally link some things
    # against the dynamic library, but if we're not building that, we'll
    # have to link everything statically.
    DBUS_STATIC_BUILD_CPPFLAGS=-DDBUS_STATIC_BUILD
fi
AC_SUBST([DBUS_STATIC_BUILD_CPPFLAGS])

AC_ARG_ENABLE(ansi, AS_HELP_STRING([--enable-ansi],[enable -ansi -pedantic gcc flags]),enable_ansi=$enableval,enable_ansi=no)
AC_ARG_ENABLE(verbose-mode, AS_HELP_STRING([--enable-verbose-mode],[support verbose debug mode]),enable_verbose_mode=$enableval,enable_verbose_mode=$enable_developer)
AC_ARG_ENABLE(asserts, AS_HELP_STRING([--enable-asserts],[include assertion checks]),enable_asserts=$enableval,enable_asserts=$enable_developer)
AC_ARG_ENABLE(checks, AS_HELP_STRING([--enable-checks],[include sanity checks on public API]),enable_checks=$enableval,enable_checks=yes)
AC_ARG_ENABLE(xml-docs, AS_HELP_STRING([--enable-xml-docs],[build XML documentation (requires xmlto)]),enable_xml_docs=$enableval,enable_xml_docs=auto)
AC_ARG_ENABLE(doxygen-docs, AS_HELP_STRING([--enable-doxygen-docs],[build DOXYGEN documentation (requires Doxygen)]),enable_doxygen_docs=$enableval,enable_doxygen_docs=auto)
AC_ARG_ENABLE([ducktype-docs],
              AS_HELP_STRING([--enable-ducktype-docs],
                             [build Ducktype documentation (requires Ducktype)]),
              [enable_ducktype_docs=$enableval], [enable_ducktype_docs=auto])
AC_ARG_ENABLE(selinux, AS_HELP_STRING([--enable-selinux],[build with SELinux support]),enable_selinux=$enableval,enable_selinux=auto)
AC_ARG_ENABLE([apparmor],
  [AS_HELP_STRING([--enable-apparmor], [build with AppArmor support])],
  [enable_apparmor=$enableval],
  [enable_apparmor=auto])
AC_ARG_ENABLE(libaudit,AS_HELP_STRING([--enable-libaudit],[build audit daemon support for SELinux]),enable_libaudit=$enableval,enable_libaudit=auto)
AC_ARG_ENABLE(inotify, AS_HELP_STRING([--enable-inotify],[build with inotify support (linux only)]),enable_inotify=$enableval,enable_inotify=auto)
AC_ARG_ENABLE(kqueue, AS_HELP_STRING([--enable-kqueue],[build with kqueue support]),enable_kqueue=$enableval,enable_kqueue=auto)
AC_ARG_ENABLE(console-owner-file, AS_HELP_STRING([--enable-console-owner-file],[enable console owner file]),enable_console_owner_file=$enableval,enable_console_owner_file=auto)
AC_ARG_ENABLE(launchd, AS_HELP_STRING([--enable-launchd],[build with launchd auto-launch support]),enable_launchd=$enableval,enable_launchd=auto)
AC_ARG_ENABLE(systemd, AS_HELP_STRING([--enable-systemd],[build with systemd at_console support]),enable_systemd=$enableval,enable_systemd=auto)
AC_ARG_ENABLE(traditional-activation, AS_HELP_STRING([--disable-traditional-activation], [Do not build support for service activation without using SystemdService]), enable_traditional_activation="$enableval", enable_traditional_activation=yes)

AC_ARG_WITH(session-socket-dir, AS_HELP_STRING([--with-session-socket-dir=[dirname]],[Where to put sockets for the per-login-session message bus]))
AC_ARG_WITH(test-socket-dir, AS_HELP_STRING([--with-test-socket-dir=[dirname]],[Where to put sockets for make check]))
AC_ARG_WITH(system-pid-file, AS_HELP_STRING([--with-system-pid-file=[pidfile]],[PID file for systemwide daemon]))
AC_ARG_WITH(system-socket, AS_HELP_STRING([--with-system-socket=[filename]],[UNIX domain socket for systemwide daemon]))
AC_ARG_WITH(console-auth-dir, AS_HELP_STRING([--with-console-auth-dir=[dirname]],[directory to check for console ownerhip]))
AC_ARG_WITH(console-owner-file, AS_HELP_STRING([--with-console-owner-file=[filename]],[file whose owner determines current console owner]))
AC_ARG_WITH(launchd-agent-dir, AS_HELP_STRING([--with-launchd-agent-dir=[dirname]],[directory to put the launchd agent (default: /Library/LaunchAgents)]))
AC_ARG_WITH(dbus_user, AS_HELP_STRING([--with-dbus-user=<user>],[User for running the DBUS daemon (messagebus)]))
AC_ARG_WITH([test_user],
  [AS_HELP_STRING([--with-test-user=<user>],
    [Unprivileged user for regression tests, other than root and the dbus_user (default: nobody)])])
AC_ARG_WITH(dbus_daemondir, AS_HELP_STRING([--with-dbus-daemondir=[dirname]],[Directory for installing the DBUS daemon]))

AC_ARG_ENABLE([embedded-tests],
  AS_HELP_STRING([--enable-embedded-tests],
    [enable unit test code in the library and binaries]),
  [], [enable_embedded_tests=$enable_developer])
AC_ARG_ENABLE([modular-tests],
  AS_HELP_STRING([--enable-modular-tests],
    [enable modular regression tests (requires GLib)]),
  [], [enable_modular_tests=auto])
# --enable-tests overrides both --enable-embedded-tests and
# --enable-modular-tests
AC_ARG_ENABLE([tests],
  AS_HELP_STRING([--enable-tests],
    [enable/disable all tests, overriding embedded-tests/modular-tests]),
  [
  if test "x$enableval" = xyes; then
    AC_MSG_NOTICE([Full test coverage was requested with --enable-tests=yes])
    AC_MSG_NOTICE([This requires GLib])
  fi
  enable_embedded_tests=$enableval
  enable_modular_tests=$enableval
  ],
  [])

# DBUS_ENABLE_EMBEDDED_TESTS controls unit tests built in to .c files
# and also some stuff in the test/ subdir.
AM_CONDITIONAL([DBUS_ENABLE_EMBEDDED_TESTS],
  [test "x$enable_embedded_tests" = xyes])
if test "x$enable_embedded_tests" = xyes; then
    AC_DEFINE([DBUS_ENABLE_EMBEDDED_TESTS], [1],
      [Define to build test code into the library and binaries])
fi

# DBUS_ENABLE_MODULAR_TESTS controls tests that work based on public API.
# These use GTest, from GLib, because life's too short. They're enabled by
# default (unless you don't have GLib), because they don't bloat the library
# or binaries.

dnl Don't do anything too subtle here, because the CMake build system
dnl parses these lines with regular expressions. If necessary, adjust
dnl cmake/modules/MacrosAutotools.cmake to compensate.
AC_DEFINE([GLIB_VERSION_MIN_REQUIRED], [GLIB_VERSION_2_38], [Ignore post-2.38 deprecations])
AC_DEFINE([GLIB_VERSION_MAX_ALLOWED], [G_ENCODE_VERSION(2,44)], [Prevent post-2.44 APIs])

with_glib=yes

AS_IF([test "x$enable_modular_tests" != xno],
  [
  PKG_CHECK_MODULES([GLIB], [glib-2.0 >= 2.40, gio-2.0 >= 2.40],
    [
      AS_IF([test "x$dbus_unix" = xyes],
        [PKG_CHECK_MODULES([GIO_UNIX], [gio-unix-2.0],
           [AC_DEFINE([HAVE_GIO_UNIX], [1], [Define if you have gio-unix-2.0])], [])])
    ],
    [if test "x$enable_modular_tests" = xyes; then
      AC_MSG_NOTICE([Full test coverage (--enable-modular-tests=yes or --enable-tests=yes) requires GLib])
      AC_MSG_ERROR([$GLIB_ERRORS])
    else # assumed to be "auto"
      with_glib=no
    fi])
  ],
  [with_glib=no])

if test "x$enable_modular_tests" != xno; then
  AC_DEFINE([DBUS_ENABLE_MODULAR_TESTS], [1],
    [Define to build independent test binaries])
fi
AM_CONDITIONAL([DBUS_ENABLE_MODULAR_TESTS],
  [test "x$enable_modular_tests" != xno])

if test "x$with_glib" != xno; then
  AC_DEFINE([DBUS_WITH_GLIB], [1],
    [Define if GLib, GObject, GIO are available])
fi
AM_CONDITIONAL([DBUS_WITH_GLIB], [test "x$with_glib" != xno])

AC_ARG_ENABLE([installed-tests],
  AS_HELP_STRING([--enable-installed-tests],
    [enable unit test code in the library and binaries]),
  [], [enable_installed_tests=no])
AM_CONDITIONAL([DBUS_ENABLE_INSTALLED_TESTS],
  [test "x$enable_installed_tests" = xyes])

if test x$enable_verbose_mode = xyes; then
    AC_DEFINE(DBUS_ENABLE_VERBOSE_MODE,1,[Support a verbose mode])
fi

dnl Intentional:
dnl - $DISABLE_WARNINGS disables unused-label warnings if not
dnl   checking or not asserting (tested further below)
dnl - we are not going to stop using deprecated functions on a stable
dnl   branch
dnl - missing field initializers being 0 is a C feature, not a bug
dnl - unused-parameter is to make writing callbacks less annoying
DISABLE_WARNINGS="$DISABLE_WARNINGS
                  -Wno-deprecated-declarations
                  -Wno-missing-field-initializers
                  -Wno-unused-parameter"

if test x$enable_asserts = xno; then
    AC_DEFINE(DBUS_DISABLE_ASSERT,1,[Disable assertion checking])
    DISABLE_WARNINGS="$DISABLE_WARNINGS -Wno-unused-label"
    R_DYNAMIC_LDFLAG=""
    if test x$enable_embedded_tests = xyes; then
        DISABLE_WARNINGS="$DISABLE_WARNINGS
                          -Wno-unused-but-set-variable
                          -Wno-unused-variable
                          -Wno-unused-function"
    fi
else
    # -rdynamic is needed for glibc's backtrace_symbols to work.
    # No clue how much overhead this adds, but it's useful
    # to do this on any assertion failure,
    # so for now it's enabled anytime asserts are (currently not
    # in production builds).

    # To get -rdynamic you pass -export-dynamic to libtool.
    AC_DEFINE(DBUS_BUILT_R_DYNAMIC,1,[whether -export-dynamic was passed to libtool])
    R_DYNAMIC_LDFLAG=-export-dynamic
fi
AC_SUBST(R_DYNAMIC_LDFLAG)

if test x$enable_checks = xno; then
    AC_DEFINE(DBUS_DISABLE_CHECKS,1,[Disable public API sanity checking])
    AC_DEFINE(G_DISABLE_CHECKS,1,[Disable GLib public API sanity checking])
    DISABLE_WARNINGS="$DISABLE_WARNINGS -Wno-unused-label"
fi

AH_BOTTOM([
/* explicitly define these macros to get less confusing conditions */
#ifndef DBUS_DISABLE_ASSERT
#  define DBUS_ENABLE_ASSERT 1
#endif
#ifndef DBUS_DISABLE_CHECKS
#  define DBUS_ENABLE_CHECKS 1
#endif])

# Test for code-coverage tools if --enable-code-coverage
AX_CODE_COVERAGE

AS_IF([test x$enable_code_coverage = xyes],[
    AC_DEFINE_UNQUOTED(
      [DBUS_GCOV_ENABLED], [1],
      [Defined if gcov is enabled to force a rebuild due to config.h changing])
  ])

#### Simple checks for things with no special dependencies

# This construct is only suitable for functions that are in the system's
# standard C library and do not have unusual dependencies. Other
# functions need their own checks. Please keep sorted in LC_ALL=C order
AC_CHECK_FUNCS_ONCE([
accept4
clearenv
fpathconf
getgrouplist
getpeereid
getpeerucred
getrandom
getresuid
getrlimit
inotify_init1
issetugid
localeconv
nanosleep
pipe2
poll
prctl
prlimit
raise
setenv
setlocale
setresuid
setrlimit
socketpair
strtoll
strtoull
unsetenv
usleep
])

# This construct is only suitable for headers that are self-contained
# and do not require extra headers to be included first. More complex
# headers need their own checks. Please keep sorted in LC_ALL=C order
AC_CHECK_HEADERS_ONCE([
alloca.h
byteswap.h
crt_externs.h
dirent.h
errno.h
locale.h
signal.h
stdint.h
sys/prctl.h
sys/random.h
sys/resource.h
sys/time.h
unistd.h
ws2tcpip.h
])

#### Integer sizes

AC_CHECK_SIZEOF(char)
AC_CHECK_SIZEOF(short)
AC_CHECK_SIZEOF(long)
AC_CHECK_SIZEOF(int)
AC_CHECK_SIZEOF(void *)
AC_CHECK_SIZEOF(long long)
AC_CHECK_SIZEOF(__int64)

### See what our 64 bit type is called
AC_MSG_CHECKING([64-bit integer type])

case 8 in
$ac_cv_sizeof_int)
  dbusint64=int
  dbusint64_constant='(val)'
  dbusuint64_constant='(val)'
  ;;
$ac_cv_sizeof_long)
  dbusint64=long
  dbusint64_constant='(val##L)'
  dbusuint64_constant='(val##UL)'
  ;;
$ac_cv_sizeof_long_long)
  dbusint64='long long'
  dbusint64_constant='(val##LL)'
  dbusuint64_constant='(val##ULL)'
  ;;
$ac_cv_sizeof___int64)
  dbusint64=__int64
  dbusint64_constant='(val##i64)'
  dbusuint64_constant='(val##ui64)'
  ;;
esac

AS_IF(
  [test -z "$dbusint64"],
  [AC_MSG_RESULT([not found])
  AC_MSG_ERROR([Could not find a 64-bit integer type.

Please report a bug here with details of your platform and compiler:

    http://bugs.freedesktop.org/enter_bug.cgi?product=DBus&component=core])
  ],
  dnl else
  [
        DBUS_INT64_TYPE="$dbusint64"
        DBUS_INT64_CONSTANT="$dbusint64_constant"
        DBUS_UINT64_CONSTANT="$dbusuint64_constant"
        AC_MSG_RESULT($DBUS_INT64_TYPE)
  ])

AC_SUBST(DBUS_INT64_TYPE)
AC_SUBST(DBUS_INT64_CONSTANT)
AC_SUBST(DBUS_UINT64_CONSTANT)

### see what 32-bit int is called
AC_MSG_CHECKING([32-bit integer type])

case 4 in
$ac_cv_sizeof_short)
  dbusint32=short
  ;;
$ac_cv_sizeof_int)
  dbusint32=int
  ;;
$ac_cv_sizeof_long)
  dbusint32=long
  ;;
esac

if test -z "$dbusint32" ; then
        DBUS_INT32_TYPE="no_int32_type_detected"
        AC_MSG_ERROR([No 32-bit integer type found])
else
        DBUS_INT32_TYPE="$dbusint32"
        AC_MSG_RESULT($DBUS_INT32_TYPE)
fi

AC_SUBST(DBUS_INT32_TYPE)

### see what 16-bit int is called
AC_MSG_CHECKING([16-bit integer type])

case 2 in
$ac_cv_sizeof_short)
  dbusint16=short
  ;;
$ac_cv_sizeof_int)
  dbusint16=int
  ;;
esac

if test -z "$dbusint16" ; then
        DBUS_INT16_TYPE="no_int16_type_detected"
        AC_MSG_ERROR([No 16-bit integer type found])
else
        DBUS_INT16_TYPE="$dbusint16"
        AC_MSG_RESULT($DBUS_INT16_TYPE)
fi

AC_SUBST(DBUS_INT16_TYPE)

## byte order
case $host_os in
	darwin*)
		# check at compile-time, so that it is possible to build universal
		# (with multiple architectures at once on the compile line)
		AH_VERBATIM([WORDS_BIGENDIAN_DARWIN], [
			/* Use the compiler-provided endianness defines to allow universal compiling. */
			#if defined(__BIG_ENDIAN__)
			#define WORDS_BIGENDIAN 1
			#endif
		])
		;;
	*)
		AC_C_BIGENDIAN
		;;
esac

# As a GNU extension, glibc declares environ in unistd.h, which is one of
# the AC_INCLUDES_DEFAULT.
AC_CHECK_DECLS([environ])

dnl **********************************
dnl *** va_copy checks (from GLib) ***
dnl **********************************
dnl we currently check for all three va_copy possibilities, so we get
dnl all results in config.log for bug reports.
AC_CACHE_CHECK([for an implementation of va_copy()],dbus_cv_va_copy,[
	AC_LINK_IFELSE([AC_LANG_SOURCE([#include <stdarg.h>
#include <stdlib.h>
	static void f (int i, ...) {
	va_list args1, args2;
	va_start (args1, i);
	va_copy (args2, args1);
	if (va_arg (args2, int) != 42 || va_arg (args1, int) != 42)
	  exit (1);
	va_end (args1); va_end (args2);
	}
	int main() {
	  f (0, 42);
	  return 0;
	}])],
	[dbus_cv_va_copy=yes],
	[dbus_cv_va_copy=no])
])
AC_CACHE_CHECK([for an implementation of __va_copy()],dbus_cv___va_copy,[
	AC_LINK_IFELSE([AC_LANG_SOURCE([#include <stdarg.h>
#include <stdlib.h>
	static void f (int i, ...) {
	va_list args1, args2;
	va_start (args1, i);
	__va_copy (args2, args1);
	if (va_arg (args2, int) != 42 || va_arg (args1, int) != 42)
	  exit (1);
	va_end (args1); va_end (args2);
	}
	int main() {
	  f (0, 42);
	  return 0;
	}])],
	[dbus_cv___va_copy=yes],
	[dbus_cv___va_copy=no])
])

if test "x$dbus_cv_va_copy" = "xyes"; then
  dbus_va_copy_func=va_copy
else if test "x$dbus_cv___va_copy" = "xyes"; then
  dbus_va_copy_func=__va_copy
fi
fi

AS_IF([test -n "$dbus_va_copy_func"],
  [AC_DEFINE_UNQUOTED([DBUS_VA_COPY], [$dbus_va_copy_func], [A 'va_copy' style function])],
  [AC_MSG_ERROR([dbus requires an ISO C99-compatible va_copy() macro, or a compatible __va_copy(), or MSVC >= 2010 and CMake])])

#### Atomic integers

AC_CACHE_CHECK([whether $CC knows __sync_sub_and_fetch()],
  dbus_cv_sync_sub_and_fetch,
  [AC_LINK_IFELSE([
     AC_LANG_PROGRAM([[]], [[int a = 4; int b = __sync_sub_and_fetch(&a, 4); return b; ]])],
     [dbus_cv_sync_sub_and_fetch=yes],
     [dbus_cv_sync_sub_and_fetch=no])
  ])

if test "x$dbus_cv_sync_sub_and_fetch" = "xyes" ; then
   have_sync=1
else
   have_sync=0
fi

AC_DEFINE_UNQUOTED([DBUS_USE_SYNC], [$have_sync], [Use the gcc __sync extension])

#### Various functions
AC_SEARCH_LIBS(socket,[socket network])
AC_CHECK_FUNC(gethostbyname,,[AC_CHECK_LIB(nsl,gethostbyname)])

AC_CHECK_HEADERS([syslog.h])
if test "x$ac_cv_header_syslog_h" = "xyes"; then
  AC_CHECK_DECLS([LOG_PERROR], [], [], [[#include <syslog.h>]])
fi

AC_MSG_CHECKING(for dirfd)
AC_LINK_IFELSE([AC_LANG_PROGRAM([[
#include <sys/types.h>
#include <dirent.h>
]], [[
DIR *dirp;
dirp = opendir(".");
dirfd(dirp);
closedir(dirp);
]])],
[dbus_have_dirfd=yes],
[dbus_have_dirfd=no])
AC_MSG_RESULT($dbus_have_dirfd)
if test "$dbus_have_dirfd" = yes; then
	AC_DEFINE(HAVE_DIRFD,1,[Have dirfd function])
else
	AC_MSG_CHECKING(for DIR *dirp->dd_fd)
	AC_LINK_IFELSE([AC_LANG_PROGRAM([[
#include <sys/types.h>
#include <dirent.h>
	]], [[
DIR *dirp;
int fd;
dirp = opendir(".");
fd = dirp->dd_fd;
closedir(dirp);
        ]])],
        [dbus_have_ddfd=yes],
        [dbus_have_ddfd=no])
	AC_MSG_RESULT($dbus_have_ddfd)
	if test "$dbus_have_ddfd" = yes; then
		AC_DEFINE(HAVE_DDFD,1,[Have the ddfd member of DIR])
	fi
fi

AC_CHECK_HEADERS([execinfo.h],
	[AC_SEARCH_LIBS([backtrace], [execinfo],
		[AC_DEFINE([HAVE_BACKTRACE], [1],
			[Define to 1 if you have backtrace().])])])

# Add -D_POSIX_PTHREAD_SEMANTICS if on Solaris
#
case $host_os in
    solaris*)
       CFLAGS="$CFLAGS -D_POSIX_PTHREAD_SEMANTICS" ;;
esac

AC_CHECK_FUNCS_ONCE([getpwnam_r])

dnl check for socklen_t
AC_MSG_CHECKING(whether socklen_t is defined)
AC_COMPILE_IFELSE([AC_LANG_PROGRAM([[
#include <sys/types.h>
#include <sys/socket.h>
#include <netdb.h>
]], [[
socklen_t foo;
foo = 1;
]])],
[dbus_have_socklen_t=yes],
[dbus_have_socklen_t=no])
AC_MSG_RESULT($dbus_have_socklen_t)

if test "x$dbus_have_socklen_t" = "xyes"; then
    AC_DEFINE(HAVE_SOCKLEN_T,1,[Have socklen_t type])
fi

dnl check for writev header and writev function so we're
dnl good to go if HAVE_WRITEV gets defined.
AC_CHECK_HEADERS(sys/uio.h, [AC_CHECK_FUNCS(writev)])

dnl Make it easy to check if we have MSG_NOSIGNAL without actually having to include sys/socket.h
AC_CHECK_DECLS([MSG_NOSIGNAL], [], [], [[ #include <sys/types.h>
#include <sys/socket.h> ]])

dnl Check for various credentials.
AC_MSG_CHECKING(for struct cmsgcred)
AC_COMPILE_IFELSE([AC_LANG_PROGRAM([[
#include <sys/types.h>
#include <sys/socket.h>
]], [[
struct cmsgcred cred;

cred.cmcred_pid = 0;
]])],
[dbus_have_struct_cmsgcred=yes],
[dbus_have_struct_cmsgcred=no])
AC_MSG_RESULT($dbus_have_struct_cmsgcred)

if test x$dbus_have_struct_cmsgcred = xyes; then
    AC_DEFINE(HAVE_CMSGCRED,1,[Have cmsgcred structure])
fi

AC_CHECK_MEMBER([struct unpcbid.unp_pid],
                [AC_DEFINE([HAVE_UNPCBID], 1, [Have unpcbid structure])],
                [],
                [[#include <sys/un.h>]])

PKG_CHECK_MODULES([EXPAT], [expat])

save_cflags="$CFLAGS"
save_libs="$LIBS"
CFLAGS="$CFLAGS $EXPAT_CFLAGS"
LIBS="$LIBS $EXPAT_LIBS"
AC_CHECK_FUNCS([XML_SetHashSalt])
CFLAGS="$save_cflags"
LIBS="$save_libs"

# Thread lib detection
AC_ARG_VAR([THREAD_LIBS])
save_libs="$LIBS"
LIBS="$LIBS $THREAD_LIBS"

is_missing_pthread_function="is required when compiling D-Bus on Unix platforms, but is not in your libc or libpthread. Please open a bug on https://bugs.freedesktop.org/enter_bug.cgi?product=dbus with details of your platform."

# Don't do these automatic checks if the user set THREAD_LIBS on the
# configure command-line. If they did, we assume they're right.
#
# We also don't do these checks on Windows, because you don't need magical
# linker flags to have threading support there.
AS_IF([test "x$dbus_unix" = xyes && test "x$THREAD_LIBS" = x],
  [
    # Mandatory pthread functions. In principle, some of these could be made
    # optional if there are platforms that don't have them.
    #
    # Currently, we only look in -lpthread.
    # In principle we might need to look in -lpthreads, -lthreads, ...
    # as well - please file a bug if your platform needs this.
    AC_SEARCH_LIBS([pthread_cond_timedwait],
        [pthread],
        [THREAD_LIBS="$LIBS"],
        [AC_MSG_ERROR([pthread_cond_timedwait $is_missing_pthread_function])],
        [])
    AC_SEARCH_LIBS([pthread_mutexattr_init],
        [pthread],
        [THREAD_LIBS="$LIBS"],
        [AC_MSG_ERROR([pthread_mutexattr_init $is_missing_pthread_function])],
        [])
    AC_SEARCH_LIBS([pthread_mutexattr_settype],
        [pthread],
        [THREAD_LIBS="$LIBS"],
        [AC_MSG_ERROR([pthread_mutexattr_settype $is_missing_pthread_function])],
        [])

    # Optional, for monotonic clocks. Because it's optional, this check
    # is non-fatal if we don't find it.
    AC_SEARCH_LIBS([pthread_condattr_setclock],
        [pthread],
        [THREAD_LIBS="$LIBS"])

    AS_IF([test "x$ac_cv_search_pthread_condattr_setclock" != xno],
      [
        AC_SEARCH_LIBS([clock_getres], [rt], [THREAD_LIBS="$LIBS"])
        AC_MSG_CHECKING([for CLOCK_MONOTONIC])
        AC_COMPILE_IFELSE([AC_LANG_PROGRAM(
[[#include <time.h>
#include <pthread.h>
]], [[
struct timespec monotonic_timer;
pthread_condattr_t attr;
pthread_condattr_init (&attr);
pthread_condattr_setclock (&attr, CLOCK_MONOTONIC);
clock_getres (CLOCK_MONOTONIC,&monotonic_timer);
]])],
            [have_clock_monotonic=true],
            [have_clock_monotonic=false])
        AS_IF([test x$have_clock_monotonic = xtrue],
	  [
            AC_MSG_RESULT([found])
            AC_DEFINE(HAVE_MONOTONIC_CLOCK, 1, [Define if we have CLOCK_MONOTONIC])
	  ],
          [AC_MSG_RESULT([not found])])
      ]) dnl have pthread_condattr_setclock
  ]) dnl on Unix

LIBS="$save_libs"

AC_SUBST([THREAD_LIBS])

# SELinux detection
if test x$enable_selinux = xno ; then
    have_selinux=no;
else
    # See if we have SELinux library
    PKG_CHECK_MODULES([SELINUX], [libselinux >= 2.0.86],
                 [have_selinux=yes], [have_selinux=no])

    if test x$enable_selinux = xauto ; then
        if test x$have_selinux = xno ; then
                AC_MSG_WARN([Sufficiently new SELinux library not found])
        fi
    else
        if test x$have_selinux = xno ; then
                AC_MSG_ERROR([SElinux explicitly required, and SELinux library not found])
        fi
    fi
fi

AM_CONDITIONAL(HAVE_SELINUX, test x$have_selinux = xyes)

if test x$have_selinux = xyes ; then
    # the selinux code creates threads
    # which requires libpthread even on linux
    AC_CHECK_FUNC(pthread_create,,[AC_CHECK_LIB(pthread,pthread_create,
                                                [SELINUX_THREAD_LIBS="-lpthread"])])

    SELINUX_LIBS="$SELINUX_LIBS $SELINUX_THREAD_LIBS"
    AC_DEFINE(HAVE_SELINUX,1,[SELinux support])
else
    SELINUX_LIBS=
fi

# AppArmor detection
AS_IF([test x$enable_apparmor = xno],
  [have_apparmor=no],
  [
  PKG_CHECK_MODULES([APPARMOR], [libapparmor >= 2.8.95],
                    [have_apparmor=yes], [have_apparmor=no])
  PKG_CHECK_MODULES([APPARMOR_2_10], [libapparmor >= 2.10],
                    [have_apparmor_2_10=yes], [have_apparmor_2_10=no])

  AS_IF([test x$enable_apparmor = xauto && test x$have_apparmor = xno],
        [AC_MSG_WARN([Sufficiently new AppArmor library not found])])
  AS_IF([test x$enable_apparmor != xauto && test x$have_apparmor = xno],
        [AC_MSG_ERROR([AppArmor explicitly required, and AppArmor library not found])])
  ])

AS_IF([test x$have_apparmor = xyes],
      [AC_DEFINE([HAVE_APPARMOR], [1], [AppArmor Support])])
AS_IF([test x$have_apparmor_2_10 = xyes],
      [AC_DEFINE([HAVE_APPARMOR_2_10], [1],
                 [Define if libapparmor is version 2.10 or later])])

# inotify checks
if test x$enable_inotify = xno ; then
    have_inotify=no;
else
    AC_CHECK_HEADERS(sys/inotify.h, have_inotify=yes, have_inotify=no)
fi

dnl check if inotify backend is enabled
if test x$have_inotify = xyes; then
   AC_DEFINE(DBUS_BUS_ENABLE_INOTIFY,1,[Use inotify])
fi

AM_CONDITIONAL(DBUS_BUS_ENABLE_INOTIFY, test x$have_inotify = xyes)

# For simplicity, we require the userland API for epoll_create1 at
# compile-time (glibc 2.9), but we'll run on kernels that turn out
# not to have it at runtime.
AC_ARG_ENABLE([epoll],
              [AS_HELP_STRING([--enable-epoll],[use epoll(4) on Linux])],
              [enable_epoll=$enableval], [enable_epoll=auto])
if test x$enable_epoll = xno; then
    have_linux_epoll=no
else
    AC_MSG_CHECKING([for Linux epoll(4)])
    AC_LINK_IFELSE([AC_LANG_PROGRAM(
        [
        #ifndef __linux__
        #error This is not Linux
        #endif
        #include <sys/epoll.h>
        ],
        [epoll_create1 (EPOLL_CLOEXEC);])],
        [have_linux_epoll=yes],
        [have_linux_epoll=no])
    AC_MSG_RESULT([$have_linux_epoll])
fi
if test x$enable_epoll,$have_linux_epoll = xyes,no; then
    AC_MSG_ERROR([epoll support explicitly enabled but not available])
fi
if test x$have_linux_epoll = xyes; then
  AC_DEFINE([DBUS_HAVE_LINUX_EPOLL], 1, [Define to use epoll(4) on Linux])
fi
AM_CONDITIONAL([DBUS_HAVE_LINUX_EPOLL], [test x$have_linux_epoll = xyes])

# kqueue checks
if test x$enable_kqueue = xno ; then
    have_kqueue=no
else
    have_kqueue=yes
    AC_CHECK_HEADER(sys/event.h, , have_kqueue=no)
    AC_CHECK_FUNC(kqueue, , have_kqueue=no)

    if test x$enable_kqueue = xyes -a x$have_kqueue = xno; then
        AC_MSG_ERROR(kqueue support explicitly enabled but not available)
    fi
fi

dnl check if kqueue backend is enabled
if test x$have_kqueue = xyes; then
   AC_DEFINE(DBUS_BUS_ENABLE_KQUEUE,1,[Use kqueue])
fi

AM_CONDITIONAL(DBUS_BUS_ENABLE_KQUEUE, test x$have_kqueue = xyes)

# launchd checks
if test x$enable_launchd = xno ; then
    have_launchd=no
else
    have_launchd=yes
    AC_CHECK_HEADER([launch.h], , have_launchd=no)
    AC_PATH_PROG([LAUNCHCTL], [launchctl])
    if test "x$LAUNCHCTL" = "x"; then
        have_launchd=no
    fi

    if test x$enable_launchd = xyes && test x$have_launchd = xno ; then
        AC_MSG_ERROR([launchd support explicitly enabled but not available])
    fi
fi

dnl check if launchd is enabled
if test x$have_launchd = xyes; then
    AC_DEFINE(DBUS_ENABLE_LAUNCHD,1,[Use launchd autolaunch])
fi

AM_CONDITIONAL(DBUS_ENABLE_LAUNCHD, test x$have_launchd = xyes)

#### Directory to place launchd agent file
if test "x$with_launchd_agent_dir" = "x"; then
   LAUNCHD_AGENT_DIR="/Library/LaunchAgents"
else
   LAUNCHD_AGENT_DIR="$with_launchd_agent_dir"
fi

AC_SUBST(LAUNCHD_AGENT_DIR)

dnl console owner file
if test x$enable_console_owner_file = xno ; then
    have_console_owner_file=no;
else
    case $host_os in
    solaris*)
        have_console_owner_file=yes;
        AC_DEFINE(HAVE_CONSOLE_OWNER_FILE,1,[Have console owner file])
        ;;
    *)
        have_console_owner_file=no;;
    esac
fi

AM_CONDITIONAL(HAVE_CONSOLE_OWNER_FILE, test x$have_console_owner_file = xyes)

dnl systemd detection
if test x$enable_systemd = xno ; then
    have_systemd=no;
else
    PKG_CHECK_MODULES([SYSTEMD],
        [libsystemd >= 209],
        [have_systemd=yes],
        [PKG_CHECK_MODULES([SYSTEMD],
            [libsystemd-login >= 32, libsystemd-daemon >= 32, libsystemd-journal >= 32],
            [have_systemd=yes],
            [have_systemd=no])])
fi

if test x$have_systemd = xyes; then
    AC_DEFINE(HAVE_SYSTEMD,1,[Have systemd])
fi

if test x$enable_systemd = xyes -a x$have_systemd != xyes ; then
    AC_MSG_ERROR([Explicitly requested systemd support, but systemd not found])
fi

AS_IF([test "x$enable_traditional_activation" = xyes],
    AC_DEFINE(ENABLE_TRADITIONAL_ACTIVATION,[1], [Enable traditional activation without using systemd])
    AS_IF([test "x$enable_systemd" = xno],
       AC_MSG_WARN([Traditional activation and systemd activation are both disabled, so service activation (automatically starting services that receive messages) will not work])))

AM_CONDITIONAL(ENABLE_TRADITIONAL_ACTIVATION, test x$enable_traditional_activation = xyes)

# If not found in $PATH, we might still have systemd and systemctl at runtime
# (perhaps dbus is being compiled in a minimal chroot with no systemd).
# Assume the upstream-recommended location. Distributors with split /usr
# can override this with ./configure SYSTEMCTL=/bin/systemctl
AC_PATH_PROG([SYSTEMCTL], [systemctl], [/usr/bin/systemctl])

# libaudit detection
if test x$enable_libaudit = xno ; then
    have_libaudit=no;
else
    # See if we have audit daemon & capabilities library
    AC_CHECK_LIB(audit, audit_log_user_avc_message,
                 have_libaudit=yes, have_libaudit=no)
    if test x$have_libaudit = xyes ; then
        AC_CHECK_LIB(cap-ng, capng_clear,
                 have_libaudit=yes, have_libaudit=no)
    fi
fi

AM_CONDITIONAL(HAVE_LIBAUDIT, test x$have_libaudit = xyes)

if test x$have_libaudit = xyes ; then
    SELINUX_LIBS="$SELINUX_LIBS -laudit -lcap-ng"
    AC_DEFINE(HAVE_LIBAUDIT,1,[audit daemon SELinux support])
fi

AC_SUBST([SELINUX_LIBS])

# Check for ADT API (Solaris Basic Security Mode auditing)
AC_MSG_CHECKING(for ADT API)
AC_COMPILE_IFELSE([AC_LANG_PROGRAM([[
#include <bsm/adt.h>
adt_user_context = ADT_USER;
]], [[]])], [ check_adt_audit=yes ], [ check_adt_audit=no ])

if test ${check_adt_audit} = yes
then
   AC_DEFINE([HAVE_ADT], [], [Adt audit API])
   ADT_LIBS="-lbsm"
   LIBS="-lbsm $LIBS"
   AC_MSG_RESULT(yes)
else
   AC_MSG_RESULT(no)
fi
AC_SUBST([ADT_LIBS])

# Check for SCM_RIGHTS
AC_MSG_CHECKING([for SCM_RIGHTS])
AC_COMPILE_IFELSE([AC_LANG_PROGRAM([[
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/un.h>
static int x = SCM_RIGHTS;
]], [[]])],
[ AC_MSG_RESULT([supported])
  AC_DEFINE([HAVE_UNIX_FD_PASSING], [1], [Supports sending UNIX file descriptors]) ],
[ AC_MSG_RESULT([not supported]) ])

NETWORK_libs=
if test x$dbus_win = xyes ; then
  if test x$dbus_wince = xyes ; then
    NETWORK_libs="-lws2"
  else
    NETWORK_libs="-lws2_32 -liphlpapi -ldbghelp"
  fi
fi

AC_SUBST([NETWORK_libs])

AC_ARG_WITH([valgrind],
  [AS_HELP_STRING([--with-valgrind],
     [Add instrumentation to help valgrind to understand our allocator])],
  [],
  [with_valgrind=no])

AS_IF([test "x$with_valgrind" = xauto],
  [PKG_CHECK_EXISTS([valgrind >= 3.6],
    [with_valgrind=yes], [with_valgrind=no])])

if test x$with_valgrind != xno; then
  PKG_CHECK_MODULES([VALGRIND], [valgrind >= 3.6])
  AC_DEFINE([WITH_VALGRIND], [1], [Define to add Valgrind instrumentation])
fi

#### Set up final flags
LIBDBUS_LIBS="$THREAD_LIBS $NETWORK_libs $SYSTEMD_LIBS"
AC_SUBST([LIBDBUS_LIBS])

### X11 detection
DBUS_X_LIBS=
DBUS_X_CFLAGS=

AC_ARG_WITH([x],
  [AS_HELP_STRING([--without-x], [build without X11 support])],
  [], [with_x=auto])

AC_ARG_ENABLE([x11-autolaunch],
  AS_HELP_STRING([--enable-x11-autolaunch], [build with X11 auto-launch support]),
  [], [enable_x11_autolaunch=auto])

AS_IF([test "x$dbus_win" = xyes], [
    AS_IF([test "x$enable_x11_autolaunch" = xyes], [
        AC_MSG_ERROR([X11 auto-launch is not supported on Windows])
    ])

    enable_x11_autolaunch=no
    have_x11=no
], [test "x$with_x" != xno], [
    PKG_CHECK_MODULES([X], [x11],
            [AC_DEFINE([HAVE_X11], [1], [Define to 1 if you have X11 library])
	     have_x11=yes
	     DBUS_X_LIBS="$X_LIBS"
	     DBUS_X_CFLAGS="$X_CFLAGS"
            ], [
             AS_IF([test "x$with_x" = xyes],
                [AC_MSG_ERROR([Couldn't find X11, tried with pkg-config.])],
                [AC_MSG_WARN([Couldn't find X11, tried with pkg-config.])]
             )
             have_x11=no
            ])
], [
    have_x11=no
    AS_IF([test "x$enable_x11_autolaunch" = "xyes"], [
      AC_MSG_ERROR([--enable-x11-autolaunch and --without-x are not compatible])
    ])
])

if test "x$enable_x11_autolaunch,$have_x11" = xyes,no; then
    AC_MSG_ERROR([X11 auto-launch requires X headers/libraries])
else
    # move from "auto" to "yes" or "no" if necessary
    if test "x$enable_x11_autolaunch" != xno; then
        enable_x11_autolaunch="$have_x11"
    fi
fi

if test "x$have_x11" = xyes ; then
   AC_DEFINE([DBUS_BUILD_X11], [1], [Define to build X11 functionality])
fi

if test "x$enable_x11_autolaunch" = xyes ; then
   AC_DEFINE([DBUS_ENABLE_X11_AUTOLAUNCH], [1], [Define to enable X11 auto-launch])
fi
AM_CONDITIONAL([DBUS_ENABLE_X11_AUTOLAUNCH],
  [test "x$enable_x11_autolaunch" = xyes])

AC_SUBST([DBUS_X_CFLAGS])
AC_SUBST([DBUS_X_LIBS])

# We're treating -fno-common like a warning: it makes the linker more
# strict, because on some systems the linker is *always* this strict
TEST_CFLAGS="$TEST_CFLAGS -fno-common"

AS_IF([test "x$enable_ansi" = "xyes"],[
    TEST_CFLAGS="$TEST_CFLAGS -ansi -pedantic"
    AC_DEFINE([_POSIX_C_SOURCE],[199309L],[Define to enable POSIX features])
    AC_DEFINE([_BSD_SOURCE],[1],[Define to enable BSD features])
  ])

dnl We are only calling this for its side-effect of setting up
dnl --enable-compile-warnings; the WARN_CFLAGS, etc. are ignored,
dnl to work around https://github.com/peti/autoconf-archive/pull/96
AX_COMPILER_FLAGS([], [], [$disable_developer])

dnl Work around https://github.com/peti/autoconf-archive/pull/96 by using
dnl a non-default variable name here (in particular there is no way to tell
dnl AX_COMPILER_FLAGS to not use WARN_CXXFLAGS)
AX_COMPILER_FLAGS_CFLAGS([EXTRA_CFLAGS],
                         [$disable_developer],
                         [$TEST_CFLAGS],
                         [-Wchar-subscripts \
                          -Wfloat-equal \
                          -Wpointer-sign \
                          $DISABLE_WARNINGS])
dnl cc1plus: warning: command line option ‘-Wpointer-sign’ is valid for
dnl C/ObjC but not for C++
AX_COMPILER_FLAGS_CXXFLAGS([EXTRA_CXXFLAGS],
                           [$disable_developer],
                           [],
                           [-Wchar-subscripts \
                            -Wfloat-equal \
                            $DISABLE_WARNINGS])
AX_COMPILER_FLAGS_LDFLAGS([EXTRA_LDFLAGS],
                          [$disable_developer])

dnl TODO: In principle we should put EXTRA_CFLAGS in each Makefile.am like
dnl telepathy-glib does, since CFLAGS is meant to be reserved for the user...
dnl but prepending to CFLAGS (so the user can override it with later CFLAGS)
dnl is the next best thing.
CFLAGS="$EXTRA_CFLAGS $CFLAGS"
CXXFLAGS="$EXTRA_CXXFLAGS $CXXFLAGS"
LDFLAGS="$EXTRA_LDFLAGS $LDFLAGS"

AC_ARG_VAR([SANITIZE_CFLAGS],
           [Extra CFLAGS for modules that are instrumented for error-checking])

case $host_os in
    solaris*)
        # Solaris' C library apparently needs these runes to be threadsafe...
        CFLAGS="$CFLAGS -D_POSIX_PTHREAD_SEMANTICS -D_REENTRANT"
        # ... this opt-in to get sockaddr_in6 and sockaddr_storage...
        CFLAGS="$CFLAGS -D__EXTENSIONS__"
        # ... and this opt-in to get file descriptor passing support
        CFLAGS="$CFLAGS -D_XOPEN_SOURCE=500"
        ;;
esac

### Detect if ld supports --version-script

gl_LD_VERSION_SCRIPT
AM_CONDITIONAL([HAVE_LD_VERSION_SCRIPT],
               [test "x$have_ld_version_script" = xyes])
### Doxygen Documentation

AC_PATH_PROG(DOXYGEN, doxygen, no)

AC_MSG_CHECKING([whether to build Doxygen documentation])

if test x$DOXYGEN = xno ; then
    have_doxygen=no
else
    have_doxygen=yes
fi

if test x$enable_doxygen_docs = xauto ; then
    if test x$have_doxygen = xno ; then
        enable_doxygen_docs=no
    else
        enable_doxygen_docs=yes
    fi
fi

if test x$enable_doxygen_docs = xyes; then
    if test x$have_doxygen = xno; then
	AC_MSG_ERROR([Building Doxygen docs explicitly required, but Doxygen not found])
    fi
fi

if test x$dbus_win = xno; then
    DBUS_GENERATE_MAN=YES
else
    DBUS_GENERATE_MAN=NO
fi
AC_SUBST([DBUS_GENERATE_MAN])

AM_CONDITIONAL(DBUS_DOXYGEN_DOCS_ENABLED, test x$enable_doxygen_docs = xyes)
AC_MSG_RESULT($enable_doxygen_docs)

AC_ARG_WITH([qchdir],
            AS_HELP_STRING([--with-qchdir=DIR], [Directory for installing Qt help file]),
            [qchdir=$withval],
            [qchdir="$docdir"])
AC_SUBST([qchdir])

AC_ARG_ENABLE([qt-help],
              [AS_HELP_STRING([--enable-qt-help=auto|yes|no], [Build Qt help documentation])],
              [],
              [enable_qt_help=auto])
AS_IF([test "x$enable_qt_help" != xno],
      [AC_CHECK_PROGS([QHELPGENERATOR], [qhelpgenerator qhelpgenerator-qt5])],
      [QHELPGENERATOR=])
AS_IF([test "x$QHELPGENERATOR" = x && test "x$enable_qt_help" != xno && test "x$enable_qt_help" != xauto],
      [AC_MSG_ERROR([Building of Qt help requested, but qhelpgenerator not found])])

AC_MSG_CHECKING([whether to build Qt help documentation])
AS_IF([test "x$enable_doxygen_docs" = xyes && test "x$QHELPGENERATOR" != x], [
    enable_qthelp_docs=yes
    DOXYGEN_GENERATE_QHP=YES
    DOXYGEN_QHG_LOCATION="$QHELPGENERATOR"
    DOXYGEN_QCH_FILE="$(pwd)/doc/api/qch/dbus-$VERSION.qch"
  ], [
    DOXYGEN_GENERATE_QHP=NO
    enable_qthelp_docs=no
  ])
AC_SUBST([DOXYGEN_GENERATE_QHP])
AC_SUBST([DOXYGEN_QHG_LOCATION])
AC_SUBST([DOXYGEN_QCH_FILE])

AM_CONDITIONAL([DBUS_QTHELP_DOCS_ENABLED], [test "x$enable_qthelp_docs" = xyes])
AC_MSG_RESULT($enable_qthelp_docs)

AC_CHECK_PROGS([XSLTPROC], [xsltproc])
AM_CONDITIONAL([DBUS_HAVE_XSLTPROC], [test "x$XSLTPROC" != "x"])

### Ducktype/Yelp documentation

AC_PATH_PROG([DUCKTYPE],[ducktype],[no])
AC_PATH_PROG([YELP_BUILD],[yelp-build],[no])

AC_MSG_CHECKING([whether to build Ducktype documentation])

AS_IF([test "$DUCKTYPE" = "no"],[have_ducktype=no],[have_ducktype=yes])
AS_IF([test "$YELP_BUILD" = "no"],[have_yelp_build=no],[have_yelp_build=yes])

AS_IF([test "$enable_ducktype_docs" = "auto"],[
    AS_IF([test "$have_ducktype" = "no" || test "$have_yelp_build" = "no"],[
        enable_ducktype_docs=no
    ],[
        enable_ducktype_docs=yes
    ])
])

AS_IF([test "$enable_ducktype_docs" = "yes"],[
    AS_IF([test "$have_ducktype" = "no"],[
        AC_MSG_ERROR([Building Ducktype docs explicitly required, but ducktype not found])
    ])
    AS_IF([test "$have_yelp_build" = "no"],[
        AC_MSG_ERROR([Building Ducktype docs explicitly required, but yelp-build not found])
    ])
])

AM_CONDITIONAL([DBUS_DUCKTYPE_DOCS_ENABLED],[test "$enable_ducktype_docs" = "yes"])
AC_MSG_RESULT([$enable_ducktype_docs])

### XML Documentation

AC_PATH_PROG(XMLTO, xmlto, no)

AC_MSG_CHECKING([whether to build XML documentation])

if test x$XMLTO = xno ; then
    have_xmlto=no
else
    have_xmlto=yes
fi

if test x$enable_xml_docs = xauto ; then
    if test x$have_xmlto = xno ; then
        enable_xml_docs=no
    else
        enable_xml_docs=yes
    fi
fi

if test x$enable_xml_docs = xyes; then
    if test x$have_xmlto = xno; then
	AC_MSG_ERROR([Building XML docs explicitly required, but xmlto not found])
    fi
fi

AM_CONDITIONAL(DBUS_XML_DOCS_ENABLED, test x$enable_xml_docs = xyes)
AC_MSG_RESULT($enable_xml_docs)

AM_CONDITIONAL(DBUS_CAN_UPLOAD_DOCS,
    [test x$enable_doxygen_docs = xyes && test x$enable_xml_docs = xyes &&
     test x$enable_ducktype_docs = xyes])

# Autoconf 2.70 will support this, and many distros patch this option in,
# but Autoconf 2.70 hasn't actually been released yet.
AS_IF([test -z "${runstatedir}"], [runstatedir='${localstatedir}/run'])
AC_SUBST([runstatedir])

#### Have to go $localstatedir->$prefix/var->/usr/local/var

#### find the actual value for $prefix that we'll end up with
##   (I know this is broken and should be done in the Makefile, but
##    that's a major pain and almost nobody actually seems to care)
AS_AC_EXPAND(EXPANDED_PREFIX, "$prefix")
AS_AC_EXPAND(EXPANDED_LOCALSTATEDIR, "$localstatedir")
AS_AC_EXPAND(EXPANDED_SYSCONFDIR, "$sysconfdir")
AS_AC_EXPAND(EXPANDED_BINDIR, "$bindir")
AS_AC_EXPAND(EXPANDED_LIBDIR, "$libdir")
AS_AC_EXPAND(EXPANDED_LIBEXECDIR, "$libexecdir")
AS_AC_EXPAND(EXPANDED_DATADIR, "$datadir")
AS_AC_EXPAND(EXPANDED_RUNSTATEDIR, "$runstatedir")

##### systemd unit files
AC_ARG_WITH([systemdsystemunitdir],
AS_HELP_STRING([--with-systemdsystemunitdir=DIR], [Directory for systemd service files]),
    [],
    [
    PKG_CHECK_EXISTS([systemd],
      [with_systemdsystemunitdir=$($PKG_CONFIG --variable=systemdsystemunitdir systemd)],
      [with_systemdsystemunitdir=no])
    ])
if test "x$with_systemdsystemunitdir" != xno; then
   AC_SUBST([systemdsystemunitdir], [$with_systemdsystemunitdir])
fi
AM_CONDITIONAL(HAVE_SYSTEMD, [test "x$have_systemd" != "xno" -a -n "$with_systemdsystemunitdir" -a "x$with_systemdsystemunitdir" != xno ])

AC_ARG_WITH([systemduserunitdir],
AS_HELP_STRING([--with-systemduserunitdir=DIR], [Directory for systemd user service files]),
    [],
    [
    PKG_CHECK_EXISTS([systemd],
      [with_systemduserunitdir=$($PKG_CONFIG --variable=systemduserunitdir systemd)],
      [with_systemduserunitdir='${libdir}/systemd/user'])
    ])
AC_SUBST([systemduserunitdir], [$with_systemduserunitdir])

##### Set up location for system bus socket
if ! test -z "$with_system_socket"; then
   DBUS_SYSTEM_SOCKET=$with_system_socket
else
   # We don't use runstatedir for this (yet?), because /var/run has been the
   # interoperable system bus socket for 10+ years.
   # See https://bugs.freedesktop.org/show_bug.cgi?id=101628
   DBUS_SYSTEM_SOCKET=${EXPANDED_LOCALSTATEDIR}/run/dbus/system_bus_socket
fi

AC_SUBST(DBUS_SYSTEM_SOCKET)
AC_DEFINE_UNQUOTED(DBUS_SYSTEM_SOCKET,"$DBUS_SYSTEM_SOCKET",[The name of the socket the system bus listens on by default])

## System bus only listens on local domain sockets, and never
## on an abstract socket (so only root can create the socket).
##
## This won't work on Windows. It's not meant to - the system bus is
## meaningless on Windows anyway.
##
## This has to be suitable for hard-coding in client libraries as well as
## in the dbus-daemon's configuration, so it has to be valid to listen on
## and also to connect to. If this ever changes, it'll need to be split into
## two variables, one for the listening address and one for the connecting
## address.
DBUS_SYSTEM_BUS_DEFAULT_ADDRESS="unix:path=$DBUS_SYSTEM_SOCKET"
AC_SUBST(DBUS_SYSTEM_BUS_DEFAULT_ADDRESS)
AC_DEFINE_UNQUOTED(DBUS_SYSTEM_BUS_DEFAULT_ADDRESS, "$DBUS_SYSTEM_BUS_DEFAULT_ADDRESS",[The default D-Bus address of the system bus])

#### Set up the pid file
if ! test -z "$with_system_pid_file"; then
   DBUS_SYSTEM_PID_FILE=$with_system_pid_file
else
   DBUS_SYSTEM_PID_FILE="${EXPANDED_RUNSTATEDIR}/dbus/pid"
fi

AC_SUBST(DBUS_SYSTEM_PID_FILE)

#### Directory to check for console ownership
AS_IF([test -n "$with_console_auth_dir" && test "x$with_console_auth_dir" != xno],
    [AC_DEFINE_UNQUOTED([DBUS_CONSOLE_AUTH_DIR], ["$with_console_auth_dir"],
        [Directory to check for pam_console/pam_foreground flag files, or undefined])],
    [with_console_auth_dir=])

#### File to check for console ownership
if test x$have_console_owner_file = xyes; then
   if ! test -z "$with_console_owner_file"; then
      DBUS_CONSOLE_OWNER_FILE=$with_console_owner_file
   else
      DBUS_CONSOLE_OWNER_FILE=/dev/console
   fi
else
  DBUS_CONSOLE_OWNER_FILE=
fi

AC_SUBST(DBUS_CONSOLE_OWNER_FILE)
AC_DEFINE_UNQUOTED(DBUS_CONSOLE_OWNER_FILE, "$DBUS_CONSOLE_OWNER_FILE", [File to check for console ownerhip])

#### User to start the system bus as
if test -z "$with_dbus_user" ; then
    DBUS_USER=messagebus
else
    DBUS_USER=$with_dbus_user
fi
AC_SUBST(DBUS_USER)
AC_DEFINE_UNQUOTED(DBUS_USER,"$DBUS_USER", [User for running the system BUS daemon])

#### User for regression tests
AS_IF([test -z "$with_test_user"], [with_test_user=nobody])
DBUS_TEST_USER="$with_test_user"
AC_SUBST([DBUS_TEST_USER])
AC_DEFINE_UNQUOTED([DBUS_TEST_USER], ["$DBUS_TEST_USER"],
  [Unprivileged user used in some regression tests])

#### Prefix to install into
DBUS_PREFIX=$EXPANDED_PREFIX
AC_SUBST(DBUS_PREFIX)
AC_DEFINE_UNQUOTED(DBUS_PREFIX,"$DBUS_PREFIX", [Prefix for installing DBUS])

#### Direcotry to install data files into
DBUS_DATADIR=$EXPANDED_DATADIR
AC_SUBST(DBUS_DATADIR)
AC_DEFINE_UNQUOTED(DBUS_DATADIR,"$DBUS_DATADIR", [Directory for installing DBUS data files])

#### Directory to install dbus-daemon
if test -z "$with_dbus_daemondir" ; then
    DBUS_DAEMONDIR=$EXPANDED_BINDIR
    dbus_daemondir='${bindir}'
else
    DBUS_DAEMONDIR=$with_dbus_daemondir
    dbus_daemondir=$with_dbus_daemondir
fi
AC_SUBST(DBUS_DAEMONDIR)
AC_SUBST(dbus_daemondir)
AC_DEFINE_UNQUOTED(DBUS_DAEMONDIR,"$DBUS_DAEMONDIR", [Directory for installing the DBUS daemon])

#### Directory to install the other binaries
DBUS_BINDIR="$EXPANDED_BINDIR"
AC_SUBST(DBUS_BINDIR)
AC_DEFINE_UNQUOTED(DBUS_BINDIR,"$DBUS_BINDIR", [Directory for installing the binaries])

#### Directory to install the libexec binaries
DBUS_LIBEXECDIR="$EXPANDED_LIBEXECDIR"
AC_SUBST(DBUS_LIBEXECDIR)
AC_DEFINE_UNQUOTED(DBUS_LIBEXECDIR,"$DBUS_LIBEXECDIR", [Directory for installing the libexec binaries])

AC_ARG_ENABLE([relocation],
  [AS_HELP_STRING([--enable-relocation[=yes/no/auto]],
    [Make pkg-config metadata relocatable [default=auto]])],
  [], [enable_relocation=auto])

can_relocate=yes

AS_CASE(["${exec_prefix}"],
  ['NONE'|'${prefix}'|"${prefix}"],
    [:],
  [*],
    [
      can_relocate=no
      # If the user said --enable-relocation but we can't do it, error out
      AS_IF([test "x$enable_relocation" = xyes],
        [AC_MSG_ERROR([Relocatable pkg-config metadata requires --exec-prefix='\${prefix}', not ${exec_prefix}])])
    ])

AS_CASE(["${libdir}"],
  ['${prefix}/lib'|'${prefix}/lib64'|'${exec_prefix}/lib'|'${exec_prefix}/lib64'|"${prefix}/lib"|"${exec_prefix}/lib"|"${prefix}/lib64"|"${exec_prefix}/lib64"],
    [:],
  [*],
    [
      can_relocate=no
      # If the user said --enable-relocation but we can't do it, error out
      AS_IF([test "x$enable_relocation" = xyes],
        [AC_MSG_ERROR([Relocatable pkg-config metadata requires default libdir, not ${libdir}])])
    ])

# By default, on Windows we are relocatable if possible
AS_IF([test "x$enable_relocation" = xauto && test "x$dbus_win" = xyes],
  [enable_relocation="$can_relocate"])

# By default, on non-Windows we are not relocatable because it can interfere
# with pkg-config's ability to filter out system include directories,
# resulting in linking an outdated system-wide library in preference to a
# newer version installed elsewhere
AS_IF([test "x$enable_relocation" = xauto],
  [enable_relocation="no"])


AS_IF([test "x$enable_relocation" = xyes],
  [AC_SUBST([pkgconfig_prefix], ['${pcfiledir}/../..'])],
  [AC_SUBST([pkgconfig_prefix], ['${original_prefix}'])])

#### Directory to source sysconfdir configuration from

# On Windows this is relative to where we put the bus setup, in
# ${datadir}/dbus-1. For simplicity, we only do this if
# ${sysconfdir} = ${prefix}/etc and ${datadir} = ${prefix}/share.
#
# On Unix, or on Windows with weird install layouts, it's the absolute path.
AS_IF([test "${dbus_win}" = yes && \
       test "$EXPANDED_SYSCONFDIR" = "$EXPANDED_PREFIX/etc" && \
       test "$EXPANDED_DATADIR" = "$EXPANDED_PREFIX/share"],
      [SYSCONFDIR_FROM_PKGDATADIR="../../etc"
       DATADIR_FROM_PKGSYSCONFDIR="../../share"],
      [SYSCONFDIR_FROM_PKGDATADIR="$EXPANDED_SYSCONFDIR"
       DATADIR_FROM_PKGSYSCONFDIR="$EXPANDED_DATADIR"])
AC_SUBST([SYSCONFDIR_FROM_PKGDATADIR])
AC_SUBST([DATADIR_FROM_PKGSYSCONFDIR])

#### Tell tests where to find certain stuff in builddir

DBUS_PWD=`pwd`
# Useful in a cross-compilation environment, where the tests are run on the host system.
AC_ARG_WITH(dbus-test-dir, AS_HELP_STRING([--with-dbus-test-dir=[dirname]],[path where the tests tools are available]),
			   DBUS_PWD=$withval)

DBUS_TEST_EXEC="$DBUS_PWD/test"
DBUS_TEST_DATA="$DBUS_PWD/test/data"

AC_SUBST([DBUS_TEST_DATA])
AC_SUBST([DBUS_TEST_EXEC])

AC_DEFINE_UNQUOTED([DBUS_EXEEXT], ["$EXEEXT"],
                   [Extension for executables, typically empty or .exe])

## Export the non-setuid external helper
TEST_LAUNCH_HELPER_BINARY="$DBUS_TEST_EXEC/dbus-daemon-launch-helper-for-tests$EXEEXT"
AC_SUBST(TEST_LAUNCH_HELPER_BINARY)
AC_DEFINE_UNQUOTED(DBUS_TEST_LAUNCH_HELPER_BINARY, "$TEST_LAUNCH_HELPER_BINARY",
                   [Full path to the launch helper test program in the builddir])

#### Find socket directories
if ! test -z "$TMPDIR" ; then
   DEFAULT_SOCKET_DIR=$TMPDIR
elif ! test -z "$TEMP" ; then
   DEFAULT_SOCKET_DIR=$TEMP
elif ! test -z "$TMP" ; then
   DEFAULT_SOCKET_DIR=$TMP
else
   DEFAULT_SOCKET_DIR=/tmp
fi

DEFAULT_SOCKET_DIR=`echo $DEFAULT_SOCKET_DIR | sed 's/+/%2B/g'`

if ! test -z "$with_test_socket_dir" ; then
   TEST_SOCKET_DIR="$with_test_socket_dir"
else
   TEST_SOCKET_DIR=$DEFAULT_SOCKET_DIR
fi
AC_SUBST(TEST_SOCKET_DIR)
AC_DEFINE_UNQUOTED(DBUS_TEST_SOCKET_DIR, "$TEST_SOCKET_DIR", [Where to put test sockets])

if test "x$dbus_unix" = xyes; then
  TEST_LISTEN="unix:tmpdir=$TEST_SOCKET_DIR"
else
  TEST_LISTEN="tcp:host=localhost"
fi
AC_SUBST([TEST_LISTEN])
AC_DEFINE_UNQUOTED([TEST_LISTEN], ["$TEST_LISTEN"],
  [Listening address for regression tests])

if ! test -z "$with_session_socket_dir" ; then
   DBUS_SESSION_SOCKET_DIR="$with_session_socket_dir"
else
   DBUS_SESSION_SOCKET_DIR=$DEFAULT_SOCKET_DIR
fi
AC_DEFINE_UNQUOTED(DBUS_SESSION_SOCKET_DIR, "$DBUS_SESSION_SOCKET_DIR", [Where per-session bus puts its sockets])
AC_SUBST(DBUS_SESSION_SOCKET_DIR)

# This must be a listening address. It doesn't necessarily need to be an
# address you can connect to - it can be something vague like
# "nonce-tcp:".
#
# The default varies by platform.
AC_ARG_WITH([dbus_session_bus_listen_address],
            AS_HELP_STRING([--with-dbus-session-bus-listen-address=[ADDRESS]],
                           [default address for a session bus to listen on (see configure.ac)]),
            [with_dbus_session_bus_listen_address=$withval],
            [with_dbus_session_bus_listen_address=])

if test "x$with_dbus_session_bus_listen_address" != "x"; then
        # the user specified something, trust them
        DBUS_SESSION_BUS_LISTEN_ADDRESS="$with_dbus_session_bus_listen_address"
elif test x$dbus_win = xyes; then
        # On Windows, you can (and should) listen on autolaunch addresses,
        # because autolaunching is different.
        # See https://bugs.freedesktop.org/show_bug.cgi?id=38201
        DBUS_SESSION_BUS_LISTEN_ADDRESS="autolaunch:"
elif test x$have_launchd = xyes; then
        # Mac OS X default is to use launchd
        DBUS_SESSION_BUS_LISTEN_ADDRESS="launchd:env=DBUS_LAUNCHD_SESSION_BUS_SOCKET"
else
        # The default on all other Unix platforms (notably Linux)
        # is to create a randomly named socket in /tmp or similar
        DBUS_SESSION_BUS_LISTEN_ADDRESS="unix:tmpdir=$DBUS_SESSION_SOCKET_DIR"
fi
AC_SUBST([DBUS_SESSION_BUS_LISTEN_ADDRESS])

# This must be an address you can connect to. It doesn't necessarily
# need to be an address you can listen on - it can be "autolaunch:",
# even on Unix.
#
# The default varies by platform.
AC_ARG_WITH([dbus_session_bus_connect_address],
            AS_HELP_STRING([--with-dbus-session-bus-connect-address=[ADDRESS]],
                           [fallback address for a session bus client to connect to (see configure.ac)]),
            [with_dbus_session_bus_connect_address=$withval],
            [with_dbus_session_bus_connect_address=])

if test "x$with_dbus_session_bus_connect_address" != "x"; then
        # the user specified something, trust them
        DBUS_SESSION_BUS_CONNECT_ADDRESS="$with_dbus_session_bus_connect_address"
elif test x$dbus_win = xyes; then
        # Windows autolaunching is a bit different; leaving it in its own
        # branch of the conditional because the default might conceivably
        # change (see #38201)
        DBUS_SESSION_BUS_CONNECT_ADDRESS="autolaunch:"
else
        # The default on all other Unix platforms (notably Linux)
        # is to use auto-launching - this works a bit differently on Mac OS X
        # but comes out basically the same in the end
        DBUS_SESSION_BUS_CONNECT_ADDRESS="autolaunch:"
fi
AC_SUBST([DBUS_SESSION_BUS_CONNECT_ADDRESS])
AC_DEFINE_UNQUOTED([DBUS_SESSION_BUS_CONNECT_ADDRESS],
  ["$DBUS_SESSION_BUS_CONNECT_ADDRESS"],
  [Fallback address for session bus clients])

# darwin needs this to initialize the environment
AC_CHECK_FUNC(_NSGetEnviron, [AC_DEFINE(HAVE_NSGETENVIRON, 1, [Define if your system needs _NSGetEnviron to set up the environment])])
AH_VERBATIM(_DARWIN_ENVIRON,
[
#if defined(HAVE_NSGETENVIRON) && defined(HAVE_CRT_EXTERNS_H)
# include <sys/time.h>
# include <crt_externs.h>
# define environ (*_NSGetEnviron())
#endif
])

AH_VERBATIM([USE_MINGW_ANSI_STDIO],
[
/* On Windows, we expect to be using msvcrt.dll-compatible printf
 * (%I64u instead of %llu) unless otherwise specified */
#ifndef __USE_MINGW_ANSI_STDIO
#   define __USE_MINGW_ANSI_STDIO 0
#endif
])

AC_ARG_ENABLE([stats],
  [AS_HELP_STRING([--disable-stats],
    [disable bus daemon usage statistics])],
  [], [enable_stats=yes])
if test "x$enable_stats" = xyes; then
  AC_DEFINE([DBUS_ENABLE_STATS], [1],
    [Define to enable bus daemon usage statistics])
fi

AC_ARG_ENABLE([user-session],
  [AS_HELP_STRING([--enable-user-session],
    [enable user-session semantics for session bus under systemd])],
  [], [enable_user_session=no])
AM_CONDITIONAL([DBUS_ENABLE_USER_SESSION],
  [test "x$enable_user_session" = xyes])

AC_CONFIG_FILES([
Doxyfile
dbus/Version
dbus/versioninfo.rc
dbus/dbus-arch-deps.h
bus/system.conf
bus/session.conf
bus/legacy-config/system.conf
bus/legacy-config/session.conf
bus/example-system-enable-stats.conf
bus/example-session-disable-stats.conf
bus/org.freedesktop.dbus-session.plist
bus/dbus.service
bus/dbus.socket
bus/systemd-user/dbus.service
bus/systemd-user/dbus.socket
bus/sysusers.d/dbus.conf
bus/tmpfiles.d/dbus.conf
Makefile
dbus/Makefile
bus/Makefile
tools/Makefile
test/Makefile
test/name-test/Makefile
doc/Makefile
doc/dbus-cleanup-sockets.1.xml
doc/dbus-daemon.1.xml
doc/dbus-launch.1.xml
doc/dbus-monitor.1.xml
doc/dbus-run-session.1.xml
doc/dbus-send.1.xml
doc/dbus-test-tool.1.xml
doc/dbus-update-activation-environment.1.xml
doc/dbus-uuidgen.1.xml
doc/index.html
dbus-1.pc
dbus-1-uninstalled.pc
cmake/DBus1Config.cmake:cmake/DBus1Config.pkgconfig.in
cmake/DBus1ConfigVersion.cmake
])
AC_OUTPUT

dnl ==========================================================================
echo "
                    D-Bus $VERSION
                  ==============

	prefix:                   ${EXPANDED_PREFIX}
	exec_prefix:              ${exec_prefix}
        libdir:                   ${EXPANDED_LIBDIR}
        libexecdir:               ${EXPANDED_LIBEXECDIR}
        bindir:                   ${EXPANDED_BINDIR}
        sysconfdir:               ${EXPANDED_SYSCONFDIR}
        localstatedir:            ${EXPANDED_LOCALSTATEDIR}
        runstatedir:              ${EXPANDED_RUNSTATEDIR}
	datadir:                  ${EXPANDED_DATADIR}
	source code location:	  ${srcdir}
	compiler:		  ${CC}
	cflags:		          ${CFLAGS}
	cppflags:		  ${CPPFLAGS}
	cxxflags:		  ${CXXFLAGS}
	ldflags:		  ${LDFLAGS}
	64-bit int:		  ${DBUS_INT64_TYPE}
	32-bit int:		  ${DBUS_INT32_TYPE}
	16-bit int:		  ${DBUS_INT16_TYPE}
        Doxygen:                  ${DOXYGEN:-not found}
        xmlto:                    ${XMLTO:-not found}
        ducktype:                 ${DUCKTYPE:-not found}
        yelp-build:               ${YELP_BUILD:-not found}"

echo "
        Rebuilding generated files: ${USE_MAINTAINER_MODE}
        gcc coverage profiling:   ${enable_code_coverage}
        Building embedded tests:  ${enable_embedded_tests}
        Building modular tests:   ${enable_modular_tests}
            - with GLib:          ${with_glib}
        Installing tests:         ${enable_installed_tests}
        Building verbose mode:    ${enable_verbose_mode}
        Building assertions:      ${enable_asserts}
        Building checks:          ${enable_checks}
        Building bus stats API:   ${enable_stats}
        Building SELinux support: ${have_selinux}
        Building AppArmor support: ${have_apparmor}
        Building inotify support: ${have_inotify}
        Building kqueue support:  ${have_kqueue}
        Building systemd support: ${have_systemd}
        Traditional activation:   ${enable_traditional_activation}
        Building X11 code:        ${have_x11}
        Building Doxygen docs:    ${enable_doxygen_docs}
        Building Qt help file:    ${enable_qthelp_docs}
        Building Ducktype docs:   ${enable_ducktype_docs}
        Building XML docs:        ${enable_xml_docs}
        Building launchd support: ${have_launchd}
        System bus socket:        ${DBUS_SYSTEM_SOCKET}
        System bus address:       ${DBUS_SYSTEM_BUS_DEFAULT_ADDRESS}
        System bus PID file:      ${DBUS_SYSTEM_PID_FILE}
        Session bus listens on:   ${DBUS_SESSION_BUS_LISTEN_ADDRESS}
        Session clients connect to: ${DBUS_SESSION_BUS_CONNECT_ADDRESS}
        pam_console auth dir:     ${with_console_auth_dir:-(none)}
        Console owner file:       ${have_console_owner_file}
        Console owner file path:  ${DBUS_CONSOLE_OWNER_FILE}
	System bus user:          ${DBUS_USER}
	Session bus services dir: ${EXPANDED_DATADIR}/dbus-1/services
        'make check' socket dir:  ${TEST_SOCKET_DIR}
"
if test x$have_launchd = xyes; then
        echo "        launchd agent dir:        ${LAUNCHD_AGENT_DIR}"
fi
echo

if test x$enable_embedded_tests = xyes; then
        echo "NOTE: building with unit tests increases the size of the installed library and renders it insecure."
fi
if test x$enable_embedded_tests = xyes -a x$enable_asserts = xno; then
        echo "NOTE: building with embedded tests but without assertions means tests may not properly report failures (this configuration is only useful when doing something like profiling the tests)"
fi
AS_IF([test x$enable_code_coverage = xyes],[
    AC_MSG_WARN([Building with coverage profiling is definitely for developers only.])
  ])
if test x$enable_verbose_mode = xyes; then
        echo "NOTE: building with verbose mode increases library size, may slightly increase security risk, and decreases performance."
fi
if test x$enable_asserts = xyes; then
        echo "NOTE: building with assertions increases library size and decreases performance."
fi
if test x$enable_checks = xno; then
        echo "NOTE: building without checks for arguments passed to public API makes it harder to debug apps using D-Bus, but will slightly decrease D-Bus library size and _very_ slightly improve performance."
fi
