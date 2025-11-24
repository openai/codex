#!perl
#
# test apparatus for Text::Template module
# still incomplete.

use strict;
use warnings;

use Test::More;

unless (eval { require Safe; 1 }) {
    plan skip_all => 'Safe.pm is required for this test';
}
else {
    plan tests => 20;
}

use_ok 'Text::Template' or exit 1;

my ($BADOP, $FAILURE);
if ($^O eq 'MacOS') {
    $BADOP   = qq{};
    $FAILURE = q{};
}
else {
    $BADOP   = qq{kill 0};
    $FAILURE = q{Program fragment at line 1 delivered error ``kill trapped by operation mask''};
}

our $v = 119;

my $c = Safe->new or die;

my $goodtemplate = q{This should succeed: { $v }};
my $goodoutput   = q{This should succeed: 119};

my $template1 = Text::Template->new(type => 'STRING', source => $goodtemplate);
my $template2 = Text::Template->new(type => 'STRING', source => $goodtemplate);

my $text1 = $template1->fill_in();
ok defined $text1;

my $text2 = $template1->fill_in(SAFE => $c);
ok defined $text2;

my $text3 = $template2->fill_in(SAFE => $c);
ok defined $text3;

# (4) Safe and non-safe fills of different template objects with the
# same template text should yield the same result.
# print +($text1 eq $text3 ? '' : 'not '), "ok $n\n";
# (4) voided this test:  it's not true, because the unsafe fill
# uses package main, while the safe fill uses the secret safe package.
# We could alias the secret safe package to be identical to main,
# but that wouldn't be safe.  If you want the aliasing, you have to
# request it explicitly with `PACKAGE'.

# (5) Safe and non-safe fills of the same template object
# should yield the same result.
# (5) voided this test for the same reason as #4.
# print +($text1 eq $text2 ? '' : 'not '), "ok $n\n";

# (6) Make sure the output was actually correct
is $text1, $goodoutput;

my $badtemplate     = qq{This should fail: { $BADOP; 'NOFAIL' }};
my $badnosafeoutput = q{This should fail: NOFAIL};
my $badsafeoutput =
    q{This should fail: Program fragment delivered error ``kill trapped by operation mask at template line 1.''};

$template1 = Text::Template->new('type' => 'STRING', 'source' => $badtemplate);
isa_ok $template1, 'Text::Template';

$template2 = Text::Template->new('type' => 'STRING', 'source' => $badtemplate);
isa_ok $template2, 'Text::Template';

# none of these should fail
$text1 = $template1->fill_in();
ok defined $text1;

$text2 = $template1->fill_in(SAFE => $c);
ok defined $text2;

$text3 = $template2->fill_in(SAFE => $c);
ok defined $text3;

my $text4 = $template1->fill_in();
ok defined $text4;

# (11) text1 and text4 should be the same (using safe in between
# didn't change anything.)
is $text1, $text4;

# (12) text2 and text3 should be the same (same template text in different
# objects
is $text2, $text3;

# (13) text1 should yield badnosafeoutput
is $text1, $badnosafeoutput;

# (14) text2 should yield badsafeoutput
$text2 =~ s/'kill'/kill/;    # 5.8.1 added quote marks around the op name
is $text2, $badsafeoutput;

my $template = q{{$x=1}{$x+1}};

$template1 = Text::Template->new('type' => 'STRING', 'source' => $template);
isa_ok $template1, 'Text::Template';

$template2 = Text::Template->new('type' => 'STRING', 'source' => $template);
isa_ok $template2, 'Text::Template';

$text1 = $template1->fill_in();
$text2 = $template1->fill_in(SAFE => Safe->new);

# (15) Do effects persist in safe compartments?
is $text1, $text2;

# (16) Try the BROKEN routine in safe compartments
sub my_broken {
    my %a = @_;
    $a{error} =~ s/ at.*//s;
    "OK! text:$a{text} error:$a{error} lineno:$a{lineno} arg:$a{arg}";
}

my $templateB = Text::Template->new(TYPE => 'STRING', SOURCE => '{die}');
isa_ok $templateB, 'Text::Template';

$text1 = $templateB->fill_in(
    BROKEN     => \&my_broken,
    BROKEN_ARG => 'barg',
    SAFE       => Safe->new);

my $result1 = qq{OK! text:die error:Died lineno:1 arg:barg};
is $text1, $result1;
