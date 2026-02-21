#!/usr/bin/env bash
set -euo pipefail

# Required for bubblewrap to work on Linux CI runners.
sudo sysctl -w kernel.unprivileged_userns_clone=1

# Ubuntu 24.04+ can additionally gate unprivileged user namespaces behind AppArmor.
if sudo sysctl -a 2>/dev/null | grep -q '^kernel.apparmor_restrict_unprivileged_userns'; then
  sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
fi
