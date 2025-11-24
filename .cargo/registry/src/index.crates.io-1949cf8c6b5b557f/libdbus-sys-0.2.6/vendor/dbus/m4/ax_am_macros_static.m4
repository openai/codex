# ===========================================================================
#   https://www.gnu.org/software/autoconf-archive/ax_am_macros_static.html
# ===========================================================================
#
# SYNOPSIS
#
#   AX_AM_MACROS_STATIC
#
# DESCRIPTION
#
#   Adds support for macros that create Automake rules. You must manually
#   add the following line
#
#     include $(top_srcdir)/aminclude_static.am
#
#   to your Makefile.am files.
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

#serial 11

AC_DEFUN([AMINCLUDE_STATIC],[aminclude_static.am])

AC_DEFUN([AX_AM_MACROS_STATIC],
[
AX_AC_PRINT_TO_FILE(AMINCLUDE_STATIC,[
# ]AMINCLUDE_STATIC[ generated automatically by Autoconf
# from AX_AM_MACROS_STATIC on ]m4_esyscmd([LC_ALL=C date])[
])
])
