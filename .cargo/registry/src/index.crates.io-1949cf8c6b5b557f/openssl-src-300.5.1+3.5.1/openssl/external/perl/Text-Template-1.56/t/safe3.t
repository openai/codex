#!perl
#
# test apparatus for Text::Template module

use strict;
use warnings;
use Test::More;

unless (eval { require Safe; 1 }) {
    plan skip_all => 'Safe.pm is required for this test';
}
else {
    plan tests => 4;
}

use_ok 'Text::Template' or exit 1;

# Test the OUT feature with safe compartments

my $template = q{
This line should have a 3: {1+2}

This line should have several numbers:
{ $t = ''; foreach $n (1 .. 20) { $t .= $n . ' ' } $t }
};

my $templateOUT = q{
This line should have a 3: { $OUT = 1+2 }

This line should have several numbers:
{ foreach $n (1 .. 20) { $OUT .= $n . ' ' } }
};

my $c = Safe->new;

# Build templates from string
$template = Text::Template->new(
    type   => 'STRING',
    source => $template,
    SAFE   => $c) or die;

$templateOUT = Text::Template->new(
    type   => 'STRING',
    source => $templateOUT,
    SAFE    => $c) or die;

# Fill in templates
my $text = $template->fill_in()
    or die;
my $textOUT = $templateOUT->fill_in()
    or die;

# (1) They should be the same
is $text, $textOUT;

# (2-3)  "Joel Appelbaum" <joel@orbz.com> <000701c0ac2c$aed1d6e0$0201a8c0@prime>
# "Contrary to the documentation the $OUT variable is not always
# undefined at the start of each program fragment.  The $OUT variable
# is never undefined after it is used once if you are using the SAFE
# option.  The result is that every fragment after the fragment that
# $OUT was used in is replaced by the old $OUT value instead of the
# result of the fragment.  This holds true even after the
# Text::Template object goes out of scope and a new one is created!"
#
# Also reported by Daini Xie.

{
    my $template = q{{$OUT = 'x'}y{$OUT .= 'z'}};
    my $expected = "xyz";
    my $s        = Safe->new;
    my $o        = Text::Template->new(
        type   => 'string',
        source => $template);

    for (1 .. 2) {
        my $r = $o->fill_in(SAFE => $s);

        is $r, $expected;
    }
}
