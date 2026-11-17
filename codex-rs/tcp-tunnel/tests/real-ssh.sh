#!/usr/bin/env bash
set -euo pipefail

# Opt-in Linux integration test using an unprivileged OpenSSH server.
fixture="$(mktemp -d)"
sshd_pid=""
trap 'if [[ -n "$sshd_pid" ]]; then kill "$sshd_pid" 2>/dev/null || true; fi; rm -rf "$fixture"' EXIT
ssh-keygen -q -t ed25519 -N "" -f "$fixture/host"
ssh-keygen -q -t ed25519 -N "" -f "$fixture/client"
cp "$fixture/client.pub" "$fixture/authorized_keys"
port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')"
cat > "$fixture/proxy.py" <<'PY'
import socket
import sys
import threading

peer = socket.create_connection(("127.0.0.1", int(sys.argv[1])))

def upload():
    try:
        while chunk := sys.stdin.buffer.read1(65536):
            peer.sendall(chunk)
        peer.shutdown(socket.SHUT_WR)
    except OSError:
        pass

threading.Thread(target=upload, daemon=True).start()
while chunk := peer.recv(65536):
    sys.stdout.buffer.write(chunk)
    sys.stdout.buffer.flush()
PY
"$(command -v sshd)" -D -e -f /dev/null -p "$port" -h "$fixture/host" \
    -o ListenAddress=127.0.0.1 -o PidFile=none -o UsePAM=no -o StrictModes=no \
    -o "AuthorizedKeysFile=$fixture/authorized_keys" -o PasswordAuthentication=no \
    -o KbdInteractiveAuthentication=no -o LogLevel=ERROR &
sshd_pid=$!
export PORTALS_TEST_SSHD_PORT="$port" PORTALS_TEST_KEY="$fixture/client"
export PORTALS_TEST_PROXY="$fixture/proxy.py"
cd "$(dirname "$0")/../.."
just test -p codex-tcp-tunnel --lib real_ssh_delivery_over_http3 --run-ignored all
