# ===========================================================================
#   https://www.gnu.org/software/autoconf-archive/ax_ac_append_to_file.html
# ===========================================================================
#
# SYNOPSIS
#
#   AX_AC_APPEND_TO_FILE([FILE],[DATA])
#
# DESCRIPTION
#
#   Appends the specified data to the specified Autoconf is run. If you want
#   to append to a file when configure is run use AX_APPEND_TO_FILE instead.
#
# LICENSE
#
#   Copyright (c) 2009 Allan Caffee <allan.caffee@gmail.com>
#
#   Copying and distribution of this file, with or without modification, are
#   permitted in any medium without royalty provided the copyright notice
#   and this notice are preserved. This file is offered as-is, without any
#   warranty.

#serial 10

AC_DEFUN([AX_AC_APPEND_TO_FILE],[
AC_REQUIRE([AX_FILE_ESCAPES])
m4_esyscmd(
AX_FILE_ESCAPES
[
printf "%s" "$2" >> "$1"
])
])
