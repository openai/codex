#!/usr/bin/env python
# -*- coding: utf-8
#
# Copyright 2015 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This script uses the following Unicode tables:
# - auxiliary/GraphemeBreakTest.txt
# - auxiliary/WordBreakTest.txt
#
# Since this should not require frequent updates, we just store this
# out-of-line and check the unicode.rs file into git.
from __future__ import print_function

import unicode, re, os, fileinput

def load_test_data(f, optsplit=[]):
    testRe1 = re.compile(r"^÷\s+([^\s].*[^\s])\s+÷\s+#\s+÷\s+\[0.2\].*?([÷×].*)\s+÷\s+\[0.3\]\s*$")

    unicode.fetch(f)
    data = []
    for line in fileinput.input(os.path.basename(f)):
        # lines that include a test start with the ÷ character
        if len(line) < 2 or not line.startswith('÷'):
            continue

        m = testRe1.match(line)
        if not m:
            print("error: no match on line where test was expected: %s" % line)
            continue

        # process the characters in this test case
        chars = process_split_string(m.group(1))
        # skip test case if it contains invalid characters (viz., surrogates)
        if not chars:
            continue

        # now process test cases
        (chars, info) = process_split_info(m.group(2), chars, optsplit)

        # make sure that we have break info for each break!
        assert len(chars) - 1 == len(info)

        data.append((chars, info))

    return data

def process_split_info(s, c, o):
    outcs = []
    outis = []
    workcs = c.pop(0)

    # are we on a × or a ÷?
    isX = False
    if s.startswith('×'):
        isX = True

    # find each instance of '(÷|×) [x.y] '
    while s:
        # find the currently considered rule number
        sInd = s.index('[') + 1
        eInd = s.index(']')

        # if it's '× [a.b]' where 'a.b' is in o, then
        # we consider it a split even though it's not
        # marked as one
        # if it's ÷ then it's always a split
        if not isX or s[sInd:eInd] in o:
            outis.append(s[sInd:eInd])
            outcs.append(workcs)
            workcs = c.pop(0)
        else:
            workcs.extend(c.pop(0))

        idx = 1
        while idx < len(s):
            if s[idx:].startswith('×'):
                isX = True
                break
            if s[idx:].startswith('÷'):
                isX = False
                break
            idx += 1
        s = s[idx:]

    outcs.append(workcs)
    return (outcs, outis)

def process_split_string(s):
    outls = []
    workls = []

    inls = s.split()

    for i in inls:
        if i == '÷' or i == '×':
            outls.append(workls)
            workls = []
            continue

        ival = int(i,16)

        if unicode.is_surrogate(ival):
            return []

        workls.append(ival)

    if workls:
        outls.append(workls)

    return outls

def showfun(x):
    outstr = '("'
    for c in x[0]:
        outstr += "\\u{%x}" % c
    outstr += '",&['
    xfirst = True
    for xx in x[1:]:
        if not xfirst:
            outstr += '],&['
        xfirst = False
        sfirst = True
        for sp in xx:
            if not sfirst:
                outstr += ','
            sfirst = False
            outstr += '"'
            for c in sp:
                outstr += "\\u{%x}" % c
            outstr += '"'
    outstr += '])'
    return outstr

def create_grapheme_data(f):
    # rules 9.1, 9.2, and 9.3 are for extended graphemes only
    optsplits = ['9.1', '9.2', '9.3']
    d = load_test_data("auxiliary/GraphemeBreakTest.txt", optsplits)

    test_same = []
    test_diff = []

    for (c, i) in d:
        allchars = [cn for s in c for cn in s]
        extgraphs = []
        extwork = []

        extwork.extend(c[0])
        for n in range(0,len(i)):
            if i[n] in optsplits:
                extwork.extend(c[n+1])
            else:
                extgraphs.append(extwork)
                extwork = []
                extwork.extend(c[n+1])

        # these are the extended grapheme clusters
        extgraphs.append(extwork)

        if extgraphs == c:
            test_same.append((allchars, c))
        else:
            test_diff.append((allchars, extgraphs, c))

    stype = "&[(&str, &[&str])]"
    dtype = "&[(&str, &[&str], &[&str])]"
    f.write("    // official Unicode test data\n")
    f.write("    // http://www.unicode.org/Public/%s/ucd/auxiliary/GraphemeBreakTest.txt\n" % unicode.UNICODE_VERSION_NUMBER)
    unicode.emit_table(f, "TEST_SAME", test_same, stype, True, showfun, True)
    unicode.emit_table(f, "TEST_DIFF", test_diff, dtype, True, showfun, True)

def create_words_data(f):
    d = load_test_data("auxiliary/WordBreakTest.txt")

    test = []

    for (c, i) in d:
        allchars = [cn for s in c for cn in s]
        test.append((allchars, c))

    wtype = "&[(&str, &[&str])]"
    f.write("    // official Unicode test data\n")
    f.write("    // http://www.unicode.org/Public/%s/ucd/auxiliary/WordBreakTest.txt\n" % unicode.UNICODE_VERSION_NUMBER)
    unicode.emit_table(f, "TEST_WORD", test, wtype, True, showfun, True)

def create_sentence_data(f):
    d = load_test_data("auxiliary/SentenceBreakTest.txt")

    test = []

    for (c, i) in d:
        allchars = [cn for s in c for cn in s]
        test.append((allchars, c))

    wtype = "&[(&str, &[&str])]"
    f.write("    // official Unicode test data\n")
    f.write("    // http://www.unicode.org/Public/%s/ucd/auxiliary/SentenceBreakTest.txt\n" % unicode.UNICODE_VERSION_NUMBER)
    unicode.emit_table(f, "TEST_SENTENCE", test, wtype, True, showfun, True)

if __name__ == "__main__":
    with open("testdata.rs", "w") as rf:
        rf.write(unicode.preamble)
        create_grapheme_data(rf)
        create_words_data(rf)
        create_sentence_data(rf)
