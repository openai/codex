#! /bin/sh
# Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# These functions load, manipulate and store the current version information
# for OpenSSL 3.0 and on.
# They are meant to be minimalistic for easy refactoring depending on OpenSSL
# version.
#
# Version information is stored in the following variables:
#
# |MAJOR|, |MINOR|, |PATCH| are the three parts of a version number.
# |MAJOR| is to be increased for new major releases, |MINOR| for new
# minor releases, and |PATCH| for update releases.
#
# |SERIES| tells what release series the current version belongs to, and
# is composed from |MAJOR| and |MINOR|.
# |VERSION| tells what the current version is, and is composed from |MAJOR|,
# |MINOR| and |PATCH|.
# |TYPE| tells what state the source is in.  It may have an empty value
# for released source, or 'dev' for "in development".
# |PRE_LABEL| may be "alpha" or "beta" to signify an ongoing series of
# alpha or beta releases.  |PRE_NUM| is a pre-release counter for the
# alpha and beta release series, but isn't necessarily strictly tied
# to the prerelease label.
#
# Scripts loading this file are not allowed to manipulate these
# variables directly.  They must use functions such as fixup_version()
# below, or next_release_state(), found in release-state-fn.sh.

# These functions depend on |SOURCEDIR|, which must have the intended
# OpenSSL source directory as value.

get_version () {
    eval $(git cat-file blob HEAD:VERSION.dat)
    VERSION="$MAJOR.$MINOR.$PATCH"
    SERIES="$MAJOR.$MINOR"
    TYPE=$( echo "$PRE_RELEASE_TAG" \
                | sed -E \
                      -e 's|^dev$|dev|' \
                      -e 's|^alpha([0-9]+)(-(dev))?$|\3|' \
                      -e 's|^beta([0-9]+)(-(dev))?$|\3|' )
    PRE_LABEL=$( echo "$PRE_RELEASE_TAG" \
                     | sed -E \
                           -e 's|^dev$||' \
                           -e 's|^alpha([0-9]+)(-(dev))?$|alpha|' \
                           -e 's|^beta([0-9]+)(-(dev))?$|beta|' )
    PRE_NUM=$( echo "$PRE_RELEASE_TAG" \
                   | sed -E \
                         -e 's|^dev$|0|' \
                         -e 's|^alpha([0-9]+)(-(dev))?$|\1|' \
                         -e 's|^beta([0-9]+)(-(dev))?$|\1|' )
    _BUILD_METADATA=''
    if [ -n "$PRE_RELEASE_TAG" ]; then _PRE_RELEASE_TAG="-${PRE_RELEASE_TAG}"; fi
    if [ -n "$BUILD_METADATA" ]; then _BUILD_METADATA="+${BUILD_METADATA}"; fi
}

# $1 is one of "alpha", "beta", "final", "", or "minor"
fixup_version () {
    local new_label="$1"

    case "$new_label" in
        alpha | beta )
            if [ "$new_label" != "$PRE_LABEL" ]; then
                PRE_LABEL="$new_label"
                PRE_NUM=1
            elif [ "$TYPE" = 'dev' ]; then
                PRE_NUM=$(expr $PRE_NUM + 1)
            fi
            ;;
        final | '' )
            if [ "$TYPE" = 'dev' ]; then
                PATCH=$(expr $PATCH + 1)
            fi
            PRE_LABEL=
            PRE_NUM=0
            ;;
        minor )
            if [ "$TYPE" = 'dev' ]; then
                MINOR=$(expr $MINOR + 1)
                PATCH=0
            fi
            PRE_LABEL=
            PRE_NUM=0
            ;;
    esac

    VERSION="$MAJOR.$MINOR.$PATCH"
    SERIES="$MAJOR.$MINOR"
}

set_version () {
    case "$TYPE+$PRE_LABEL+$PRE_NUM" in
        *++* )
            PRE_RELEASE_TAG="$TYPE"
            ;;
        dev+* )
            PRE_RELEASE_TAG="$PRE_LABEL$PRE_NUM-dev"
            ;;
        +* )
            PRE_RELEASE_TAG="$PRE_LABEL$PRE_NUM"
            ;;
    esac
    if [ -n "$PRE_RELEASE_TAG" ]; then _PRE_RELEASE_TAG="-${PRE_RELEASE_TAG}"; fi
    cat > "$SOURCEDIR/VERSION.dat" <<EOF
MAJOR=$MAJOR
MINOR=$MINOR
PATCH=$PATCH
PRE_RELEASE_TAG=$PRE_RELEASE_TAG
BUILD_METADATA=$BUILD_METADATA
RELEASE_DATE="$RELEASE_DATE"
SHLIB_VERSION=$SHLIB_VERSION
EOF
}
