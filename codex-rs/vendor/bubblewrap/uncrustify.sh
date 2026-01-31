#!/bin/sh
uncrustify -c uncrustify.cfg --no-backup `git ls-tree --name-only -r HEAD | grep \\\.[ch]$`
