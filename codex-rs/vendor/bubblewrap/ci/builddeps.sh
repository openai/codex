#!/usr/bin/env bash
# Copyright 2021 Simon McVittie
# SPDX-License-Identifier: LGPL-2.0-or-later

set -eux
set -o pipefail

usage() {
    if [ "${1-2}" -ne 0 ]; then
        exec >&2
    fi
    cat <<EOF
Usage: see source code
EOF
    exit "${1-2}"
}

opt_clang=

getopt_temp="help"
getopt_temp="$getopt_temp,clang"

getopt_temp="$(getopt -o '' --long "${getopt_temp}" -n "$0" -- "$@")"
eval set -- "$getopt_temp"
unset getopt_temp

while true; do
    case "$1" in
        (--clang)
            clang=yes
            shift
            ;;

        (--help)
            usage 0
            # not reached
            ;;

        (--)
            shift
            break
            ;;

        (*)
            echo 'Error parsing options' >&2
            usage 2
            ;;
    esac
done

# No more arguments please
for arg in "$@"; do
    usage 2
done

if dpkg-vendor --derives-from Debian; then
    apt-get -y update
    apt-get -q -y install \
        build-essential \
        docbook-xml \
        docbook-xsl \
        libcap-dev \
        libselinux1-dev \
        libtool \
        meson \
        pkg-config \
        python3 \
        xsltproc \
        ${NULL+}

    if [ -n "${opt_clang}" ]; then
        apt-get -y install clang
    fi

    exit 0
fi

if command -v yum; then
    yum -y install \
        'pkgconfig(libselinux)' \
        /usr/bin/eu-readelf \
        docbook-style-xsl \
        gcc \
        git \
        libasan \
        libcap-devel \
        libtool \
        libtsan \
        libubsan \
        libxslt \
        make \
        meson \
        redhat-rpm-config \
        rsync \
        ${NULL+}

    if [ -n "${opt_clang}" ]; then
        yum -y install clang
    fi

    exit 0
fi

echo "Unknown distribution" >&2
exit 1

# vim:set sw=4 sts=4 et:
