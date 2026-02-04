#!/usr/bin/env bash

set -xeuo pipefail

srcd=$(cd $(dirname "$0") && pwd)

. ${srcd}/libtest.sh

bn=$(basename "$0")

test_count=0
ok () {
    test_count=$((test_count + 1))
    echo ok $test_count "$@"
}
ok_skip () {
    ok "# SKIP" "$@"
}
done_testing () {
    echo "1..$test_count"
}

# Test help
${BWRAP} --help > help.txt
assert_file_has_content help.txt "usage: ${BWRAP}"
ok "Help works"

for ALT in "" "--unshare-user-try" "--unshare-pid" "--unshare-user-try --unshare-pid"; do
    # Test fuse fs as bind source
    if [ "x$FUSE_DIR" != "x" ]; then
        $RUN $ALT --proc /proc --dev /dev --bind $FUSE_DIR /tmp/foo true
        ok "can bind-mount a FUSE directory with $ALT"
    else
        ok_skip "no FUSE support"
    fi
    # no --dev => no devpts => no map_root workaround
    $RUN $ALT --proc /proc true
    ok "can mount /proc with $ALT"
    # No network
    $RUN $ALT --unshare-net --proc /proc --dev /dev true
    ok "can unshare network, create new /dev with $ALT"
    # Unreadable file
    echo -n "expect EPERM: " >&2

    # Test caps when bwrap is not setuid
    if test -n "${bwrap_is_suid:-}"; then
        CAP="--cap-add ALL"
    else
        CAP=""
    fi

    if ! cat /etc/shadow >/dev/null &&
        $RUN $CAP $ALT --unshare-net --proc /proc --bind /etc/shadow /tmp/foo cat /tmp/foo; then
        assert_not_reached Could read /etc/shadow via /tmp/foo bind-mount
    fi

    if ! cat /etc/shadow >/dev/null &&
        $RUN $CAP $ALT --unshare-net --proc /proc --bind /etc/shadow /tmp/foo cat /etc/shadow; then
        assert_not_reached Could read /etc/shadow
    fi

    ok "cannot read /etc/shadow with $ALT"
    # Unreadable dir
    if [ "x$UNREADABLE" != "x" ]; then
        echo -n "expect EPERM: " >&2
        if $RUN $ALT --unshare-net --proc /proc --dev /dev --bind $UNREADABLE /tmp/foo cat /tmp/foo; then
            assert_not_reached Could read $UNREADABLE
        fi
        ok "cannot read $UNREADABLE with $ALT"
    else
        ok_skip "not sure what unreadable file to use"
    fi

    # bind dest in symlink (https://github.com/projectatomic/bubblewrap/pull/119)
    $RUN $ALT --dir /tmp/dir --symlink dir /tmp/link --bind /etc /tmp/link true
    ok "can bind a destination over a symlink"
done

# Test symlink behaviour
rm -f ./symlink
$RUN --ro-bind / / --bind "$(pwd)" "$(pwd)" --symlink /dev/null "$(pwd)/symlink" true >&2
readlink ./symlink > target.txt
assert_file_has_content target.txt /dev/null
ok "--symlink works"
$RUN --ro-bind / / --bind "$(pwd)" "$(pwd)" --symlink /dev/null "$(pwd)/symlink" true >&2
ok "--symlink is idempotent"
if $RUN --ro-bind / / --bind "$(pwd)" "$(pwd)" --symlink /dev/full "$(pwd)/symlink" true 2>err.txt; then
    fatal "creating a conflicting symlink should have failed"
else
    assert_file_has_content err.txt "Can't make symlink .*: existing destination is /dev/null"
fi
ok "--symlink doesn't overwrite a conflicting symlink"

# Test devices
$RUN --unshare-pid --dev /dev ls -al /dev/{stdin,stdout,stderr,null,random,urandom,fd,core} >/dev/null
ok "all expected devices were created"

# Test --as-pid-1
$RUN --unshare-pid --as-pid-1 --bind / / bash -c 'echo $$' > as_pid_1.txt
assert_file_has_content as_pid_1.txt "1"
ok "can run as pid 1"

# Test --info-fd and --json-status-fd
if $RUN --unshare-all --info-fd 42 --json-status-fd 43 -- bash -c 'exit 42' 42>info.json 43>json-status.json 2>err.txt; then
    fatal "should have been exit 42"
