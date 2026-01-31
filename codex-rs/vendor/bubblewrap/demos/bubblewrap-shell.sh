#!/usr/bin/env bash
# Use bubblewrap to run /bin/sh reusing the host OS binaries (/usr), but with
# separate /tmp, /home, /var, /run, and /etc. For /etc we just inherit the
# host's resolv.conf, and set up "stub" passwd/group files.  Not sharing
# /home for example is intentional.  If you wanted to, you could design
# a bwrap-using program that shared individual parts of /home, perhaps
# public content.
#
# Another way to build on this example is to remove --share-net to disable
# networking.
set -euo pipefail
(exec bwrap --ro-bind /usr /usr \
      --dir /tmp \
      --dir /var \
      --symlink ../tmp var/tmp \
      --proc /proc \
      --dev /dev \
      --ro-bind /etc/resolv.conf /etc/resolv.conf \
      --symlink usr/lib /lib \
      --symlink usr/lib64 /lib64 \
      --symlink usr/bin /bin \
      --symlink usr/sbin /sbin \
      --chdir / \
      --unshare-all \
      --share-net \
      --die-with-parent \
      --dir /run/user/$(id -u) \
      --setenv XDG_RUNTIME_DIR "/run/user/`id -u`" \
      --setenv PS1 "bwrap-demo$ " \
      --file 11 /etc/passwd \
      --file 12 /etc/group \
      /bin/sh) \
    11< <(getent passwd $UID 65534) \
    12< <(getent group $(id -g) 65534)
