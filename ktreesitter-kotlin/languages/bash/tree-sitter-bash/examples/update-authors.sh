#!/bin/sh

git log --reverse --format='%aN <%aE>' | perl -wnE '
BEGIN {
  say "# Authors sorted by whether or not they\x27re me";
}

print $seen{$_} = $_ unless $seen{$_}
' > AUTHORS
