#! /bin/sh
# Copyright 2018-2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

HERE=`dirname $0`

# Get all version data as shell variables
. $HERE/../VERSION.dat

if [ -n "$PRE_RELEASE_TAG" ]; then PRE_RELEASE_TAG=-$PRE_RELEASE_TAG; fi
if [ -n "$BUILD_METADATA" ]; then BUILD_METADATA=+$BUILD_METADATA; fi
version=$MAJOR.$MINOR.$PATCH$PRE_RELEASE_TAG$BUILD_METADATA
basename=openssl

NAME="$basename-$version"

while [ $# -gt 0 ]; do
    case "$1" in
        --name=* ) NAME=`echo "$1" | sed -e 's|[^=]*=||'`       ;;
        --name ) shift; NAME="$1"                               ;;
        --tarfile=* ) TARFILE=`echo "$1" | sed -e 's|[^=]*=||'` ;;
        --tarfile ) shift; TARFILE="$1"                         ;;
        * ) echo >&2 "Could not parse '$1'"; exit 1             ;;
    esac
    shift
done

if [ -z "$TARFILE" ]; then TARFILE="$NAME.tar"; fi

# This counts on .gitattributes to specify what files should be ignored
git archive --worktree-attributes -9 --prefix="$NAME/" -o $TARFILE.gz -v HEAD

# Good old way to ensure we display an absolute path
td=`dirname $TARFILE`
tf=`basename $TARFILE`
ls -l "`cd $td; pwd`/$tf.gz"
