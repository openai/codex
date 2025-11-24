Auxiliary files for dev/release.sh
==================================

- release-state-fn.sh

  This is the main version and state update logic...  you could say
  that it's the innermost engine for the release mechanism.  It
  tries to be agnostic of versioning schemes, and relies on
  release-version-fn.sh to supply necessary functions that are
  specific for versioning schemes.

- release-version-fn.sh

  Supplies functions that are specific to versioning schemes:

  get_version() gets the version data from appropriate files.

  set_version() writes the version data to appropriate files.

  fixup_version() updates the version data, given a first argument
  that instructs it what update to do.

- openssl-announce-pre-release.tmpl and openssl-announce-release.tmpl

  Templates for announcements

- fixup-*-release.pl and fixup-*-postrelease.pl

  Fixup scripts for specific files, to be done for the release
  commit and for the post-release commit.