fi
assert_file_has_content info.json '"child-pid": [0-9]'
assert_file_has_content json-status.json '"child-pid": [0-9]'
assert_file_has_content_literal json-status.json '"exit-code": 42'
ok "info and json-status fd"

DATA=$($RUN --proc /proc --unshare-all --info-fd 42 --json-status-fd 43 -- bash -c 'stat -L -c "%n %i" /proc/self/ns/*' 42>info.json 43>json-status.json 2>err.txt)

for NS in "ipc" "mnt" "net" "pid" "uts"; do

    want=$(echo "$DATA" | grep "/proc/self/ns/$NS" | awk '{print $2}')
    assert_file_has_content info.json "$want"
    assert_file_has_content json-status.json "$want"
done

ok "namespace id info in info and json-status fd"

if ! command -v strace >/dev/null || ! strace -h | grep -v -e default | grep -e fault >/dev/null; then
    ok_skip "no strace fault injection"
else
    ! strace -o /dev/null -f -e trace=prctl -e fault=prctl:when=39 $RUN --die-with-parent --json-status-fd 42 true 42>json-status.json
    assert_not_file_has_content json-status.json '"exit-code": [0-9]'
    ok "pre-exec failure doesn't include exit-code in json-status"
fi

notanexecutable=/
$RUN --json-status-fd 42 $notanexecutable 42>json-status.json || true
assert_not_file_has_content json-status.json '"exit-code": [0-9]'
ok "exec failure doesn't include exit-code in json-status"

# These tests require --unshare-user
if test -n "${bwrap_is_suid:-}"; then
    ok_skip "no --cap-add support"
    ok_skip "no --cap-add support"
    ok_skip "no --disable-userns"
else
    BWRAP_RECURSE="$BWRAP --unshare-user --uid 0 --gid 0 --cap-add ALL --bind / / --bind /proc /proc"

    # $BWRAP May be inaccessible due to the user namespace so use /proc/self/exe
    $BWRAP_RECURSE -- /proc/self/exe --unshare-all --bind / / --bind /proc /proc echo hello > recursive_proc.txt
    assert_file_has_content recursive_proc.txt "hello"
    ok "can mount /proc recursively"

    $BWRAP_RECURSE -- /proc/self/exe --unshare-all ${BWRAP_RO_HOST_ARGS} findmnt > recursive-newroot.txt
    assert_file_has_content recursive-newroot.txt "/usr"
    ok "can pivot to new rootfs recursively"

    $BWRAP --dev-bind / / -- true
    ! $BWRAP --assert-userns-disabled --dev-bind / / -- true
    $BWRAP --unshare-user --disable-userns --dev-bind / / -- true
    ! $BWRAP --unshare-user --disable-userns --dev-bind / / -- $BWRAP --dev-bind / / -- true
    $BWRAP --unshare-user --disable-userns --dev-bind / / -- sh -c "echo 2 > /proc/sys/user/max_user_namespaces || true; ! $BWRAP --unshare-user --dev-bind / / -- true"
    $BWRAP --unshare-user --disable-userns --dev-bind / / -- sh -c "echo 100 > /proc/sys/user/max_user_namespaces || true; ! $BWRAP --unshare-user --dev-bind / / -- true"
    $BWRAP --unshare-user --disable-userns --dev-bind / / -- sh -c "! $BWRAP --unshare-user --dev-bind / / --assert-userns-disabled -- true"

    $BWRAP_RECURSE --dev-bind / / -- true
    ! $BWRAP_RECURSE --assert-userns-disabled --dev-bind / / -- true
    $BWRAP_RECURSE --unshare-user --disable-userns --dev-bind / / -- true
    ! $BWRAP_RECURSE --unshare-user --disable-userns --dev-bind / / -- /proc/self/exe --dev-bind / / -- true
    $BWRAP_RECURSE --unshare-user --disable-userns --dev-bind / / -- sh -c "echo 2 > /proc/sys/user/max_user_namespaces || true; ! $BWRAP --unshare-user --dev-bind / / -- true"
    $BWRAP_RECURSE --unshare-user --disable-userns --dev-bind / / -- sh -c "echo 100 > /proc/sys/user/max_user_namespaces || true; ! $BWRAP --unshare-user --dev-bind / / -- true"
    $BWRAP_RECURSE --unshare-user --disable-userns --dev-bind / / -- sh -c "! $BWRAP --unshare-user --dev-bind / / --assert-userns-disabled -- true"

    ok "can disable nested userns"
