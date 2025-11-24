# ld-version-script.m4 serial 1
dnl Copyright (C) 2008, 2009 Free Software Foundation, Inc.
dnl This file is free software; the Free Software Foundation
dnl gives unlimited permission to copy and/or distribute it,
dnl with or without modifications, as long as this notice is preserved.

dnl From Simon Josefsson

# FIXME: The test below returns a false positive for mingw
# cross-compiles, 'local:' statements does not reduce number of
# exported symbols in a DLL.  Use --disable-ld-version-script to work
# around the problem.

# gl_LD_VERSION_SCRIPT
# --------------------
# Check if LD supports linker scripts, and define automake conditional
# HAVE_LD_VERSION_SCRIPT if so.
AC_DEFUN([gl_LD_VERSION_SCRIPT],
[
  AC_ARG_ENABLE([ld-version-script],
    AS_HELP_STRING([--enable-ld-version-script],
      [enable linker version script (default is enabled when possible)]),
      [have_ld_version_script=$enableval], [])
  if test -z "$have_ld_version_script"; then
    AC_MSG_CHECKING([if LD -Wl,--version-script works])
    save_LDFLAGS="$LDFLAGS"
    LDFLAGS="$LDFLAGS -Wl,--version-script=conftest.map"
    cat > conftest.map <<EOF
VERS_1 {
	global: sym;
};

VERS_2 {
        global: sym;
} VERS_1;
EOF
    AC_LINK_IFELSE([AC_LANG_PROGRAM([[]], [[]])],
                   [have_ld_version_script=yes], [have_ld_version_script=no])
    rm -f conftest.map
    LDFLAGS="$save_LDFLAGS"
    AC_MSG_RESULT($have_ld_version_script)
  fi
])
