#!/usr/bin/env python
# Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# A python program written to parse (version 42) of the ACVP test vectors for
# ML_KEM. The 2 files that can be processed by this utility can be downloaded
# from
#  https://github.com/usnistgov/ACVP-Server/blob/master/gen-val/json-files/ML-KEM-keyGen-FIPS203/internalProjection.json
#  https://github.com/usnistgov/ACVP-Server/blob/master/gen-val/json-files/ML-KEM-encapDecap-FIPS203/internalProjection.json
# and output from this utility to
#  test/recipes/30-test_evp_data/evppkey_ml_kem_keygen.txt
#  test/recipes/30-test_evp_data/evppkey_ml_kem_encapdecap.txt
#
# e.g. python3 mlkem_parse.py ~/Downloads/keygen.json > ./test/recipes/30-test_evp_data/evppkey_ml_kem_keygen.txt
#
import json
import argparse
import datetime
import sys

def eprint(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)

def print_label(label, value):
    print(label + " = " + value)

def print_hexlabel(label, tag, value):
    print(label + " = hex" + tag + ":" + value)

def parse_ml_kem_key_gen(groups):
    for grp in groups:
        for tst in grp['tests']:
            print("");
            print_label("FIPSversion", ">=3.5.0")
            print_label("KeyGen", grp['parameterSet'])
            print_label("KeyName", "tcId" + str(tst['tcId']))
            print_hexlabel("Ctrl", "seed", tst['d'] + tst['z'])
            print_hexlabel("CtrlOut", "pub", tst['ek'])
            print_hexlabel("CtrlOut", "priv", tst['dk'])

def parse_ml_kem_encap_decap(groups):
    for grp in groups:
        name = grp['parameterSet'].replace('-', '_')
        function = grp['function']
        if function == "encapsulation":
            for tst in grp['tests']:
                print("");
                print('# tcId = ' + str(tst['tcId']));
                print_label("FIPSversion", ">=3.5.0")
                print_label("Kem", grp['parameterSet'])
                print_label("Entropy", tst['m'])
                print_label("EncodedPublicKey", tst['ek'])
                print_label("Ciphertext", tst['c'])
                print_label("Output", tst['k'])
        elif function == "decapsulation":
            dk = grp['dk']
            for tst in grp['tests']:
                print("");
                print('# tcId = ' + str(tst['tcId']));
                print_label("FIPSversion", ">=3.5.0")
                print_label("Kem", grp['parameterSet'])
                print_label("EncodedPrivateKey", dk)
                print("# " +  tst['reason'])
                print_label("Input", tst['c'])
                print_label("Output", tst['k'])
        else:
            eprint("Unsupported function " + function)

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
revision = data['revision']

print("# Copyright " + str(year) + " The OpenSSL Project Authors. All Rights Reserved.")
print("#")
print("# Licensed under the Apache License 2.0 (the \"License\").  You may not use")
print("# this file except in compliance with the License.  You can obtain a copy")
print("# in the file LICENSE in the source distribution or at")
print("# https://www.openssl.org/source/license.html\n")
print("# ACVP test data for " + algorithm + " " + mode + " generated from")
print("# https://github.com/usnistgov/ACVP-Server/blob/master/gen-val/json-files/"
      + algorithm + "-" + mode + "-" + revision + "/internalProjection.json")
print("# [version " + str(version) + "]")

print("")
print_label("Title", algorithm + " " + mode + " ACVP Tests")

if algorithm == "ML-KEM":
    if mode == "keyGen":
        parse_ml_kem_key_gen(data['testGroups'])
    elif mode == "encapDecap":
        parse_ml_kem_encap_decap(data['testGroups'])
    else:
        eprint("Unsupported mode " + mode)
else:
    eprint("Unsupported algorithm " + algorithm)
