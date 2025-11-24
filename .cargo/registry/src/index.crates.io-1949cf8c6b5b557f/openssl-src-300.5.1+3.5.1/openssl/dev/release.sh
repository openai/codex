#! /bin/bash -e
# Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# This is the most shell agnostic way to specify that POSIX rules.
POSIXLY_CORRECT=1

# Force C locale because some commands (like date +%b) relies
# on the current locale.
export LC_ALL=C

usage () {
    cat <<EOF
Usage: release.sh [ options ... ]

--alpha         Start or increase the "alpha" pre-release tag.
--next-beta     Switch to the "beta" pre-release tag after alpha release.
                It can only be given with --alpha.
--beta          Start or increase the "beta" pre-release tag.
--final         Get out of "alpha" or "beta" and make a final release.
                Implies --branch.

--branch        Create a release branch 'openssl-{major}.{minor}',
                where '{major}' and '{minor}' are the major and minor
                version numbers.

--reviewer=<id> The reviewer of the commits.
--local-user=<keyid>
                For the purpose of signing tags and tar files, use this
                key (default: use the default e-mail address’ key).

--no-upload     Don't upload to upload@dev.openssl.org.
--no-update     Don't perform 'make update' and 'make update-fips-checksums'.
--verbose       Verbose output.
--debug         Include debug output.  Implies --no-upload.

--force         Force execution

--help          This text
--manual        The manual

If none of --alpha, --beta, or --final are given, this script tries to
figure out the next step.
EOF
    exit 0
}

# Set to one of 'major', 'minor', 'alpha', 'beta' or 'final'
next_method=
next_method2=

do_branch=false
warn_branch=false

do_clean=true
do_upload=true
do_update=true
DEBUG=:
VERBOSE=:
git_quiet=-q

force=false

do_help=false
do_manual=false

tagkey=' -s'
gpgkey=
reviewers=

upload_address=upload@dev.openssl.org

TEMP=$(getopt -l 'alpha,next-beta,beta,final' \
              -l 'branch' \
              -l 'no-upload,no-update' \
              -l 'verbose,debug' \
              -l 'local-user:' \
              -l 'reviewer:' \
              -l 'force' \
              -l 'help,manual' \
              -n release.sh -- - "$@")
eval set -- "$TEMP"
while true; do
    case $1 in
    --alpha | --beta | --final )
        next_method=$(echo "x$1" | sed -e 's|^x--||')
        if [ -z "$next_method2" ]; then
            next_method2=$next_method
        fi
        shift
        if [ "$next_method" = 'final' ]; then
            do_branch=true
        fi
        ;;
    --next-beta )
        next_method2=$(echo "x$1" | sed -e 's|^x--next-||')
        shift
        ;;
    --branch )
        do_branch=true
        warn_branch=true
        shift
        ;;
    --no-upload )
        do_upload=false
        shift
        ;;
    --no-update )
        do_update=false
        shift
        ;;
    --verbose )
        VERBOSE=echo
        git_quiet=
        shift
        ;;
    --debug )
        DEBUG=echo
        do_upload=false
        shift
        ;;
    --local-user )
        shift
        tagkey=" -u $1"
        gpgkey=" -u $1"
        shift
        ;;
    --reviewer )
        reviewers="$reviewers $1=$2"
        shift
        shift
        ;;
    --force )
        force=true
        shift
        ;;
    --help )
        usage
        exit 0
        ;;
    --manual )
        sed -e '1,/^### BEGIN MANUAL/d' \
            -e '/^### END MANUAL/,$d' \
            < "$0" \
            | pod2man \
            | man -l -
        exit 0
        ;;
    -- )
        shift
        break
        ;;
    * )
        echo >&2 "Unknown option $1"
        shift
        exit 1
        ;;
    esac
done

$DEBUG >&2 "DEBUG: \$next_method=$next_method"
$DEBUG >&2 "DEBUG: \$next_method2=$next_method2"

$DEBUG >&2 "DEBUG: \$do_branch=$do_branch"

