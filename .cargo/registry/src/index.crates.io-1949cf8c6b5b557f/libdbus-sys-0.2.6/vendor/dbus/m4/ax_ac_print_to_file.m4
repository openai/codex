# ===========================================================================
#   https://www.gnu.org/software/autoconf-archive/ax_ac_print_to_file.html
# ===========================================================================
#
# SYNOPSIS
#
#   AX_AC_PRINT_TO_FILE([FILE],[DATA])
#
# DESCRIPTION
#
#   Writes the specified data to the specified file when Autoconf is run. If
#   you want to print to a file when configure is run use AX_PRINT_TO_FILE
#   instead.
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

AC_DEFUN([AX_AC_PRINT_TO_FILE],[
m4_esyscmd(
AC_REQUIRE([AX_FILE_ESCAPES])
[
printf "%s" "$2" > "$1"
])
])
