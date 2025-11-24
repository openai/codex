#!perl
#
# Tests for user-specified delimiter functions
# These tests first appeared in version 1.20.

use strict;
use warnings;
use Test::More tests => 19;

use_ok 'Text::Template' or exit 1;

# (1) Try a simple delimiter: <<..>>
# First with the delimiters specified at object creation time
our $V = $V = 119;
my $template  = q{The value of $V is <<$V>>.};
my $result    = q{The value of $V is 119.};
my $template1 = Text::Template->new(
    TYPE       => 'STRING',
    SOURCE     => $template,
    DELIMITERS => [ '<<', '>>' ])
        or die "Couldn't construct template object: $Text::Template::ERROR; aborting";

my $text = $template1->fill_in();
is $text, $result;

# (2) Now with delimiter choice deferred until fill-in time.
$template1 = Text::Template->new(TYPE => 'STRING', SOURCE => $template);
$text = $template1->fill_in(DELIMITERS => [ '<<', '>>' ]);
is $text, $result;

# (3) Now we'll try using regex metacharacters
# First with the delimiters specified at object creation time
$template  = q{The value of $V is [$V].};
$template1 = Text::Template->new(
    TYPE       => 'STRING',
    SOURCE     => $template,
    DELIMITERS => [ '[', ']' ])
        or die "Couldn't construct template object: $Text::Template::ERROR; aborting";

$text = $template1->fill_in();
is $text, $result;

# (4) Now with delimiter choice deferred until fill-in time.
$template1 = Text::Template->new(TYPE => 'STRING', SOURCE => $template);
$text = $template1->fill_in(DELIMITERS => [ '[', ']' ]);
is $text, $result;

# (5-18) Make sure \ is working properly
# (That is to say, it is ignored.)
# These tests are similar to those in 01-basic.t.
my @tests = (
    '{""}' => '',    # (5)

    # Backslashes don't matter
    '{"}"}'           => undef,
    '{"\\}"}'         => undef,    # One backslash
    '{"\\\\}"}'       => undef,    # Two backslashes
    '{"\\\\\\}"}'     => undef,    # Three backslashes
    '{"\\\\\\\\}"}'   => undef,    # Four backslashes (10)
    '{"\\\\\\\\\\}"}' => undef,    # Five backslashes

    # Backslashes are always passed directly to Perl
    '{"x20"}'           => 'x20',
    '{"\\x20"}'         => ' ',          # One backslash
    '{"\\\\x20"}'       => '\\x20',      # Two backslashes
    '{"\\\\\\x20"}'     => '\\ ',        # Three backslashes (15)
    '{"\\\\\\\\x20"}'   => '\\\\x20',    # Four backslashes
    '{"\\\\\\\\\\x20"}' => '\\\\ ',      # Five backslashes
    '{"\\x20\\}"}'      => undef,        # (18)
);

while (my ($test, $result) = splice @tests, 0, 2) {
    my $tmpl = Text::Template->new(
        TYPE       => 'STRING',
        SOURCE     => $test,
        DELIMITERS => [ '{', '}' ]);

    my $text = $tmpl->fill_in;

    my $ok = (!defined $text && !defined $result || $text eq $result);

    ok($ok) or diag "expected .$result., got .$text.";
}