fi

# Test error prefixing
if $RUN --unshare-pid --bind /source-enoent /dest true 2>err.txt; then
    assert_not_reached "bound nonexistent source"
fi
assert_file_has_content err.txt "^bwrap: Can't find source path.*source-enoent"
ok "error prefixing"

if ! ${is_uidzero}; then
    # When invoked as non-root, check that by default we have no caps left
    for OPT in "" "--unshare-user-try --as-pid-1" "--unshare-user-try" "--as-pid-1"; do
        e=0
        $RUN $OPT --unshare-pid getpcaps 1 >&2 2> caps.test || e=$?
        sed -e 's/^/# /' < caps.test >&2
        test "$e" = 0
        assert_not_file_has_content caps.test ': =.*cap'
    done
    ok "we have no caps as uid != 0"
else
    capsh --print | sed -e 's/no-new-privs=0/no-new-privs=1/' > caps.expected

    for OPT in "" "--as-pid-1"; do
        $RUN $OPT --unshare-pid capsh --print >caps.test
        diff -u caps.expected caps.test
    done
    # And test that we can drop all, as well as specific caps
    $RUN $OPT --cap-drop ALL --unshare-pid capsh --print >caps.test
    assert_file_has_content caps.test 'Current: =$'
    # Check for dropping kill/fowner (we assume all uid 0 callers have this)
    # But we should still have net_bind_service for example
    $RUN $OPT --cap-drop CAP_KILL --cap-drop CAP_FOWNER --unshare-pid capsh --print >caps.test
    # capsh's output format changed from v2.29 -> drops are now indicated with -eip
    if grep 'Current: =.*+eip$' caps.test; then
        assert_not_file_has_content caps.test '^Current: =.*cap_kill.*+eip$'
        assert_not_file_has_content caps.test '^Current: =.*cap_fowner.*+eip$'
        assert_file_has_content caps.test '^Current: =.*cap_net_bind_service.*+eip$'
    else
        assert_file_has_content caps.test '^Current: =eip.*cap_kill.*-eip$'
        assert_file_has_content caps.test '^Current: =eip.*cap_fowner.*-eip$'
        assert_not_file_has_content caps.test '^Current: =.*cap_net_bind_service.*-eip$'
    fi
    ok "we have the expected caps as uid 0"
fi

# Test --die-with-parent

cat >lockf-n.py <<EOF
#!/usr/bin/env python3
import struct,fcntl,sys
path = sys.argv[1]
if sys.argv[2] == 'wait':
  locktype = fcntl.F_SETLKW
else:
  locktype = fcntl.F_SETLK
lockdata = struct.pack("hhqqhh", fcntl.F_WRLCK, 0, 0, 0, 0, 0)
fd=open(sys.argv[1], 'a')
try:
  fcntl.fcntl(fd.fileno(), locktype, lockdata)
except IOError as e:
  sys.exit(1)
sys.exit(0)
EOF
chmod a+x lockf-n.py
touch lock

for die_with_parent_argv in "--die-with-parent" "--die-with-parent --unshare-pid"; do
    # We have to loop here, because bwrap doesn't wait for the lock if
    # another process is holding it. If we're unlucky, lockf-n.py will
    # be holding it.
    bash -c "while true; do $RUN ${die_with_parent_argv} --lock-file $(pwd)/lock sleep 1h; done" &
    childshellpid=$!

    # Wait for lock to be taken (yes hacky)
    for x in $(seq 10); do
        if ./lockf-n.py ./lock nowait; then
            sleep 1
        else
            break
        fi
    done
    if ./lockf-n.py ./lock nowait; then
        assert_not_reached "timed out waiting for lock"
    fi

    # Kill the shell, which should kill bwrap (and the sleep)
    kill -9 ${childshellpid}
    # Lock file should be unlocked
    ./lockf-n.py ./lock wait
    ok "die with parent ${die_with_parent_argv}"
done

printf '%s--dir\0/tmp/hello/world\0' '' > test.args
printf '%s--dir\0/tmp/hello/world2\0' '' > test.args2
printf '%s--dir\0/tmp/hello/world3\0' '' > test.args3
$RUN --args 3 --args 4 --args 5 /bin/sh -c 'test -d /tmp/hello/world && test -d /tmp/hello/world2 && test -d /tmp/hello/world3' 3<test.args 4<test.args2 5<test.args3
ok "we can parse arguments from a fd"