$DEBUG >&2 "DEBUG: \$do_upload=$do_upload"
$DEBUG >&2 "DEBUG: \$do_update=$do_update"
$DEBUG >&2 "DEBUG: \$DEBUG=$DEBUG"
$DEBUG >&2 "DEBUG: \$VERBOSE=$VERBOSE"
$DEBUG >&2 "DEBUG: \$git_quiet=$git_quiet"

case "$next_method+$next_method2" in
    major+major | minor+minor )
        # These are expected
        ;;
    alpha+alpha | alpha+beta | beta+beta | final+final | + | +beta )
        # These are expected
        ;;
    * )
        echo >&2 "Internal option error ($next_method, $next_method2)"
        exit 1
        ;;
esac

# Verbosity feed for certain commands
VERBOSITY_FIFO=/tmp/openssl-$$.fifo
mkfifo -m 600 $VERBOSITY_FIFO
( cat $VERBOSITY_FIFO | while read L; do $VERBOSE "> $L"; done ) &
exec 42>$VERBOSITY_FIFO
trap "exec 42>&-; rm $VERBOSITY_FIFO" 0 2

# Setup ##############################################################

# Make sure we're in the work directory
cd $(dirname $0)/..
HERE=$(pwd)

# Check that we have the scripts that define functions we use
found=true
for fn in "$HERE/dev/release-aux/release-version-fn.sh" \
          "$HERE/dev/release-aux/release-state-fn.sh"; do
    if ! [ -f "$fn" ]; then
        echo >&2 "'$fn' is missing"
        found=false
    fi
done
if ! $found; then
    exit 1
fi

# Load version functions
. $HERE/dev/release-aux/release-version-fn.sh
. $HERE/dev/release-aux/release-state-fn.sh

# Make sure it's a branch we recognise
orig_branch=$(git rev-parse --abbrev-ref HEAD)
if (echo "$orig_branch" \
        | grep -E -q \
               -e '^master$' \
               -e '^OpenSSL_[0-9]+_[0-9]+_[0-9]+[a-z]*-stable$' \
               -e '^openssl-[0-9]+\.[0-9]+$'); then
    :
elif $force; then
    :
else
    echo >&2 "Not in master or any recognised release branch"
    echo >&2 "Please 'git checkout' an appropriate branch"
    exit 1
fi
orig_HEAD=$(git rev-parse HEAD)

# Initialize #########################################################

echo "== Initializing work tree"

get_version

# Generate a cloned directory name
release_clone="$orig_branch-release-tmp"

echo "== Work tree will be in $release_clone"

# Make a clone in a subdirectory and move there
if ! [ -d "$release_clone" ]; then
    $VERBOSE "== Cloning to $release_clone"
    git clone $git_quiet -b "$orig_branch" -o parent . "$release_clone"
fi
cd "$release_clone"

get_version

# Branches we will work with.  The release branch is where we make the
# changes for the release, the update branch is where we make the post-
# release changes
update_branch="$orig_branch"
release_branch="openssl-$SERIES"

# among others, we only create a release branch if the patch number is zero
if [ "$update_branch" = "$release_branch" ] || [ $PATCH -ne 0 ]; then
    if $do_branch && $warn_branch; then
        echo >&2 "Warning! We're already in a release branch; --branch ignored"
    fi
    do_branch=false
fi

if ! $do_branch; then
    release_branch="$update_branch"
fi

# Branches we create for PRs
branch_version="$VERSION${PRE_LABEL:+-$PRE_LABEL$PRE_NUM}"
tmp_update_branch="OSSL--$update_branch--$branch_version"
tmp_release_branch="OSSL--$release_branch--$branch_version"

# Check that we're still on the same branch as our parent repo, or on a
# release branch
current_branch=$(git rev-parse --abbrev-ref HEAD)
if [ "$current_branch" = "$update_branch" ]; then
    :
elif [ "$current_branch" = "$release_branch" ]; then
    :
else
    echo >&2 "The cloned sub-directory '$release_clone' is on a branch"
    if [ "$update_branch" = "$release_branch" ]; then
        echo >&2 "other than '$update_branch'."
    else
        echo >&2 "other than '$update_branch' or '$release_branch'."
    fi
    echo >&2 "Please 'cd \"$(pwd)\"; git checkout $update_branch'"
    exit 1
