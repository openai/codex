#!perl
#
# Tests of basic, essential functionality
#

use strict;
use warnings;
use Test::More tests => 34;
use File::Temp;

my $tmpfile = File::Temp->new;

use_ok 'Text::Template' or exit 1;

$X::v = $Y::v = 0;    # Suppress `var used only once'

my $template_1 = <<EOM;
We will put value of \$v (which is "abc") here -> {\$v}
We will evaluate 1+1 here -> {1 + 1}
EOM

# (1) Construct temporary template file for testing
# file operations
my $TEMPFILE = $tmpfile->filename;

eval {
    open my $tmp, '>', $TEMPFILE
        or die "Couldn't write tempfile $TEMPFILE: $!";

    print $tmp $template_1;
    close $tmp;

    pass;
};
if ($@) {
    fail $@;
}

# (2) Build template from file
my $template = Text::Template->new('type' => 'FILE', 'source' => $TEMPFILE);
ok(defined $template) or diag $Text::Template::ERROR;

# (3) Fill in template from file
$X::v = "abc";
my $resultX = <<EOM;
We will put value of \$v (which is "abc") here -> abc
We will evaluate 1+1 here -> 2
EOM
$Y::v = "ABC";
my $resultY = <<EOM;
We will put value of \$v (which is "abc") here -> ABC
We will evaluate 1+1 here -> 2
EOM

my $text = $template->fill_in('package' => 'X');
is $text, $resultX;

# (4) Fill in same template again
$text = $template->fill_in('package' => 'Y');
is $text, $resultY;

# (5) Simple test of `fill_this_in'
$text = Text::Template->fill_this_in($template_1, 'package' => 'X');
is $text, $resultX;

# (6) test creation of template from filehandle
open my $tmpl, '<', $TEMPFILE or die "failed to open $TEMPFILE: $!";

$template = Text::Template->new(type => 'FILEHANDLE', source => $tmpl);
ok defined $template or diag $Text::Template::ERROR;

# (7) test filling in of template from filehandle
$text = $template->fill_in('package' => 'X');
is $text, $resultX;

# (8) test second fill_in on same template object
$text = $template->fill_in('package' => 'Y');
is $text, $resultY;

close $tmpl;

# (9) test creation of template from array
$template = Text::Template->new(
    type   => 'ARRAY',
    source => [
        'We will put value of $v (which is "abc") here -> {$v}', "\n",
        'We will evaluate 1+1 here -> {1+1}',                    "\n"
    ]
);

ok defined $template;    # or diag $Text::Template::ERROR;

# (10) test filling in of template from array
$text = $template->fill_in('package' => 'X');
is $text, $resultX;

# (11) test second fill_in on same array template object
$text = $template->fill_in('package' => 'Y');
is $text, $resultY;

# (12) Make sure \ is working properly
# Test added for version 1.11
$tmpl = Text::Template->new(TYPE => 'STRING', SOURCE => 'B{"\\}"}C{"\\{"}D');

# This should fail if the \ are not interpreted properly.
$text = $tmpl->fill_in();
is $text, 'B}C{D';

# (13) Make sure \ is working properly
# Test added for version 1.11
$tmpl = Text::Template->new(TYPE => 'STRING', SOURCE => qq{A{"\t"}B});

# Symptom of old problem:  ALL \ were special in templates, so
# The lexer would return (A, PROGTEXT("t"), B), and the
# result text would be AtB instead of A(tab)B.
$text = $tmpl->fill_in();

is $text, "A\tB";

# (14-27) Make sure \ is working properly
# Test added for version 1.11
# This is a sort of general test.
my @tests = (
    '{""}'              => '',           # (14)
    '{"}"}'             => undef,        # (15)
    '{"\\}"}'           => '}',          # One backslash
    '{"\\\\}"}'         => undef,        # Two backslashes
    '{"\\\\\\}"}'       => '}',          # Three backslashes
    '{"\\\\\\\\}"}'     => undef,        # Four backslashes
    '{"\\\\\\\\\\}"}'   => '\}',         # Five backslashes  (20)
    '{"x20"}'           => 'x20',
    '{"\\x20"}'         => ' ',          # One backslash
    '{"\\\\x20"}'       => '\\x20',      # Two backslashes
    '{"\\\\\\x20"}'     => '\\ ',        # Three backslashes
    '{"\\\\\\\\x20"}'   => '\\\\x20',    # Four backslashes  (25)
    '{"\\\\\\\\\\x20"}' => '\\\\ ',      # Five backslashes
    '{"\\x20\\}"}'      => ' }',         # (27)
);

while (my ($test, $result) = splice @tests, 0, 2) {
    my $tmpl = Text::Template->new(TYPE => 'STRING', SOURCE => $test);
    my $text = $tmpl->fill_in;

    ok(!defined $text && !defined $result || $text eq $result)
        or diag "expected .$result. got .$text.";
}

# (28-30) I discovered that you can't pass a glob ref as your filehandle.
# MJD 20010827
# (28) test creation of template from filehandle
$tmpl = undef;
ok(open $tmpl, '<', $TEMPFILE) or diag "Couldn't open $TEMPFILE: $!";
$template = Text::Template->new(type => 'FILEHANDLE', source => $tmpl);
ok(defined $template) or diag $Text::Template::ERROR;

# (29) test filling in of template from filehandle
$text = $template->fill_in('package' => 'X');
is $text, $resultX;

# (30) test second fill_in on same template object
$text = $template->fill_in('package' => 'Y');
is $text, $resultY;

close $tmpl;

# (31) Test _scrubpkg for leakiness
$Text::Template::GEN0::test = 1;
Text::Template::_scrubpkg('Text::Template::GEN0');
ok !($Text::Template::GEN0::test
    || exists $Text::Template::GEN0::{test}
    || exists $Text::Template::{'GEN0::'});

# that filename parameter works. we use BROKEN to verify this
$text = Text::Template->new(
    TYPE   => 'string',
    SOURCE => 'Hello {1/0}'
)->fill_in(FILENAME => 'foo.txt');

like $text, qr/division by zero at foo\.txt line 1/;
