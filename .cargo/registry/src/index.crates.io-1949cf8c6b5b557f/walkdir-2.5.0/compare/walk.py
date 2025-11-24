from __future__ import absolute_import, division, print_function

import os
import sys

for dirpath, dirnames, filenames in os.walk(sys.argv[1]):
    for n in dirnames:
        print(os.path.join(dirpath, n))
    for n in filenames:
        print(os.path.join(dirpath, n))