fi

SOURCEDIR=$(pwd)
$DEBUG >&2 "DEBUG: Source directory is $SOURCEDIR"

# Release ############################################################

# We always expect to start from a state of development
if [ "$TYPE" != 'dev' ]; then
    echo >&2 "Not in a development branch"
    echo >&2 "Have a look at the git log in $release_clone, it may be that"
    echo >&2 "a previous crash left it in an intermediate state and that"
    echo >&2 "need to drop the top commit:"
    echo >&2 ""
    echo >&2 "(cd $release_clone; git reset --hard HEAD^)"
    echo >&2 "# WARNING! LOOK BEFORE YOU ACT"
    exit 1
fi

# Update the version information.  This won't save anything anywhere, yet,
# but does check for possible next_method errors before we do bigger work.
next_release_state "$next_method"

# Create our temporary release branch
$VERBOSE "== Creating a local release branch: $tmp_release_branch"
git checkout $git_quiet -b "$tmp_release_branch"

echo "== Configuring OpenSSL for update and release.  This may take a bit of time"

./Configure cc >&42

$VERBOSE "== Checking source file updates and fips checksums"

make update >&42
# As long as we're doing an alpha release, we can have symbols without specific
# numbers assigned. In a beta or final release, all symbols MUST have an
# assigned number.
if [ "$next_method" != 'alpha' ]; then
    make renumber >&42
fi
make update-fips-checksums >&42

if [ -n "$(git status --porcelain)" ]; then
    $VERBOSE "== Committing updates"
    git add -u
    git commit $git_quiet -m $'make update\n\nRelease: yes'
    if [ -n "$reviewers" ]; then
        addrev --release --nopr $reviewers
    fi
fi

# Create our temporary update branch, if it's not the release branch.
# This is used in post-release below
if $do_branch; then
    $VERBOSE "== Creating a local update branch: $tmp_update_branch"
    git branch $git_quiet "$tmp_update_branch"
fi

# Write the version information we updated
set_version

if [ -n "$PRE_LABEL" ]; then
    release="$VERSION$_PRE_RELEASE_TAG$_BUILD_METADATA"
    release_text="$SERIES$_BUILD_METADATA $PRE_LABEL $PRE_NUM"
    announce_template=openssl-announce-pre-release.tmpl
else
    release="$VERSION$_BUILD_METADATA"
    release_text="$release"
    announce_template=openssl-announce-release.tmpl
fi
tag="openssl-$release"
$VERBOSE "== Updated version information to $release"

$VERBOSE "== Updating files with release date for $release : $RELEASE_DATE"
for fixup in "$HERE/dev/release-aux"/fixup-*-release.pl; do
    file="$(basename "$fixup" | sed -e 's|^fixup-||' -e 's|-release\.pl$||')"
    $VERBOSE "> $file"
    RELEASE="$release" RELEASE_TEXT="$release_text" RELEASE_DATE="$RELEASE_DATE" \
        perl -pi $fixup $file
done

$VERBOSE "== Committing updates and tagging"
git add -u
git commit $git_quiet -m "Prepare for release of $release_text"$'\n\nRelease: yes'
if [ -n "$reviewers" ]; then
    addrev --release --nopr $reviewers
fi
echo "Tagging release with tag $tag.  You may need to enter a pass phrase"
git tag$tagkey "$tag" -m "OpenSSL $release release tag"

tarfile=openssl-$release.tar
tgzfile=$tarfile.gz
announce=openssl-$release.txt

echo "== Generating tar, hash and announcement files.  This make take a bit of time"

$VERBOSE "== Making tarfile: $tgzfile"
# Unfortunately, util/mktar.sh does verbose output on STDERR...  for good
# reason, but it means we don't display errors unless --verbose
./util/mktar.sh --tarfile="../$tarfile" 2>&1 \
    | while read L; do $VERBOSE "> $L"; done

if ! [ -f "../$tgzfile" ]; then
    echo >&2 "Where did the tarball end up? (../$tgzfile)"
    exit 1
