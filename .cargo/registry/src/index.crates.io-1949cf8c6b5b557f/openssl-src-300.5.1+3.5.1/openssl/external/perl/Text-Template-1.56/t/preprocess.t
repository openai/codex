#!perl
#
# Tests for PREPROCESSOR features
# These tests first appeared in version 1.25.

use strict;
use warnings;
use Test::More tests => 9;
use File::Temp;

use_ok 'Text::Template::Preprocess' or exit 1;

my $tmpfile = File::Temp->new;
my $TMPFILE = $tmpfile->filename;

my $py = sub { tr/x/y/ };
my $pz = sub { tr/x/z/ };

my $t    = 'xxx The value of $x is {$x}';
my $outx = 'xxx The value of $x is 119';
my $outy = 'yyy The value of $y is 23';
my $outz = 'zzz The value of $z is 5';
open my $tfh, '>', $TMPFILE or die "Couldn't open test file: $!; aborting";
print $tfh $t;
close $tfh;

my @result = ($outx, $outy, $outz, $outz);
for my $trial (1, 0) {
    for my $test (0 .. 3) {
        my $tmpl;
        if ($trial == 0) {
            $tmpl = Text::Template::Preprocess->new(TYPE => 'STRING', SOURCE => $t) or die;
        }
        else {
            open $tfh, '<', $TMPFILE or die "Couldn't open test file: $!; aborting";
            $tmpl = Text::Template::Preprocess->new(TYPE => 'FILEHANDLE', SOURCE => $tfh) or die;
        }
        $tmpl->preprocessor($py) if ($test & 1) == 1;
        my @args = ((($test & 2) == 2) ? (PREPROCESSOR => $pz) : ());
        my $o = $tmpl->fill_in(@args, HASH => { x => 119, 'y' => 23, z => 5 });
        is $o, $result[$test];
    }
}
