#!perl
#
# Test for RT Bug 29928 fix
# https://rt.cpan.org/Public/Bug/Display.html?id=29928

use strict;
use warnings;
use Test::More tests => 2;

use_ok 'Text::Template::Preprocess' or exit 1;

my $tin = q{The value of $foo is: {$foo}.};

sub tester {
    1;    # dummy preprocessor to cause the bug described.
}

my $tmpl1 = Text::Template::Preprocess->new(TYPE => 'STRING', SOURCE => $tin);

$tmpl1->compile;

my $t1 = $tmpl1->fill_in(
    HASH         => { foo => 'things' },
    PREPROCESSOR => \&tester);

is $t1, 'The value of $foo is: things.';