mkdir bin
echo "#!/bin/sh" > bin/--inadvisable-executable-name--
echo "echo hello" >> bin/--inadvisable-executable-name--
chmod +x bin/--inadvisable-executable-name--
PATH="${srcd}:$PATH" $RUN -- sh -c "echo hello" > stdout
assert_file_has_content stdout hello
ok "we can run with --"
PATH="$(pwd)/bin:$PATH" $RUN -- --inadvisable-executable-name-- > stdout
assert_file_has_content stdout hello
ok "we can run an inadvisable executable name with --"
if $RUN -- --dev-bind /dev /dev sh -c 'echo should not have run'; then
    assert_not_reached "'--dev-bind' should have been interpreted as a (silly) executable name"
fi
ok "options like --dev-bind are defanged by --"

if command -v mktemp > /dev/null; then
    tempfile="$(mktemp /tmp/bwrap-test-XXXXXXXX)"
    echo "hello" > "$tempfile"
    $BWRAP --bind / / cat "$tempfile" > stdout
    assert_file_has_content stdout hello
    ok "bind-mount of / exposes real /tmp"
    $BWRAP --bind / / --bind /tmp /tmp cat "$tempfile" > stdout
    assert_file_has_content stdout hello
    ok "bind-mount of /tmp exposes real /tmp"
    if [ -d /mnt ] && [ ! -L /mnt ]; then
        $BWRAP --bind / / --bind /tmp /mnt cat "/mnt/${tempfile#/tmp/}" > stdout
        assert_file_has_content stdout hello
        ok "bind-mount of /tmp onto /mnt exposes real /tmp"
    else
        ok_skip "/mnt does not exist or is a symlink"
    fi
else
    ok_skip "mktemp not found"
    ok_skip "mktemp not found"
    ok_skip "mktemp not found"
fi

if $RUN test -d /tmp/oldroot; then
    assert_not_reached "/tmp/oldroot should not be visible"
fi
if $RUN test -d /tmp/newroot; then
    assert_not_reached "/tmp/newroot should not be visible"
fi

echo "hello" > input.$$
$BWRAP --bind / / --bind "$(pwd)" /tmp cat /tmp/input.$$ > stdout
assert_file_has_content stdout hello
if $BWRAP --bind / / --bind "$(pwd)" /tmp test -d /tmp/oldroot; then
    assert_not_reached "/tmp/oldroot should not be visible"
fi
if $BWRAP --bind / / --bind "$(pwd)" /tmp test -d /tmp/newroot; then
    assert_not_reached "/tmp/newroot should not be visible"
fi
ok "we can mount another directory onto /tmp"

echo "hello" > input.$$
$RUN --bind "$(pwd)" /tmp/here cat /tmp/here/input.$$ > stdout
assert_file_has_content stdout hello
if $RUN --bind "$(pwd)" /tmp/here test -d /tmp/oldroot; then
    assert_not_reached "/tmp/oldroot should not be visible"
fi
if $RUN --bind "$(pwd)" /tmp/here test -d /tmp/newroot; then
    assert_not_reached "/tmp/newroot should not be visible"
fi
ok "we can mount another directory inside /tmp"

touch some-file
mkdir -p some-dir
rm -fr new-dir-mountpoint
rm -fr new-file-mountpoint
$RUN \
    --bind "$(pwd -P)/some-dir" "$(pwd -P)/new-dir-mountpoint" \
    --bind "$(pwd -P)/some-file" "$(pwd -P)/new-file-mountpoint" \
    true
command stat -c '%a' new-dir-mountpoint > new-dir-permissions
assert_file_has_content new-dir-permissions 755
command stat -c '%a' new-file-mountpoint > new-file-permissions
assert_file_has_content new-file-permissions 444
ok "Files and directories created as mount points have expected permissions"


if [ -S /dev/log ]; then
    $RUN --bind / / --bind "$(realpath /dev/log)" "$(realpath /dev/log)" true
    ok "Can bind-mount a socket (/dev/log) onto a socket"
else
    ok_skip "- /dev/log is not a socket, cannot test bubblewrap#409"
fi

mkdir -p dir-already-existed
chmod 0710 dir-already-existed
mkdir -p dir-already-existed2
chmod 0754 dir-already-existed2
rm -fr new-dir-default-perms
rm -fr new-dir-set-perms
$RUN \
    --perms 1741 --dir "$(pwd -P)/new-dir-set-perms" \
    --dir "$(pwd -P)/dir-already-existed" \
    --perms 0741 --dir "$(pwd -P)/dir-already-existed2" \
    --dir "$(pwd -P)/dir-chmod" \
    --chmod 1755 "$(pwd -P)/dir-chmod" \
    --dir "$(pwd -P)/new-dir-default-perms" \
    true
