#!/usr/bin/env python3
#
# Copyright 2016-2018 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

"""Fuzzing helper, creates and uses corpus/crash directories.

fuzzer.py <fuzzer> <extra fuzzer arguments>
"""

import os
import subprocess
import sys

FUZZER = sys.argv[1]

THIS_DIR = os.path.abspath(os.path.dirname(__file__))
CORPORA_DIR = os.path.abspath(os.path.join(THIS_DIR, "corpora"))

FUZZER_DIR = os.path.abspath(os.path.join(CORPORA_DIR, FUZZER))
if not os.path.isdir(FUZZER_DIR):
    os.mkdir(FUZZER_DIR)

corpora = []

def _create(d):
    dd = os.path.abspath(os.path.join(CORPORA_DIR, d))
    if not os.path.isdir(dd):
        os.mkdir(dd)
    corpora.append(dd)

def _add(d):
    dd = os.path.abspath(os.path.join(CORPORA_DIR, d))
    if os.path.isdir(dd):
        corpora.append(dd)

def main():
    _create(FUZZER)
    _create(FUZZER + "-crash")
    _add(FUZZER + "-seed")

    cmd = ([os.path.abspath(os.path.join(THIS_DIR, FUZZER))]  + sys.argv[2:]
           + ["-artifact_prefix=" + corpora[1] + "/"] + corpora)
    print(" ".join(cmd))
    subprocess.call(cmd)

if __name__ == "__main__":
    main()
