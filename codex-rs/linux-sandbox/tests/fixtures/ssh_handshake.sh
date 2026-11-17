#!/bin/sh
# Called in private mount, PID and network namespaces. The server and client use disposable keys.
set -eu
export PATH=/usr/bin:/usr/sbin:/bin:/sbin
export LC_ALL=C
skip() { echo "$*" >&2; exit 77; }
for tool in ssh sshd ssh-keygen bwrap setpriv ip; do
    command -v "$tool" >/dev/null || skip "$tool is unavailable"
done
cd /
as_client() { setpriv --reuid 1000 --regid 1000 --clear-groups "$@"; }
as_client bwrap --unshare-user --unshare-pid --ro-bind / / --proc /proc -- /bin/true ||
    skip "the ordinary user cannot start a stock bubblewrap sandbox"
mount -t tmpfs -o mode=0755 tmpfs /etc/ssh || skip "cannot mount private SSH configuration"
mount -t tmpfs -o mode=0755 tmpfs /run || skip "cannot stage the private sshd runtime"
ip link set lo up || skip "cannot enable isolated loopback"
umask 022
mkdir -p /run/sshd /etc/ssh/home /etc/ssh/ssh_config.d
install -m 755 "$1" /etc/ssh/codex-linux-sandbox
# Supply an unlocked account independently of the host's users and PAM configuration.
{ echo 'codexssh:x:1000:1000::/etc/ssh/home:/bin/sh'; cat /etc/passwd; } > /etc/ssh/passwd
mount --bind /etc/ssh/passwd /etc/passwd
chown 1000:1000 /etc/ssh/home
cd /etc/ssh
ssh-keygen -q -t ed25519 -N '' -f host_key
ssh-keygen -q -t ed25519 -N '' -f client_key
chown 1000:1000 client_key
cat > sshd_config <<'EOF'
ListenAddress 127.0.0.1
Port 2222
HostKey /etc/ssh/host_key
PidFile /run/sshd.pid
AuthorizedKeysFile /etc/ssh/client_key.pub
AuthenticationMethods publickey
UsePAM no
UseDNS no
EOF
"$(command -v sshd)" -t -f /etc/ssh/sshd_config
"$(command -v sshd)" -D -e -f /etc/ssh/sshd_config >/run/sshd.log 2>&1 &
sshd_pid=$!
trap 'kill "$sshd_pid" 2>/dev/null || :' EXIT

echo 'ServerAliveInterval 73' > ssh_config.d/site.conf
cat > ssh_config <<'EOF'
Include /etc/ssh/ssh_config.d/*.conf
Host fixture.invalid
    HostName 127.0.0.1
    Port 2222
    User codexssh
    IdentityFile /etc/ssh/client_key
    IdentitiesOnly yes
    BatchMode yes
    ConnectionAttempts 5
    HostKeyAlias fixture.invalid
    StrictHostKeyChecking yes
    UserKnownHostsFile /etc/ssh/pinned_host
    GlobalKnownHostsFile none
EOF
pin() { { printf 'fixture.invalid '; cat "$1"; } > pinned_host; }
profile=$2
sandbox() { as_client /etc/ssh/codex-linux-sandbox --sandbox-policy-cwd / --permission-profile "$profile" -- "$@"; }
as_client ssh -G -T fixture.invalid > /run/native
grep -qx 'serveraliveinterval 73' /run/native
pin host_key.pub
sandbox ssh -T fixture.invalid 'id -u' > /run/remote
grep -qx 1000 /run/remote
sandbox sh -c 'test ! -e /etc/ssh/ssh_config.d/site.conf'

pin client_key.pub
rc=0
sandbox ssh -T fixture.invalid 'echo unexpected' > /run/wrong-key 2>/run/error || rc=$?
test "$rc" = 255
test ! -s /run/wrong-key
grep -F 'REMOTE HOST IDENTIFICATION HAS CHANGED' /run/error
