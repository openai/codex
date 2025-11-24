#!/usr/bin/env python
# Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# A python program written to parse (version 1) of the WYCHEPROOF test vectors for
# ML_DSA. The 6 files that can be processed by this utility can be downloaded
# from
#  https://github.com/C2SP/wycheproof/blob/8e7fa6f87e6993d7b613cf48b46512a32df8084a/testvectors_v1/mldsa_*_standard_*_test.json")
# and output from this utility to
# test/recipes/30-test_evp_data/evppkey_ml_dsa_44_wycheproof_sign.txt
# test/recipes/30-test_evp_data/evppkey_ml_dsa_65_wycheproof_sign.txt
# test/recipes/30-test_evp_data/evppkey_ml_dsa_87_wycheproof_sign.txt
# test/recipes/30-test_evp_data/evppkey_ml_dsa_44_wycheproof_verify.txt
# test/recipes/30-test_evp_data/evppkey_ml_dsa_65_wycheproof_verify.txt
# test/recipes/30-test_evp_data/evppkey_ml_dsa_87_wycheproof_verify.txt
#
# e.g. python3 ./test/mldsa_wycheproof_parse.py -alg ML-DSA-44 ./wycheproof/testvectors_v1/mldsa_44_standard_sign_test.json > test/recipes/30-test_evp_data/evppkey_ml_dsa_44_wycheproof_sign.txt

import json
import argparse
import datetime
from _ast import Or

def print_label(label, value):
    print(label + " = " + value)

def print_hexlabel(label, tag, value):
    print(label + " = hex" + tag + ":" + value)

def parse_ml_dsa_sig_gen(alg, groups):
    grpId = 1
    for grp in groups:
        keyOnly = False
        first = True
        name = alg.replace('-', '_')
        keyname = name + "_" + str(grpId)
        grpId += 1

        for tst in grp['tests']:
            if first:
                first = False
                if 'flags' in tst:
                    if 'IncorrectPrivateKeyLength' in tst['flags'] or 'InvalidPrivateKey' in tst['flags']:
                        keyOnly = True
                if not keyOnly:
                    print("")
                    print_label("PrivateKeyRaw", keyname + ":" + alg + ":" + grp['privateKey'])
            testname = name + "_" + str(tst['tcId'])
            print("\n# " + str(tst['tcId']) + " " + tst['comment'])

            print_label("FIPSversion", ">=3.5.0")
            if keyOnly:
                print_label("KeyFromData", alg)
                print_hexlabel("Ctrl", "priv", grp['privateKey'])
                print_label("Result", "KEY_FROMDATA_ERROR")
            else:
                print_label("Sign-Message", alg + ":" + keyname)
                print_label("Input", tst['msg'])
                print_label("Output", tst['sig'])
                if 'ctx' in tst:
                    print_hexlabel("Ctrl", "context-string", tst['ctx'])
                print_label("Ctrl", "message-encoding:1")
                print_label("Ctrl", "deterministic:1")
                if tst['result'] == "invalid":
                    print_label("Result", "PKEY_CTRL_ERROR")

def parse_ml_dsa_sig_ver(alg, groups):
    grpId = 1
    for grp in groups:
        keyOnly = False
        first = True
        name = alg.replace('-', '_')
        keyname = name + "_" + str(grpId)
        grpId += 1

        for tst in grp['tests']:
            if first:
                first = False
                if 'flags' in tst:
                    if 'IncorrectPublicKeyLength' in tst['flags'] or 'InvalidPublicKey' in tst['flags']:
                        keyOnly = True
                if not keyOnly:
                    print("")
                    print_label("PublicKeyRaw", keyname + ":" + alg + ":" + grp['publicKey'])
            testname = name + "_" + str(tst['tcId'])
            print("\n# " + str(tst['tcId']) + " " + tst['comment'])

            print_label("FIPSversion", ">=3.5.0")
            if keyOnly:
                print_label("KeyFromData", alg)
                print_hexlabel("Ctrl", "pub", grp['publicKey'])
                print_label("Result", "KEY_FROMDATA_ERROR")
            else:
                print_label("Verify-Message-Public", alg + ":" + keyname)
                print_label("Input", tst['msg'])
                print_label("Output", tst['sig'])
                if 'ctx' in tst:
                    print_hexlabel("Ctrl", "context-string", tst['ctx'])
                print_label("Ctrl", "message-encoding:1")
                print_label("Ctrl", "deterministic:1")
                if tst['result'] == "invalid":
                    if 'InvalidContext' in tst['flags']:
                        print_label("Result", "PKEY_CTRL_ERROR")
                    else:
                        print_label("Result", "VERIFY_ERROR")

parser = argparse.ArgumentParser(description="")
parser.add_argument('filename', type=str)
parser.add_argument('-alg', type=str)
args = parser.parse_args()

# Open and read the JSON file
with open(args.filename, 'r') as file:
    data = json.load(file)

year = datetime.date.today().year
version = data['generatorVersion']
algorithm = data['algorithm']
mode = data['testGroups'][0]['type']

print("# Copyright " + str(year) + " The OpenSSL Project Authors. All Rights Reserved.")
print("#")
print("# Licensed under the Apache License 2.0 (the \"License\").  You may not use")
print("# this file except in compliance with the License.  You can obtain a copy")
print("# in the file LICENSE in the source distribution or at")
print("# https://www.openssl.org/source/license.html\n")
print("# Wycheproof test data for " + algorithm + " " + mode + " generated from")
print("# https://github.com/C2SP/wycheproof/blob/8e7fa6f87e6993d7b613cf48b46512a32df8084a/testvectors_v1/mldsa_*_standard_*_test.json")

print("# [version " + str(version) + "]")

if algorithm == "ML-DSA":
    if mode == 'MlDsaSign':
        parse_ml_dsa_sig_gen(args.alg, data['testGroups'])
    elif mode == 'MlDsaVerify':
        parse_ml_dsa_sig_ver(args.alg, data['testGroups'])
    else:
        print("Unsupported mode " + mode)
else:
    print("Unsupported algorithm " + algorithm)
