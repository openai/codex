#!/bin/bash

set -ex

ANY_CA_PEM=integration-tests/one-existing-ca.pem
ANY_CA_SUBJECT="OU=GlobalSign Root CA - R3, O=GlobalSign, CN=GlobalSign"

reset() {
  security remove-trusted-cert -d $ANY_CA_PEM || true
  list | grep "$ANY_CA_SUBJECT"
}

list() {
  cargo test util_list_certs -- --nocapture 2>/dev/null
}

assert_missing() {
  set +e
  list | grep "$1"
  ret=$?
  set -e
  test $ret -eq 1
}

assert_exists() {
  list | grep "$1" > /dev/null
}

test_distrust_existing_root() {
  assert_exists "$ANY_CA_SUBJECT"
  security add-trusted-cert -d -r deny $ANY_CA_PEM
  assert_missing "$ANY_CA_SUBJECT"
  reset
}

# https://developer.apple.com/forums/thread/671582?answerId=693632022#693632022
security authorizationdb write com.apple.trust-settings.admin allow

reset
test_distrust_existing_root
printf "\n*** All tests passed ***\n"
