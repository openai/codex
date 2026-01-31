#!/bin/bash

set -e

echo "kernel.apparmor_restrict_unprivileged_userns = 0" > /etc/sysctl.d/99-userns.conf
sysctl --system
