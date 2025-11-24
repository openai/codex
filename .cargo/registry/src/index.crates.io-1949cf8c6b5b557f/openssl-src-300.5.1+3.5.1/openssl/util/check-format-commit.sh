#!/bin/bash
# Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").
# You may not use this file except in compliance with the License.
# You can obtain a copy in the file LICENSE in the source distribution
# or at https://www.openssl.org/source/license.html
#
# This script is a wrapper around check-format.pl.
# It accepts the same commit revision range as 'git diff' as arguments,
# or just a single commit id, and uses it to identify the files and line ranges
# that were changed in that commit range, filtering check-format.pl output
# only to lines that fall into the change ranges of the changed files.
# examples:
# check-format-commit.sh       # check unstaged changes
# check-format-commit.sh HEAD
# check-format-commit.sh @~3..
# check-format-commit.sh f5981c9629667a5a5d6
# check-format-commit.sh f5981c9629667a5a5d6..ee0bf38e8709bf71888

# Allowlist of files to scan
# Currently this is any .c or .h file (with an optional .in suffix)
FILE_NAME_END_ALLOWLIST=("\.[ch]\(.in\)\?")

# Global vars

# TEMPDIR is used to hold any files this script creates
# And is cleaned on EXIT with a trap function
TEMPDIR=$(mktemp -d /tmp/checkformat.XXXXXX)

# TOPDIR always points to the root of the git tree we are working in
# used to locate the check-format.pl script
TOPDIR=$(git rev-parse --show-toplevel)


# cleanup handler function, returns us to the root of the git tree
# and erases our temp directory
cleanup() {
    rm -rf $TEMPDIR
    cd $TOPDIR
}

trap cleanup EXIT

# Get the list of ids of the commits we are checking,
# or empty for unstaged changes.
# This lets us pass in symbolic ref names like master/etc and 
# resolve them to commit ids easily
COMMIT_RANGE="$@"
[ -n $COMMIT_RANGE ] && COMMIT_LAST=$(git rev-parse $COMMIT_RANGE)

# Fail gracefully if git rev-parse doesn't produce a valid commit
if [ $? -ne 0 ]
then
    echo "$1 is not a valid commit range or commit id"
    exit 1
fi

# If the commit range is exactly one revision,
# git rev-parse will output just the commit id of that one alone.
# In that case, we must manipulate a little to get a desirable result,
# as 'git diff' has a slightly different interpretation of a single commit id:
# it takes that to mean all commits up to HEAD, plus any unstaged changes.
if [ $(echo -n "$COMMIT_LAST" | wc -w) -ne 1 ]; then
    COMMIT_LAST=$(echo "$COMMIT_LAST" | head -1)
else
    # $COMMIT_RANGE is just one commit, make it an actual range
    COMMIT_RANGE=$COMMIT_RANGE^..$COMMIT_RANGE
fi

# Create an iterable list of files to check formatting on,
# including the line ranges that are changed by the commits
# It produces output of this format:
# <file name> <change start line>, <change line count>
git diff -U0 $COMMIT_RANGE | awk '
    BEGIN {myfile=""} 
    /^\+\+\+/ { sub(/^b./,"",$2); file=$2 }
    /^@@/     { sub(/^\+/,"",$3); range=$3; printf file " " range "\n" }
    ' > $TEMPDIR/ranges.txt

# filter in anything that matches on a filter regex
for i in ${FILE_NAME_END_ALLOWLIST[@]}
do
    # Note the space after the $i below.  This is done because we want
    # to match on file name suffixes, but the input file is of the form
    # <commit> <file path> <range start>, <range length>
    # So we can't just match on end of line.  The additional space
    # here lets us match on suffixes followed by the expected space
    # in the input file
    grep "$i " $TEMPDIR/ranges.txt >> $TEMPDIR/ranges.filter || true
done

REMAINING_FILES=$(wc -l <$TEMPDIR/ranges.filter)
if [ $REMAINING_FILES -eq 0 ]
then
    echo "The given commit range has no C source file changes that require checking"
    exit 0
fi

# unless checking the format of unstaged changes,
# check out the files from the commit range.
if [ -n "$COMMIT_RANGE" ]
then
    # For each file name in ranges, we show that file at the commit range
    # we are checking, and redirect it to the same path,
    # relative to $TEMPDIR/check-format.
    # This give us the full file path to run check-format.pl on
    # with line numbers matching the ranges in the $TEMPDIR/ranges.filter file
    for j in $(awk '{print $1}' $TEMPDIR/ranges.filter | sort -u)
    do
        FDIR=$(dirname $j)
        mkdir -p $TEMPDIR/check-format/$FDIR
        git show $COMMIT_LAST:$j > $TEMPDIR/check-format/$j
    done
fi

# Now for each file in $TEMPDIR/ranges.filter, run check-format.pl
for j in $(awk '{print $1}' $TEMPDIR/ranges.filter | sort -u)
do
    range_start=()
    range_end=()

    # Get the ranges for this file. Create 2 arrays.  range_start contains
    # the start lines for valid ranges from the commit.  the range_end array
    # contains the corresponding end line.  Note, since diff output gives us
    # a line count for a change, the range_end[k] entry is actually
    # range_start[k]+line count
    for k in $(grep ^$j $TEMPDIR/ranges.filter | awk '{print $2}')
    do
        RSTART=$(echo $k | awk -F',' '{print $1}')
        RLEN=$(echo $k | awk -F',' '{print $2}')
        # when the hunk is just one line, its length is implied
        if [ -z "$RLEN" ]; then RLEN=1; fi
        let REND=$RSTART+$RLEN
        range_start+=($RSTART)
        range_end+=($REND)
    done

    # Go to our checked out tree, unless checking unstaged changes
    [ -n "$COMMIT_RANGE" ] && cd $TEMPDIR/check-format

    # Actually run check-format.pl on the file, capturing the output
    # in a temporary file.  Note the format of check-format.pl output is
    # <file path>:<line number>:<error text>:<offending line contents>
    $TOPDIR/util/check-format.pl $j > $TEMPDIR/results.txt

    # Now we filter the check-format.pl output based on the changed lines
    # captured in the range_start/end arrays
    let maxidx=${#range_start[@]}-1
    for k in $(seq 0 1 $maxidx)
    do
        RSTART=${range_start[$k]}
        REND=${range_end[$k]}

        # field 2 of check-format.pl output is the offending line number
        # Check here if any line in that output falls between any of the 
        # start/end ranges defined in the range_start/range_end array.
        # If it does fall in that range, print the entire line to stdout
        awk -v rstart=$RSTART -v rend=$REND -F':' '
                /:/ { if (rstart <= $2 && $2 <= rend) print $0 }
            ' $TEMPDIR/results.txt >>$TEMPDIR/results-filtered.txt
    done
done
cat $TEMPDIR/results-filtered.txt

# If any findings were in range, exit with a different error code
if [ -s $TEMPDIR/results-filtered.txt ]
then
    exit 2
fi
