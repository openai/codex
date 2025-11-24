#!perl
#
# test apparatus for Text::Template module
# still incomplete.
#

use strict;
use warnings;
use Test::More tests => 4;

use_ok 'Text::Template' or exit 1;

my $templateIN = q{
This line should have a 3: {1+2}

This line should have several numbers:
{ $t = ''; foreach $n (1 .. 20) { $t .= $n . ' ' } $t }
};

my $templateOUT = q{
This line should have a 3: { $OUT = 1+2 }

This line should have several numbers:
{ foreach $n (1 .. 20) { $OUT .= $n . ' ' } }
};

# Build templates from string
my $template = Text::Template->new('type' => 'STRING', 'source' => $templateIN);
isa_ok $template, 'Text::Template';

$templateOUT = Text::Template->new('type' => 'STRING', 'source' => $templateOUT);
isa_ok $templateOUT, 'Text::Template';

# Fill in templates
my $text    = $template->fill_in();
my $textOUT = $templateOUT->fill_in();

# (1) They should be the same
is $text, $textOUT;

# Missing:  Test this feature in Safe compartments;
# it's a totally different code path.
# Decision: Put that into safe.t, because that file should
# be skipped when Safe.pm is unavailable.

exit;