command stat -c '%a' new-dir-default-perms > new-dir-permissions
assert_file_has_content new-dir-permissions '^755$'
command stat -c '%a' new-dir-set-perms > new-dir-permissions
assert_file_has_content new-dir-permissions '^1741$'
command stat -c '%a' dir-already-existed > dir-permissions
assert_file_has_content dir-permissions '^710$'
command stat -c '%a' dir-already-existed2 > dir-permissions
assert_file_has_content dir-permissions '^754$'
command stat -c '%a' dir-chmod > dir-permissions
assert_file_has_content dir-permissions '^1755$'
ok "Directories created explicitly have expected permissions"

rm -fr parent
rm -fr parent-of-1777
rm -fr parent-of-0755
rm -fr parent-of-0644
rm -fr parent-of-0750
rm -fr parent-of-0710
rm -fr parent-of-0720
rm -fr parent-of-0640
rm -fr parent-of-0700
rm -fr parent-of-0600
rm -fr parent-of-0705
rm -fr parent-of-0604
rm -fr parent-of-0000
$RUN \
    --dir "$(pwd -P)"/parent/dir \
    --perms 1777 --dir "$(pwd -P)"/parent-of-1777/dir \
    --perms 0755 --dir "$(pwd -P)"/parent-of-0755/dir \
    --perms 0644 --dir "$(pwd -P)"/parent-of-0644/dir \
    --perms 0750 --dir "$(pwd -P)"/parent-of-0750/dir \
    --perms 0710 --dir "$(pwd -P)"/parent-of-0710/dir \
    --perms 0720 --dir "$(pwd -P)"/parent-of-0720/dir \
    --perms 0640 --dir "$(pwd -P)"/parent-of-0640/dir \
    --perms 0700 --dir "$(pwd -P)"/parent-of-0700/dir \
    --perms 0600 --dir "$(pwd -P)"/parent-of-0600/dir \
    --perms 0705 --dir "$(pwd -P)"/parent-of-0705/dir \
    --perms 0604 --dir "$(pwd -P)"/parent-of-0604/dir \
    --perms 0000 --dir "$(pwd -P)"/parent-of-0000/dir \
    true
command stat -c '%a' parent > dir-permissions
assert_file_has_content dir-permissions '^755$'
command stat -c '%a' parent-of-1777 > dir-permissions
assert_file_has_content dir-permissions '^755$'
command stat -c '%a' parent-of-0755 > dir-permissions
assert_file_has_content dir-permissions '^755$'
command stat -c '%a' parent-of-0644 > dir-permissions
assert_file_has_content dir-permissions '^755$'
command stat -c '%a' parent-of-0750 > dir-permissions
assert_file_has_content dir-permissions '^750$'
command stat -c '%a' parent-of-0710 > dir-permissions
assert_file_has_content dir-permissions '^750$'
command stat -c '%a' parent-of-0720 > dir-permissions
assert_file_has_content dir-permissions '^750$'
command stat -c '%a' parent-of-0640 > dir-permissions
assert_file_has_content dir-permissions '^750$'
command stat -c '%a' parent-of-0700 > dir-permissions
assert_file_has_content dir-permissions '^700$'
command stat -c '%a' parent-of-0600 > dir-permissions
assert_file_has_content dir-permissions '^700$'
command stat -c '%a' parent-of-0705 > dir-permissions
assert_file_has_content dir-permissions '^705$'
command stat -c '%a' parent-of-0604 > dir-permissions
assert_file_has_content dir-permissions '^705$'
command stat -c '%a' parent-of-0000 > dir-permissions
assert_file_has_content dir-permissions '^700$'
chmod -R 0700 parent*
rm -fr parent*
ok "Directories created as parents have expected permissions"

$RUN \
    --perms 01777 --tmpfs "$(pwd -P)" \
    cat /proc/self/mountinfo >&2
$RUN \
    --perms 01777 --tmpfs "$(pwd -P)" \
    stat -c '%a' "$(pwd -P)" > dir-permissions
assert_file_has_content dir-permissions '^1777$'
$RUN \
    --tmpfs "$(pwd -P)" \
    stat -c '%a' "$(pwd -P)" > dir-permissions