fi

$VERBOSE "== Generating checksums: $tgzfile.sha1 $tgzfile.sha256"
openssl sha1 < "../$tgzfile" | \
    (IFS='='; while read X H; do echo $H; done) > "../$tgzfile.sha1"
openssl sha256 < "../$tgzfile" | \
    (IFS='='; while read X H; do echo $H; done) > "../$tgzfile.sha256"
length=$(wc -c < "../$tgzfile")
sha1hash=$(cat "../$tgzfile.sha1")
sha256hash=$(cat "../$tgzfile.sha256")

$VERBOSE "== Generating announcement text: $announce"
# Hack the announcement template
cat "$HERE/dev/release-aux/$announce_template" \
    | sed -e "s|\\\$release_text|$release_text|g" \
          -e "s|\\\$release|$release|g" \
          -e "s|\\\$series|$SERIES|g" \
          -e "s|\\\$label|$PRE_LABEL|g" \
          -e "s|\\\$tarfile|$tgzfile|" \
          -e "s|\\\$length|$length|" \
          -e "s|\\\$sha1hash|$sha1hash|" \
          -e "s|\\\$sha256hash|$sha256hash|" \
    | perl -p "$HERE/dev/release-aux/fix-title.pl" \
    > "../$announce"

$VERBOSE "== Generating signatures: $tgzfile.asc $announce.asc"
rm -f "../$tgzfile.asc" "../$announce.asc"
echo "Signing the release files.  You may need to enter a pass phrase"
gpg$gpgkey --use-agent -sba "../$tgzfile"
gpg$gpgkey --use-agent -sta --clearsign "../$announce"

# Push everything to the parent repo
$VERBOSE "== Push what we have to the parent repository"
git push --follow-tags parent HEAD

if $do_upload; then
    (
        if [ "$VERBOSE" != ':' ]; then
            echo "progress"
        fi
        echo "put ../$tgzfile"
        echo "put ../$tgzfile.sha1"
        echo "put ../$tgzfile.sha256"
        echo "put ../$tgzfile.asc"
        echo "put ../$announce.asc"
    ) \
    | sftp "$upload_address"
fi

# Post-release #######################################################

$VERBOSE "== Reset all files to their pre-release contents"
git reset $git_quiet HEAD^ -- .
git checkout -- .

prev_release_text="$release_text"
prev_release_date="$RELEASE_DATE"

next_release_state "$next_method2"
set_version

release="$VERSION$_PRE_RELEASE_TAG$_BUILD_METADATA"
release_text="$VERSION$_BUILD_METADATA"
if [ -n "$PRE_LABEL" ]; then
    release_text="$SERIES$_BUILD_METADATA $PRE_LABEL $PRE_NUM"
fi
$VERBOSE "== Updated version information to $release"

$VERBOSE "== Updating files for $release :"
for fixup in "$HERE/dev/release-aux"/fixup-*-postrelease.pl; do
    file="$(basename "$fixup" | sed -e 's|^fixup-||' -e 's|-postrelease\.pl$||')"
    $VERBOSE "> $file"
    RELEASE="$release" RELEASE_TEXT="$release_text" \
        PREV_RELEASE_TEXT="$prev_release_text" \
        PREV_RELEASE_DATE="$prev_release_date" \
        perl -pi $fixup $file
done

$VERBOSE "== Committing updates"
git add -u
git commit $git_quiet -m "Prepare for $release_text"$'\n\nRelease: yes'
if [ -n "$reviewers" ]; then
    addrev --release --nopr $reviewers
fi

# Push everything to the parent repo
$VERBOSE "== Push what we have to the parent repository"
git push parent HEAD

