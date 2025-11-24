#! /usr/bin/env perl
#
# TEST c-compress-pl with a number of examples and what should happen to them

use strict;
use warnings;

use File::Basename;

my @pairs =
    (
     [ <<'_____'
/* A hell of a program */
#def\
ine foo/* bar */ 3
#define bar /* haha "A /* comment */ that should    /* remain" */
#define  haha /* hoho */ "A /* comment */ that should /* remain" */

int main() {
    int x;
    /* one lonely comment */
}
_____
       , <<'_____'
#define foo 3
#define bar that should
#define haha "A /* comment */ that should /* remain" */
int main() {
int x;
}
_____
     ]
    );

my $here = dirname $0;
my $c_compress = "$here/lang-compress.pl";

use FileHandle;
use IPC::Open2;
use Text::Diff;
foreach (@pairs) {
    my $source = $_->[0];
    my $expected = $_->[1];
    my $pid = open2(\*Reader, \*Writer, "perl $c_compress 'C'");
    print Writer $source;
    close Writer;

    local $/ = undef;             # slurp
    my $got = <Reader>;

    if ($got ne $expected) {
        print "MISMATCH:\n", diff \$expected, \$got;
    }
}