assert_file_has_content dir-permissions '^755$'
ok "tmpfs has expected permissions"

# 1048576 = 1 MiB
if test -n "${bwrap_is_suid:-}"; then
    if $RUN --size 1048576 --tmpfs "$(pwd -P)" true; then
        assert_not_reached "Should not allow --size --tmpfs when setuid"
    fi
    ok "--size --tmpfs is not allowed when setuid"
elif df --output=size --block-size=1K "$(pwd -P)" >/dev/null 2>/dev/null; then
    $RUN \
        --size 1048576 --tmpfs "$(pwd -P)" \
        df --output=size --block-size=1K "$(pwd -P)" > dir-size
    assert_file_has_content dir-size '^ *1024$'
    $RUN \
        --size 1048576 --perms 01777 --tmpfs "$(pwd -P)" \
        stat -c '%a' "$(pwd -P)" > dir-permissions
    assert_file_has_content dir-permissions '^1777$'
    $RUN \
        --size 1048576 --perms 01777 --tmpfs "$(pwd -P)" \
        df --output=size --block-size=1K "$(pwd -P)" > dir-size
    assert_file_has_content dir-size '^ *1024$'
    $RUN \
        --perms 01777 --size 1048576 --tmpfs "$(pwd -P)" \
        stat -c '%a' "$(pwd -P)" > dir-permissions
    assert_file_has_content dir-permissions '^1777$'
    $RUN \
        --perms 01777 --size 1048576 --tmpfs "$(pwd -P)" \
        df --output=size --block-size=1K "$(pwd -P)" > dir-size
    assert_file_has_content dir-size '^ *1024$'
    ok "tmpfs has expected size"
else
    $RUN --size 1048576 --tmpfs "$(pwd -P)" true
    $RUN --perms 01777 --size 1048576 --tmpfs "$(pwd -P)" true
    $RUN --size 1048576 --perms 01777 --tmpfs "$(pwd -P)" true
    ok_skip "df is too old, cannot test --size --tmpfs fully"
fi

$RUN \
    --file 0 /tmp/file \
    stat -c '%a' /tmp/file < /dev/null > file-permissions
assert_file_has_content file-permissions '^666$'
$RUN \
    --perms 0640 --file 0 /tmp/file \
    stat -c '%a' /tmp/file < /dev/null > file-permissions
assert_file_has_content file-permissions '^640$'
$RUN \
    --bind-data 0 /tmp/file \
    stat -c '%a' /tmp/file < /dev/null > file-permissions
assert_file_has_content file-permissions '^600$'
$RUN \
    --perms 0640 --bind-data 0 /tmp/file \
    stat -c '%a' /tmp/file < /dev/null > file-permissions
assert_file_has_content file-permissions '^640$'
$RUN \
    --ro-bind-data 0 /tmp/file \
    stat -c '%a' /tmp/file < /dev/null > file-permissions
assert_file_has_content file-permissions '^600$'
$RUN \
    --perms 0640 --ro-bind-data 0 /tmp/file \
    stat -c '%a' /tmp/file < /dev/null > file-permissions
assert_file_has_content file-permissions '^640$'
ok "files have expected permissions"

if $RUN --size 0 --tmpfs /tmp/a true; then
    assert_not_reached Zero tmpfs size allowed
fi
if $RUN --size 123bogus --tmpfs /tmp/a true; then
    assert_not_reached Bogus tmpfs size allowed
fi
if $RUN --size '' --tmpfs /tmp/a true; then
    assert_not_reached Empty tmpfs size allowed
fi
if $RUN --size -12345678 --tmpfs /tmp/a true; then
    assert_not_reached Negative tmpfs size allowed
fi
if $RUN --size ' -12345678' --tmpfs /tmp/a true; then
    assert_not_reached Negative tmpfs size with space allowed
fi
# This is 2^64
if $RUN --size 18446744073709551616 --tmpfs /tmp/a true; then
    assert_not_reached Overflowing tmpfs size allowed
fi
# This is 2^63 + 1; note that the current max size is SIZE_MAX/2
if $RUN --size 9223372036854775809 --tmpfs /tmp/a true; then
    assert_not_reached Too-large tmpfs size allowed
fi
ok "bogus tmpfs size not allowed"

if $RUN --perms 0640 --perms 0640 --tmpfs /tmp/a true; then
    assert_not_reached Multiple perms options allowed
