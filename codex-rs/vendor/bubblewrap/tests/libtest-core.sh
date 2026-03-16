# Core source library for shell script tests; the
# canonical version lives in:
#
#   https://github.com/ostreedev/ostree
#
# Known copies are in the following repos:
#
# - https://github.com/containers/bubblewrap
# - https://github.com/coreos/rpm-ostree
#
# Copyright (C) 2017 Colin Walters <walters@verbum.org>
#
# SPDX-License-Identifier: LGPL-2.0-or-later
#
# This library is free software; you can redistribute it and/or
# modify it under the terms of the GNU Lesser General Public
# License as published by the Free Software Foundation; either
# version 2 of the License, or (at your option) any later version.
#
# This library is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
# Lesser General Public License for more details.
#
# You should have received a copy of the GNU Lesser General Public
# License along with this library; if not, write to the
# Free Software Foundation, Inc., 59 Temple Place - Suite 330,
# Boston, MA 02111-1307, USA.

fatal() {
    echo $@ 1>&2; exit 1
}
# fatal() is shorter to type, but retain this alias
assert_not_reached () {
    fatal "$@"
}

# Some tests look for specific English strings. Use a UTF-8 version
# of the C (POSIX) locale if we have one, or fall back to en_US.UTF-8
# (https://sourceware.org/glibc/wiki/Proposals/C.UTF-8)
#
# If we can't find the locale command assume we have support for C.UTF-8
# (e.g. musl based systems)
if type -p locale >/dev/null; then
    export LC_ALL=$(locale -a | grep -iEe '^(C|en_US)\.(UTF-8|utf8)$' | head -n1 || true)
    if [ -z "${LC_ALL}" ]; then fatal "Can't find suitable UTF-8 locale"; fi
else
    export LC_ALL=C.UTF-8
fi
# A GNU extension, used whenever LC_ALL is not C
unset LANGUAGE

# This should really be the default IMO
export G_DEBUG=fatal-warnings

assert_streq () {
    test "$1" = "$2" || fatal "$1 != $2"
}

assert_str_match () {
    if ! echo "$1" | grep -E -q "$2"; then
	      fatal "$1 does not match regexp $2"
    fi
}

assert_not_streq () {
    (! test "$1" = "$2") || fatal "$1 == $2"
}

assert_has_file () {
    test -f "$1" || fatal "Couldn't find '$1'"
}

assert_has_dir () {
    test -d "$1" || fatal "Couldn't find '$1'"
}

# Dump ls -al + file contents to stderr, then fatal()
_fatal_print_file() {
    file="$1"
    shift
    ls -al "$file" >&2
    sed -e 's/^/# /' < "$file" >&2
    fatal "$@"
}

_fatal_print_files() {
    file1="$1"
    shift
    file2="$1"
    shift
    ls -al "$file1" >&2
    sed -e 's/^/# /' < "$file1" >&2
    ls -al "$file2" >&2
    sed -e 's/^/# /' < "$file2" >&2
    fatal "$@"
}

assert_not_has_file () {
    if test -f "$1"; then
        _fatal_print_file "$1" "File '$1' exists"
    fi
}

assert_not_file_has_content () {
    fpath=$1
    shift
    for re in "$@"; do
        if grep -q -e "$re" "$fpath"; then
            _fatal_print_file "$fpath" "File '$fpath' matches regexp '$re'"
        fi
    done
}

assert_not_has_dir () {
    if test -d "$1"; then
	      fatal "Directory '$1' exists"
    fi
}

assert_file_has_content () {
    fpath=$1
    shift
    for re in "$@"; do
        if ! grep -q -e "$re" "$fpath"; then
            _fatal_print_file "$fpath" "File '$fpath' doesn't match regexp '$re'"
        fi
    done
}

assert_file_has_content_once () {
    fpath=$1
    shift
    for re in "$@"; do
        if ! test $(grep -e "$re" "$fpath" | wc -l) = "1"; then
            _fatal_print_file "$fpath" "File '$fpath' doesn't match regexp '$re' exactly once"
        fi
    done
}

assert_file_has_content_literal () {
    fpath=$1; shift
    for s in "$@"; do
        if ! grep -q -F -e "$s" "$fpath"; then
            _fatal_print_file "$fpath" "File '$fpath' doesn't match fixed string list '$s'"
        fi
    done
}

assert_file_has_mode () {
    mode=$(stat -c '%a' $1)
    if [ "$mode" != "$2" ]; then
        fatal "File '$1' has wrong mode: expected $2, but got $mode"
    fi
}

assert_symlink_has_content () {
    if ! test -L "$1"; then
        fatal "File '$1' is not a symbolic link"
    fi
    if ! readlink "$1" | grep -q -e "$2"; then
        _fatal_print_file "$1" "Symbolic link '$1' doesn't match regexp '$2'"
    fi
}

assert_file_empty() {
    if test -s "$1"; then
        _fatal_print_file "$1" "File '$1' is not empty"
    fi
}

assert_files_equal() {
    if ! cmp "$1" "$2"; then
        _fatal_print_files "$1" "$2" "File '$1' and '$2' is not equal"
    fi
}

# Use to skip all of these tests
skip() {
    echo "1..0 # SKIP" "$@"
    exit 0
}

report_err () {
  local exit_status="$?"
  { { local BASH_XTRACEFD=3; } 2> /dev/null
  echo "Unexpected nonzero exit status $exit_status while running: $BASH_COMMAND" >&2
  } 3> /dev/null
}
trap report_err ERR