if $do_branch; then
    $VERBOSE "== Going back to the update branch $tmp_update_branch"
    git checkout $git_quiet "$tmp_update_branch"

    get_version
    next_release_state "minor"
    set_version

    release="$VERSION$_PRE_RELEASE_TAG$_BUILD_METADATA"
    release_text="$SERIES$_BUILD_METADATA"
    $VERBOSE "== Updated version information to $release"

    $VERBOSE "== Updating files for $release :"
    for fixup in "$HERE/dev/release-aux"/fixup-*-postrelease.pl; do
        file="$(basename "$fixup" | sed -e 's|^fixup-||' -e 's|-postrelease\.pl$||')"
        $VERBOSE "> $file"
        RELEASE="$release" RELEASE_TEXT="$release_text" \
            perl -pi $fixup $file
    done

    $VERBOSE "== Committing updates"
    git add -u
    git commit $git_quiet -m "Prepare for $release_text"$'\n\nRelease: yes'
    if [ -n "$reviewers" ]; then
        addrev --release --nopr $reviewers
    fi
fi

# Push everything to the parent repo
$VERBOSE "== Push what we have to the parent repository"
git push parent HEAD

# Done ###############################################################

$VERBOSE "== Done"

cd $HERE
cat <<EOF

======================================================================
The release is done, and involves a few files and commits for you to
deal with.  Everything you need has been pushed to your repository,
please see instructions that follow.
======================================================================

EOF

if $do_release; then
    cat <<EOF

The following files were uploaded to $upload_address, please ensure they
are dealt with appropriately:

    $tgzfile
    $tgzfile.sha1
    $tgzfile.sha256
    $tgzfile.asc
    $announce.asc
EOF
fi

cat <<EOF

----------------------------------------------------------------------
EOF

if $do_branch; then
    cat <<EOF
You need to prepare the main repository with a new branch, '$release_branch'.
That is done directly in the server's bare repository like this:

    git branch $release_branch $orig_HEAD

Two additional release branches have been added to your repository.
Push them to github, make PRs from them and have them approved:

    $tmp_update_branch
    $tmp_release_branch

When merging them into the main repository, do it like this:

    git push git@github.openssl.org:openssl/openssl.git \\
        $tmp_release_branch:$release_branch
    git push git@github.openssl.org:openssl/openssl.git \\
        $tmp_update_branch:$update_branch
    git push git@github.openssl.org:openssl/openssl.git \\
        $tag
EOF
else
cat <<EOF
One additional release branch has been added to your repository.
Push it to github, make a PR from it and have it approved:

    $tmp_release_branch

When merging it into the main repository, do it like this:

    git push git@github.openssl.org:openssl/openssl.git \\
        $tmp_release_branch:$release_branch
    git push git@github.openssl.org:openssl/openssl.git \\
        $tag
EOF
fi

cat <<EOF

----------------------------------------------------------------------
EOF

cat <<EOF

When everything is done, or if something went wrong and you want to start
over, simply clean away temporary things left behind:

The release worktree:

    rm -rf $release_clone
EOF

if $do_branch; then
    cat <<EOF

The additional release branches:

    git branch -D $tmp_release_branch
    git branch -D $tmp_update_branch
EOF
else
    cat <<EOF

The temporary release branch:

    git branch -D $tmp_release_branch
EOF
fi

exit 0

# cat is inconsequential, it's only there to fend off zealous shell parsers
# that parse all the way here.
cat <<EOF
### BEGIN MANUAL
=pod

=head1 NAME

release.sh - OpenSSL release script

=head1 SYNOPSIS

B<release.sh>
[
B<--alpha> |
B<--next-beta> |
B<--beta> |
B<--final> |
B<--branch> |
B<--local-user>=I<keyid> |
B<--reviewer>=I<id> |
B<--no-upload> |
B<--no-update> |
B<--verbose> |
B<--debug> |
B<--help> |
B<--manual>
]

=head1 DESCRIPTION

B<release.sh> creates an OpenSSL release, given current worktree conditions.
It will refuse to work unless the current branch is C<master> or a release
branch (see L</RELEASE BRANCHES AND TAGS> below for a discussion on those).

B<release.sh> tries to be smart and figure out the next release if no hints
are given through options, and will exit with an error in ambiguous cases.

B<release.sh> finishes off with instructions on what to do next.  When
finishing commands are given, they must be followed exactly.

B<release.sh> leaves behind a clone of the local workspace, as well as one
or two branches in the local repository.  These will be mentioned and can
safely be removed after all instructions have been successfully followed.