fi
if $RUN --size 1048576 --size 1048576 --tmpfs /tmp/a true; then
    assert_not_reached Multiple perms options allowed
fi
ok "--perms and --size only allowed once"


FOO= BAR=baz $RUN --setenv FOO bar sh -c 'echo "$FOO$BAR"' > stdout
assert_file_has_content stdout barbaz
FOO=wrong BAR=baz $RUN --setenv FOO bar sh -c 'echo "$FOO$BAR"' > stdout
assert_file_has_content stdout barbaz
FOO=wrong BAR=baz $RUN --unsetenv FOO sh -c 'printf "%s%s" "$FOO" "$BAR"' > stdout
printf baz > reference
assert_files_equal stdout reference
FOO=wrong BAR=wrong $RUN --clearenv /usr/bin/env > stdout
echo "PWD=$(pwd -P)" > reference
assert_files_equal stdout reference
ok "environment manipulation"

$RUN sh -c 'echo $0' > stdout
assert_file_has_content stdout sh
$RUN --argv0 sh sh -c 'echo $0' > stdout
assert_file_has_content stdout sh
$RUN --argv0 right sh -c 'echo $0' > stdout
assert_file_has_content stdout right
ok "argv0 manipulation"

echo "foobar" > file-data
$RUN --proc /proc --dev /dev --bind / / --bind-fd 100 /tmp cat /tmp/file-data 100< . > stdout
assert_file_has_content stdout foobar

ok "bind-fd"

$RUN --chdir / --chdir / true > stdout 2>&1
assert_file_has_content stdout '^bwrap: Only the last --chdir option will take effect$'
ok "warning logged for redundant --chdir"

$RUN --level-prefix --chdir / --chdir / true > stdout 2>&1
assert_file_has_content stdout '^<4>bwrap: Only the last --chdir option will take effect$'
ok "--level-prefix"

if test -n "${bwrap_is_suid:-}"; then
    ok_skip "no --overlay support"
    ok_skip "no --overlay support"
    ok_skip "no --tmp-overlay support"
    ok_skip "no --ro-overlay support"
    ok_skip "no --overlay-src support"
