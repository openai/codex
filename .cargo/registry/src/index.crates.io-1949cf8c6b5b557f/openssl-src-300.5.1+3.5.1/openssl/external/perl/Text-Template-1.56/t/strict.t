#!perl
#
# Tests for STRICT features
# These tests first appeared in version 1.48.

use strict;
use warnings;
use Test::More tests => 4;

use_ok 'Text::Template' or exit 1;

@Emptyclass1::ISA = 'Text::Template';
@Emptyclass2::ISA = 'Text::Template';

my $tin = q{The value of $foo is: {$foo}};

Text::Template->always_prepend(q{$foo = "global"});

my $tmpl1 = Text::Template->new(
    TYPE   => 'STRING',
    SOURCE => $tin);

my $tmpl2 = Text::Template->new(
    TYPE    => 'STRING',
    SOURCE  => $tin,
    PREPEND => q{$foo = "template"});

$tmpl1->compile;
$tmpl2->compile;

# strict should cause t1 to contain an error message if wrong variable is used in template
my $t1 = $tmpl1->fill_in(PACKAGE => 'T1', STRICT => 1, HASH => { bar => 'baz' });

# non-strict still works
my $t2 = $tmpl2->fill_in(PACKAGE => 'T2', HASH => { bar => 'baz' });

# prepend overrides the hash values
my $t3 = $tmpl2->fill_in(
    PREPEND => q{$foo = "fillin"},
    PACKAGE => 'T3',
    STRICT  => 1,
    HASH    => { foo => 'hashval2' });

like $t1, qr/Global symbol "\$foo" requires explicit package/;
is $t2, 'The value of $foo is: template', "non-strict hash still works";
is $t3, "The value of \$foo is: fillin", "hash values with prepend, prepend wins, even under strict.";
