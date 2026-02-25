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
    $RUN --info-fd 42 --unshare-user --unshare-pid sh -c 'readlink /proc/self/ns/pid > sandbox-pidns; cat < donepipe' >/dev/null 42>info.json &
    while ! test -f sandbox-pidns; do sleep 1; done
    SANDBOX1PID=$(extract_child_pid info.json)

    ASAN_OPTIONS=detect_leaks=0 LSAN_OPTIONS=detect_leaks=0 \
    $RUN --userns 11 --pidns 12 readlink /proc/self/ns/pid > sandbox2-pidns 11< /proc/$SANDBOX1PID/ns/user 12< /proc/$SANDBOX1PID/ns/pid
    echo foo > donepipe

    assert_files_equal sandbox-pidns sandbox2-pidns

    rm donepipe info.json sandbox-pidns

    echo "ok - Test --pidns"
fi
