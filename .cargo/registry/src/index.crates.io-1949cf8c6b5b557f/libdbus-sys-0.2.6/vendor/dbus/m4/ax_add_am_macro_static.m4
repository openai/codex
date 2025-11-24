# ===========================================================================
#  https://www.gnu.org/software/autoconf-archive/ax_add_am_macro_static.html
# ===========================================================================
#
# SYNOPSIS
#
#   AX_ADD_AM_MACRO_STATIC([RULE])
#
# DESCRIPTION
#
#   Adds the specified rule to $AMINCLUDE.
#
# LICENSE
#
#   Copyright (c) 2009 Tom Howard <tomhoward@users.sf.net>
#   Copyright (c) 2009 Allan Caffee <allan.caffee@gmail.com>
#
#   Copying and distribution of this file, with or without modification, are
#   permitted in any medium without royalty provided the copyright notice
#   and this notice are preserved. This file is offered as-is, without any
#   warranty.

#serial 8

AC_DEFUN([AX_ADD_AM_MACRO_STATIC],[
  AC_REQUIRE([AX_AM_MACROS_STATIC])
  AX_AC_APPEND_TO_FILE(AMINCLUDE_STATIC,[$1])
])
