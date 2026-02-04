# shellcheck shell=bash

# Source library for shell script tests.
# Add non-bubblewrap-specific code to libtest-core.sh instead.
#
# Copyright (C) 2017 Colin Walters <walters@verbum.org>
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

set -e

if [ -n "${G_TEST_SRCDIR:-}" ]; then
  test_srcdir="${G_TEST_SRCDIR}/tests"
else
  test_srcdir=$(dirname "$0")
fi

if [ -n "${G_TEST_BUILDDIR:-}" ]; then
  test_builddir="${G_TEST_BUILDDIR}/tests"
else
  test_builddir=$(dirname "$0")
fi

. "${test_srcdir}/libtest-core.sh"

# Make sure /sbin/getpcaps etc. are in our PATH even if non-root
PATH="$PATH:/usr/sbin:/sbin"

tempdir=$(mktemp -d /var/tmp/tap-test.XXXXXX)
touch "${tempdir}/.testtmp"
cleanup() {
    if test -n "${TEST_SKIP_CLEANUP:-}"; then
        echo "Skipping cleanup of ${tempdir}"
    elif test -f "${tempdir}/.testtmp"; then
        rm -rf "${tempdir}"
    fi
}
trap cleanup EXIT
cd "${tempdir}"

: "${BWRAP:=bwrap}"
if test -u "$(type -p ${BWRAP})"; then
    bwrap_is_suid=true
fi

FUSE_DIR=
for mp in $(grep " fuse[. ]" /proc/self/mounts | grep "user_id=$(id -u)" | awk '{print $2}'); do
    if test -d "$mp"; then
        echo "# Using $mp as test fuse mount"
        FUSE_DIR="$mp"
        break
    fi
done

if test "$(id -u)" = "0"; then
    is_uidzero=true
else
    is_uidzero=false
fi

# This is supposed to be an otherwise readable file in an unreadable (by the user) dir
UNREADABLE=/root/.bashrc
if "${is_uidzero}" || test -x "$(dirname "$UNREADABLE")"; then
    UNREADABLE=
fi

# https://github.com/projectatomic/bubblewrap/issues/217
# are we on a merged-/usr system?
if [ /lib -ef /usr/lib ]; then
    BWRAP_RO_HOST_ARGS="--ro-bind /usr /usr
              --ro-bind /etc /etc
              --dir /var/tmp
              --symlink usr/lib /lib
              --symlink usr/lib64 /lib64
              --symlink usr/bin /bin
              --symlink usr/sbin /sbin
              --proc /proc
              --dev /dev"
else
    BWRAP_RO_HOST_ARGS="--ro-bind /usr /usr
              --ro-bind /etc /etc
              --ro-bind /bin /bin
              --ro-bind-try /lib /lib
              --ro-bind-try /lib64 /lib64
              --ro-bind-try /sbin /sbin
              --ro-bind-try /nix/store /nix/store
              --dir /var/tmp
              --proc /proc
              --dev /dev"
fi

# Default arg, bind whole host fs to /, tmpfs on /tmp
RUN="${BWRAP} --bind / / --tmpfs /tmp"

if [ -z "${BWRAP_MUST_WORK-}" ] && ! $RUN true; then
    skip Seems like bwrap is not working at all. Maybe setuid is not working
fi

extract_child_pid() {
    grep child-pid "$1" | sed "s/^.*: \([0-9]*\).*/\1/"
}