=head1 OPTIONS

=over 4

=item B<--alpha>, B<--beta>

Set the state of this branch to indicate that alpha or beta releases are
to be done.

B<--alpha> is only acceptable if the I<PATCH> version number is zero and
the current state is "in development" or that alpha releases are ongoing.

B<--beta> is only acceptable if the I<PATCH> version number is zero and
that alpha or beta releases are ongoing.

=item B<--next-beta>

Use together with B<--alpha> to switch to beta releases after the current
release is done.

=item B<--final>

Set the state of this branch to indicate that regular releases are to be
done.  This is only valid if alpha or beta releases are currently ongoing.

This implies B<--branch>.

=item B<--branch>

Create a branch specific for the I<SERIES> release series, if it doesn't
already exist, and switch to it.  The exact branch name will be
C<< openssl-I<SERIES> >>.

=item B<--no-upload>

Don't upload the produced files.

=item B<--no-update>

Don't run C<make update> and C<make update-fips-checksums>.

=item B<--verbose>

Verbose output.

=item B<--debug>

Display extra debug output.  Implies B<--no-upload>

=item B<--local-user>=I<keyid>

Use I<keyid> as the local user for C<git tag> and for signing with C<gpg>.

If not given, then the default e-mail address' key is used.

=item B<--reviewer>=I<id>

Add I<id> to the set of reviewers for the commits performed by this script.
Multiple reviewers are allowed.

If no reviewer is given, you will have to run C<addrev> manually, which
means retagging a release commit manually as well.

=item B<--force>

Force execution.  Precisely, the check that the current branch is C<master>
or a release branch is not done.

=item B<--help>

Display a quick help text and exit.

=item B<--manual>

Display this manual and exit.

=back

=head1 RELEASE BRANCHES AND TAGS

Prior to OpenSSL 3.0, the release branches were named
C<< OpenSSL_I<SERIES>-stable >>, and the release tags were named
C<< OpenSSL_I<VERSION> >> for regular releases, or
C<< OpenSSL_I<VERSION>-preI<n> >> for pre-releases.

From OpenSSL 3.0 ongoing, the release branches are named
C<< openssl-I<SERIES> >>, and the release tags are named
C<< openssl-I<VERSION> >> for regular releases, or
C<< openssl-I<VERSION>-alphaI<n> >> for alpha releases
and C<< openssl-I<VERSION>-betaI<n> >> for beta releases.

B<release.sh> recognises both forms.

=head1 VERSION AND STATE

With OpenSSL 3.0, all the version and state information is in the file
F<VERSION.dat>, where the following variables are used and changed:

=over 4

=item B<MAJOR>, B<MINOR>, B<PATCH>

The three part of the version number.

=item B<PRE_RELEASE_TAG>

The indicator of the current state of the branch.  The value may be one pf:

=over 4

=item C<dev>

This branch is "in development".  This is typical for the C<master> branch
unless there are ongoing alpha or beta releases.

=item C<< alphaI<n> >> or C<< alphaI<n>-dev >>

This branch has alpha releases going on.  C<< alphaI<n>-dev >> is what
should normally be seen in the git workspace, indicating that
C<< alphaI<n> >> is in development.  C<< alphaI<n> >> is what should be
found in the alpha release tar file.

=item C<< alphaI<n> >> or C<< alphaI<n>-dev >>

This branch has beta releases going on.  The details are otherwise exactly
as for alpha.

=item I<no value>

This is normally not seen in the git workspace, but should always be what's
found in the tar file of a regular release.

=back

=item B<BUILD_METADATA>

Extra build metadata to be used by anyone for their own purposes.

=item B<RELEASE_DATE>

This is normally empty in the git workspace, but should always have the
release date in the tar file of any release.

=back

=head1 COPYRIGHT

Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.

Licensed under the Apache License 2.0 (the "License").  You may not use
this file except in compliance with the License.  You can obtain a copy
in the file LICENSE in the source distribution or at
L<https://www.openssl.org/source/license.html>.

=cut
### END MANUAL
EOF
