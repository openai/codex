#!/usr/bin/env bash

set -xeuo pipefail

srcd=$(cd $(dirname "$0") && pwd)
. "${srcd}/libtest.sh"

echo "1..1"

# This test needs user namespaces
if test -n "${bwrap_is_suid:-}"; then
    echo "ok - # SKIP no setuid support for --unshare-user"
else
    mkfifo donepipe

    $RUN --info-fd 42 --unshare-user sh -c 'readlink /proc/self/ns/user > sandbox-userns; cat < donepipe' >/dev/null 42>info.json &
    while ! test -f sandbox-userns; do sleep 1; done
    SANDBOX1PID=$(extract_child_pid info.json)

    $RUN --userns 11 readlink /proc/self/ns/user > sandbox2-userns 11< /proc/$SANDBOX1PID/ns/user
    echo foo > donepipe

    assert_files_equal sandbox-userns sandbox2-userns

    rm donepipe info.json sandbox-userns

    echo "ok - Test --userns"
fi
