#!/usr/bin/env python3
#
# Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# A python program written to parse (version 42) of the ACVP test vectors for
# SLH_DSA. The 3 files that can be processed by this utility can be downloaded
# from
#  https://github.com/usnistgov/ACVP-Server/blob/master/gen-val/json-files/SLH-DSA-keyGen-FIPS204/internalProjection.json
#  https://github.com/usnistgov/ACVP-Server/blob/master/gen-val/json-files/SLH-DSA-sigGen-FIPS204/internalProjection.json
#  https://github.com/usnistgov/ACVP-Server/blob/master/gen-val/json-files/SLH-DSA-sigVer-FIPS204/internalProjection.json
# and output from this utility to
#  test/recipes/30-test_evp_data/evppkey_slh_dsa_keygen.txt
#  test/recipes/30-test_evp_data/evppkey_slh_dsa_siggen.txt
#  test/recipes/30-test_evp_data/evppkey_slh_dsa_sigver.txt
#
# e.g. python3 slhdsa_parse.py ~/Downloads/keygen.json > ./test/recipes/30-test_evp_data/evppkey_slh_dsa_keygen.txt
#
import json
import argparse
import datetime
import re
import sys

def eprint(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)

def print_label(label, value):
    print(label + " = " + value)

def print_hexlabel(label, tag, value):
    print(label + " = hex" + tag + ":" + value)

def parse_slh_dsa_key_gen(groups):
    for grp in groups:
        name = grp['parameterSet'].replace('-', '_')
        for tst in grp['tests']:
            print("");
            print_label("FIPSversion", ">=3.5.0")
            print_label("KeyGen", grp['parameterSet'])
            print_label("KeyName", "tcId" + str(tst['tcId']))
            print_hexlabel("Ctrl", "seed", tst['skSeed'] + tst['skPrf'] + tst['pkSeed'])
            print_hexlabel("CtrlOut", "pub", tst['pk'])
            print_hexlabel("CtrlOut", "priv", tst['sk'])

def parse_slh_dsa_sig_gen(groups):
    coverage = set()
    for grp in groups:
        name = grp['parameterSet'].replace('-', '_')
        encoding = "1" if grp['preHash'] != 'none' else "0"
        deterministic = "1" if grp['deterministic'] else "0"
        for tst in grp['tests']:
            if tst['hashAlg'] != 'none':
                continue

            # Check if there is a similar test already and skip if so
            # Signature generation tests are very expensive, so we need to
            # limit things substantially.  By default, we only test one of
            # the slow flavours and one of each kind of the fast ones.
            if name.find('f') >= 0:
                context = "1" if 'context' in tst else "0"
                coverage_name = name + "_" + encoding + deterministic + context
            else:
                coverage_name = name
            extended_test = coverage_name in coverage
            coverage.add(coverage_name)

            # Emit test case
            testname = name + "_" + str(tst['tcId'])
            print("");
            print_label("PrivateKeyRaw", testname + ":" + grp['parameterSet'] + ":" + tst['sk'])
            print("");
            print_label("FIPSversion", ">=3.5.0")
            print_label("Sign-Message", grp['parameterSet'] + ":" + testname)
            if extended_test:
                print_label("Extended-Test", "1")
            print_label("Input", tst['message'])
            print_label("Output", tst['signature'])
            if 'additionalRandomness' in tst:
                print_hexlabel("Ctrl", "test-entropy", tst['additionalRandomness']);
            print_label("Ctrl", "deterministic:" + deterministic)
            print_label("Ctrl", "message-encoding:" + encoding)
            if 'context' in tst:
                print_hexlabel("Ctrl", "context-string", tst["context"])

def parse_slh_dsa_sig_ver(groups):
    for grp in groups:
        name = grp['parameterSet'].replace('-', '_')
        encoding = "1" if grp['preHash'] != 'none' else "0"
        for tst in grp['tests']:
            if tst['hashAlg'] != 'none':
                continue
            testname = name + "_" + str(tst['tcId'])
            print("");
            print_label("PublicKeyRaw", testname + ":" + grp['parameterSet'] + ":" + tst['pk'])
            print("");
            if "reason" in tst:
                print("# " + tst['reason'])
            print_label("FIPSversion", ">=3.5.0")
            print_label("Verify-Message-Public", grp['parameterSet'] + ":" + testname)
            print_label("Input", tst['message'])
            print_label("Output", tst['signature'])
            if 'additionalRandomness' in tst:
                print_hexlabel("Ctrl", "test-entropy", tst['additionalRandomness']);
            print_label("Ctrl", "message-encoding:" + encoding)
            if 'context' in tst:
                print_hexlabel("Ctrl", "context-string", tst["context"])
            if not tst['testPassed']:
                print_label("Result", "VERIFY_ERROR")

parser = argparse.ArgumentParser(description="")
parser.add_argument('filename', type=str)
args = parser.parse_args()

# Open and read the JSON file
with open(args.filename, 'r') as file:
    data = json.load(file)

year = datetime.date.today().year
version = data['vsId']
algorithm = data['algorithm']
mode = data['mode']

print("# Copyright " + str(year) + " The OpenSSL Project Authors. All Rights Reserved.")
print("#")
print("# Licensed under the Apache License 2.0 (the \"License\").  You may not use")
print("# this file except in compliance with the License.  You can obtain a copy")
print("# in the file LICENSE in the source distribution or at")
print("# https://www.openssl.org/source/license.html\n")
print("# ACVP test data for " + algorithm + " " + mode + " generated from")
print("# https://github.com/usnistgov/ACVP-Server/blob/master/gen-val/json-files/"
      "SLH-DSA-" + mode + "-FIPS204/internalProjection.json")
print("# [version " + str(version) + "]")

if algorithm == "SLH-DSA":
    if mode == 'sigVer':
        parse_slh_dsa_sig_ver(data['testGroups'])
    elif mode == 'sigGen':
        parse_slh_dsa_sig_gen(data['testGroups'])
    elif mode == 'keyGen':
        parse_slh_dsa_key_gen(data['testGroups'])
    else:
        eprint("Unsupported mode " + mode)
else:
    eprint("Unsupported algorithm " + algorithm)