else
    mkdir lower1 lower2 upper work
    printf 1 > lower1/a
    printf 2 > lower1/b
    printf 3 > lower2/b
    printf 4 > upper/a

    # Check if unprivileged overlayfs is available
    if ! unshare -rm mount -t overlay -o lowerdir=lower1,upperdir=upper,workdir=work,userxattr overlay lower2; then
        ok_skip "no kernel support for unprivileged overlayfs"
        ok_skip "no kernel support for unprivileged overlayfs"
        ok_skip "no kernel support for unprivileged overlayfs"
        ok_skip "no kernel support for unprivileged overlayfs"
        ok_skip "no kernel support for unprivileged overlayfs"
    else

        # Test --overlay
        if $RUN --overlay upper work /tmp true 2>err.txt; then
            assert_not_reached At least one --overlay-src not required
        fi
        assert_file_has_content err.txt "^bwrap: --overlay requires at least one --overlay-src"
        $RUN --overlay-src lower1 --overlay upper work /tmp/x/y/z cat /tmp/x/y/z/a > stdout
        assert_file_has_content stdout '^4$'
        $RUN --overlay-src lower1 --overlay upper work /tmp/x/y/z cat /tmp/x/y/z/b > stdout
        assert_file_has_content stdout '^2$'
        $RUN --overlay-src lower1 --overlay-src lower2 --overlay upper work /tmp/x/y/z cat /tmp/x/y/z/a > stdout
        assert_file_has_content stdout '^4$'
        $RUN --overlay-src lower1 --overlay-src lower2 --overlay upper work /tmp/x/y/z cat /tmp/x/y/z/b > stdout
        assert_file_has_content stdout '^3$'
        $RUN --overlay-src lower1 --overlay-src lower2 --overlay upper work /tmp/x/y/z sh -c 'printf 5 > /tmp/x/y/z/b; cat /tmp/x/y/z/b' > stdout
        assert_file_has_content stdout '^5$'
        assert_file_has_content upper/b '^5$'
        ok "--overlay"

        # Test --overlay path escaping
        # Coincidentally, ":,\ is the face I make contemplating anyone who might
        # need this functionality, not that that's going to stop me from supporting
        # it.
        mkdir 'lower ":,\' 'upper ":,\' 'work ":,\'
        printf 1 > 'lower ":,\'/a
        $RUN --overlay-src 'lower ":,\' --overlay 'upper ":,\' 'work ":,\' /tmp/x sh -c 'cat /tmp/x/a; printf 2 > /tmp/x/a; cat /tmp/x/a' > stdout
        assert_file_has_content stdout '^12$'
        assert_file_has_content 'lower ":,\'/a '^1$'
        assert_file_has_content 'upper ":,\'/a '^2$'
        ok "--overlay path escaping"

        # Test --tmp-overlay
        printf 1 > lower1/a
        printf 2 > lower1/b
        printf 3 > lower2/b
        if $RUN --tmp-overlay /tmp true 2>err.txt; then
            assert_not_reached At least one --overlay-src not required
        fi
        assert_file_has_content err.txt "^bwrap: --tmp-overlay requires at least one --overlay-src"
        $RUN --overlay-src lower1 --tmp-overlay /tmp/x/y/z cat /tmp/x/y/z/a > stdout
        assert_file_has_content stdout '^1$'
        $RUN --overlay-src lower1 --tmp-overlay /tmp/x/y/z cat /tmp/x/y/z/b > stdout
        assert_file_has_content stdout '^2$'
        $RUN --overlay-src lower1 --overlay-src lower2 --tmp-overlay /tmp/x/y/z cat /tmp/x/y/z/a > stdout
        assert_file_has_content stdout '^1$'
        $RUN --overlay-src lower1 --overlay-src lower2 --tmp-overlay /tmp/x/y/z cat /tmp/x/y/z/b > stdout
        assert_file_has_content stdout '^3$'
        $RUN --overlay-src lower1 --overlay-src lower2 --tmp-overlay /tmp/x/y/z sh -c 'printf 4 > /tmp/x/y/z/b; cat /tmp/x/y/z/b' > stdout
        assert_file_has_content stdout '^4$'
        $RUN --overlay-src lower1 --tmp-overlay /tmp/x --overlay-src lower2 --tmp-overlay /tmp/y sh -c 'cat /tmp/x/b; printf 4 > /tmp/x/b; cat /tmp/x/b; cat /tmp/y/b' > stdout
        assert_file_has_content stdout '^243$'
        assert_file_has_content lower1/b '^2$'
        assert_file_has_content lower2/b '^3$'
        ok "--tmp-overlay"

        # Test --ro-overlay
        printf 1 > lower1/a
        printf 2 > lower1/b
        printf 3 > lower2/b
        if $RUN --ro-overlay /tmp true 2>err.txt; then
            assert_not_reached At least two --overlay-src not required
        fi
        assert_file_has_content err.txt "^bwrap: --ro-overlay requires at least two --overlay-src"
        if $RUN --overlay-src lower1 --ro-overlay /tmp true 2>err.txt; then
            assert_not_reached At least two --overlay-src not required
        fi
        assert_file_has_content err.txt "^bwrap: --ro-overlay requires at least two --overlay-src"
        $RUN --overlay-src lower1 --overlay-src lower2 --ro-overlay /tmp/x/y/z cat /tmp/x/y/z/a > stdout
        assert_file_has_content stdout '^1$'
        $RUN --overlay-src lower1 --overlay-src lower2 --ro-overlay /tmp/x/y/z cat /tmp/x/y/z/b > stdout
        assert_file_has_content stdout '^3$'
        $RUN --overlay-src lower1 --overlay-src lower2 --ro-overlay /tmp/x/y/z sh -c 'printf 4 > /tmp/x/y/z/b; cat /tmp/x/y/z/b' > stdout
        assert_file_has_content stdout '^3$'
        ok "--ro-overlay"

        # Test --overlay-src restrictions
        if $RUN --overlay-src /tmp true 2>err.txt; then
            assert_not_reached Trailing --overlay-src allowed
        fi
        assert_file_has_content err.txt "^bwrap: --overlay-src must be followed by another --overlay-src or one of --overlay, --tmp-overlay, or --ro-overlay"
        if $RUN --overlay-src /tmp --chdir / true 2>err.txt; then
            assert_not_reached --overlay-src allowed to precede non-overlay options
        fi
        assert_file_has_content err.txt "^bwrap: --overlay-src must be followed by another --overlay-src or one of --overlay, --tmp-overlay, or --ro-overlay"
        ok "--overlay-src restrictions"

    fi
fi

done_testing
