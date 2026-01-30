#!/usr/bin/python3
# Copyright 2022 Collabora Ltd.
# SPDX-License-Identifier: LGPL-2.0-or-later

import subprocess
import sys

if __name__ == '__main__':
    completed = subprocess.run(
        ['objdump', '-T', '-x', sys.argv[1]],
        stdout=subprocess.PIPE,
    )
    stdout = completed.stdout
    assert stdout is not None
    seen_rpath = False

    for line in stdout.splitlines():
        words = line.strip().split()

        if words and words[0] in (b'RPATH', b'RUNPATH'):
            print(line.decode(errors='backslashreplace'))
            assert len(words) == 2, words
            assert words[1] == b'${ORIGIN}/../lib', words
            seen_rpath = True

    assert seen_rpath
