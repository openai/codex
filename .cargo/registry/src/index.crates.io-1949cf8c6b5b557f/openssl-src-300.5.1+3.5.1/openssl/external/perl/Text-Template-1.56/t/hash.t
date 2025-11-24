#!perl
#
# test apparatus for Text::Template module
# still incomplete.

use strict;
use warnings;
use Test::More tests => 13;

use_ok 'Text::Template' or exit 1;

my $template = 'We will put value of $v (which is "good") here -> {$v}';

my $v = 'oops (main)';
$Q::v = 'oops (Q)';

my $vars = { 'v' => \'good' };

# (1) Build template from string
$template = Text::Template->new('type' => 'STRING', 'source' => $template);
isa_ok $template, 'Text::Template';

# (2) Fill in template in anonymous package
my $result2 = 'We will put value of $v (which is "good") here -> good';
my $text = $template->fill_in(HASH => $vars);
is $text, $result2;

# (3) Did we clobber the main variable?
is $v, 'oops (main)';

# (4) Fill in same template again
my $result4 = 'We will put value of $v (which is "good") here -> good';
$text = $template->fill_in(HASH => $vars);
is $text, $result4;

# (5) Now with a package
my $result5 = 'We will put value of $v (which is "good") here -> good';
$text = $template->fill_in(HASH => $vars, PACKAGE => 'Q');
is $text, $result5;

# (6) We expect to have clobbered the Q variable.
is $Q::v, 'good';

# (7) Now let's try it without a package
my $result7 = 'We will put value of $v (which is "good") here -> good';
$text = $template->fill_in(HASH => $vars);
is $text, $result7;

# (8-11) Now what does it do when we pass a hash with undefined values?
# Roy says it does something bad. (Added for 1.20.)
my $WARNINGS = 0;
{
    local $SIG{__WARN__} = sub { $WARNINGS++ };
    local $^W = 1;    # Make sure this is on for this test
    my $template8 = 'We will put value of $v (which is "good") here -> {defined $v ? "bad" : "good"}';
    my $result8   = 'We will put value of $v (which is "good") here -> good';
    my $template  = Text::Template->new('type' => 'STRING', 'source' => $template8);
    my $text = $template->fill_in(HASH => { 'v' => undef });

    # (8) Did we generate a warning?
    cmp_ok $WARNINGS, '==', 0;

    # (9) Was the output correct?
    is $text, $result8;

    # (10-11) Let's try that again, with a twist this time
    $WARNINGS = 0;
    $text = $template->fill_in(HASH => [ { 'v' => 17 }, { 'v' => undef } ]);

    # (10) Did we generate a warning?
    cmp_ok $WARNINGS, '==', 0;

    # (11) Was the output correct?
    SKIP: {
        skip 'not supported before 5.005', 1 unless $] >= 5.005;

        is $text, $result8;
    }
}

# (12) Now we'll test the multiple-hash option  (Added for 1.20.)
$text = Text::Template::fill_in_string(q{$v: {$v}.  @v: [{"@v"}].},
    HASH => [
        { 'v' => 17 },
        { 'v' => [ 'a', 'b', 'c' ] },
        { 'v' => \23 }
    ]
);

my $result = q{$v: 23.  @v: [a b c].};
is $text, $result;
