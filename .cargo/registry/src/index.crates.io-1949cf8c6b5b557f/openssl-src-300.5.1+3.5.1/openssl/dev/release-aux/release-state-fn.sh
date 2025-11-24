#! /bin/sh
# Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# This will increase the version number and pre-release tag, according to the
# current state of the source tree, and the function's first argument (called
# |next| internally), which is how the caller tells what the next step should
# be.
#
# The possible current source tree states are:
# ''            The source is in a released state.
# 'dev'         The source is in development.  This is the normal state.
# 'alpha', 'alphadev'
#               The source is undergoing a series of alpha releases.
# 'beta', 'betadev'
#               The source is undergoing a series of beta releases.
# These states are computed from $PRE_LABEL and $TYPE
#
# The possible |next| values are:
# 'alpha'       The source tree should move to an alpha release state, or
#               stay there.  This trips the alpha / pre-release counter.
# 'beta'        The source tree should move to a beta release state, or
#               stay there.  This trips the beta / pre-release counter.
# 'final'       The source tree should move to a final release (assuming it's
#               currently in one of the alpha or beta states).  This turns
#               off the alpha or beta states.
# ''            The source tree should move to the next release.  The exact
#               meaning depends on the current source state.  It may mean
#               tripping the alpha / beta / pre-release counter, or increasing
#               the PATCH number.
#
# 'minor'       The source tree should move to the next minor version.  This
#               should only be used in the master branch when a release branch
#               has been created.
#
# This function expects there to be a function called fixup_version(), which
# SHOULD take the |next| as first argument, and SHOULD increase the label
# counter or the PATCH number accordingly, but only when the current
# state is "in development".

next_release_state () {
    local next="$1"
    local today="$(date '+%-d %b %Y')"
    local retry=true

    local before="$PRE_LABEL$TYPE"

    while $retry; do
        retry=false

        $DEBUG >&2 "DEBUG[next_release_state]: BEGIN: \$before=$before"
        $DEBUG >&2 "DEBUG[next_release_state]: BEGIN: \$next=$next"
        $DEBUG >&2 "DEBUG[next_release_state]: BEGIN: \$MAJOR=$MAJOR"
        $DEBUG >&2 "DEBUG[next_release_state]: BEGIN: \$MINOR=$MINOR"
        $DEBUG >&2 "DEBUG[next_release_state]: BEGIN: \$PATCH=$PATCH"
        $DEBUG >&2 "DEBUG[next_release_state]: BEGIN: \$TYPE=$TYPE"
        $DEBUG >&2 "DEBUG[next_release_state]: BEGIN: \$PRE_LABEL=$PRE_LABEL"
        $DEBUG >&2 "DEBUG[next_release_state]: BEGIN: \$PRE_NUM=$PRE_NUM"
        $DEBUG >&2 "DEBUG[next_release_state]: BEGIN: \$RELEASE_DATE=$RELEASE_DATE"

        case "$before+$next" in
            # MAKING ALPHA RELEASES ##################################

            # Alpha releases can't be made from beta versions or real versions
            beta*+alpha | +alpha )
                echo >&2 "Invalid state for an alpha release"
                echo >&2 "Try --beta or --final, or perhaps nothing"
                exit 1
                ;;
            # For alpha releases, the tag update is dev => alpha or
            # alpha dev => alpha for the release itself, and
            # alpha => alpha dev for post release.
            dev+alpha | alphadev+alpha )
                TYPE=
                RELEASE_DATE="$today"
                fixup_version "alpha"
                ;;
            alpha+alpha )
                TYPE=dev
                RELEASE_DATE=
                fixup_version "alpha"
                ;;

            # MAKING BETA RELEASES ###################################

            # Beta releases can't be made from real versions
            +beta )
                echo >&2 "Invalid state for beta release"
                echo >&2 "Try --final, or perhaps nothing"
                exit 1
                ;;
            # For beta releases, the tag update is dev => beta1, or
            # alpha{n}-dev => beta1 when transitioning from alpha to
            # beta, or beta{n}-dev => beta{n} for the release itself,
            # or beta{n} => beta{n+1}-dev for post release.
            dev+beta | alphadev+beta | betadev+beta )
                TYPE=
                RELEASE_DATE="$today"
                fixup_version "beta"
                ;;
            beta+beta )
                TYPE=dev
                RELEASE_DATE=
                fixup_version "beta"
                ;;
            # It's possible to switch from alpha to beta in the
            # post release.  That's what --next-beta does.
            alpha+beta )
                TYPE=dev
                RELEASE_DATE=
                fixup_version "beta"
                ;;

            # MAKING FINAL RELEASES ##################################

            # Final releases can't be made from the main development branch
            dev+final)
                echo >&2 "Invalid state for final release"
                echo >&2 "This should have been preceded by an alpha or a beta release"
                exit 1
                ;;
            # For final releases, the starting point must be a dev state
            alphadev+final | betadev+final )
                TYPE=
                RELEASE_DATE="$today"
                fixup_version "final"
                ;;
            # The final step of a final release is to switch back to
            # development
            +final )
                TYPE=dev
                RELEASE_DATE=
                fixup_version "final"
                ;;

            # SWITCHING TO THE NEXT MINOR RELEASE ####################

            *+minor )
                TYPE=dev
                RELEASE_DATE=
                fixup_version "minor"
                ;;

            # MAKING DEFAULT RELEASES ################################

            # If we're coming from a non-dev, simply switch to dev.
            # fixup_version() should trip up the PATCH number.
            + )
                TYPE=dev
                fixup_version ""
                ;;

            # If we're coming from development, switch to non-dev, unless
            # the PATCH number is zero.  If it is, we force the caller to
            # go through the alpha and beta release process.
            dev+ )
                if [ "$PATCH" = "0" ]; then
                    echo >&2 "Can't update PATCH version number from 0"
                    echo >&2 "Please use --alpha or --beta"
                    exit 1
                fi
                TYPE=
                RELEASE_DATE="$today"
                fixup_version ""
                ;;

            # If we're currently in alpha, we continue with alpha, as if
            # the user had specified --alpha
            alpha*+ )
                next=alpha
                retry=true
                ;;

            # If we're currently in beta, we continue with beta, as if
            # the user had specified --beta
            beta*+ )
                next=beta
                retry=true
                ;;

            *)
                echo >&2 "Invalid combination of options"
                exit 1
                ;;
        esac

        $DEBUG >&2 "DEBUG[next_release_state]: END: \$before=$before"
        $DEBUG >&2 "DEBUG[next_release_state]: END: \$next=$next"
        $DEBUG >&2 "DEBUG[next_release_state]: END: \$MAJOR=$MAJOR"
        $DEBUG >&2 "DEBUG[next_release_state]: END: \$MINOR=$MINOR"
        $DEBUG >&2 "DEBUG[next_release_state]: END: \$PATCH=$PATCH"
        $DEBUG >&2 "DEBUG[next_release_state]: END: \$TYPE=$TYPE"
        $DEBUG >&2 "DEBUG[next_release_state]: END: \$PRE_LABEL=$PRE_LABEL"
        $DEBUG >&2 "DEBUG[next_release_state]: END: \$PRE_NUM=$PRE_NUM"
        $DEBUG >&2 "DEBUG[next_release_state]: END: \$RELEASE_DATE=$RELEASE_DATE"
    done
}
