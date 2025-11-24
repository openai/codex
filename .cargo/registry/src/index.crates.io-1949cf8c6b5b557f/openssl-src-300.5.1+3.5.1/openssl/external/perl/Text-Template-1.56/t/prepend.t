#!perl
#
# Tests for PREPEND features
# These tests first appeared in version 1.22.

use strict;
use warnings;
use Test::More tests => 10;

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

my $t1 = $tmpl1->fill_in(PACKAGE => 'T1');
my $t2 = $tmpl2->fill_in(PACKAGE => 'T2');
my $t3 = $tmpl2->fill_in(PREPEND => q{$foo = "fillin"}, PACKAGE => 'T3');

is $t1, 'The value of $foo is: global';
is $t2, 'The value of $foo is: template';
is $t3, 'The value of $foo is: fillin';

Emptyclass1->always_prepend(q{$foo = 'Emptyclass global';});
$tmpl1 = Emptyclass1->new(
    TYPE   => 'STRING',
    SOURCE => $tin);

$tmpl2 = Emptyclass1->new(
    TYPE    => 'STRING',
    SOURCE  => $tin,
    PREPEND => q{$foo = "template"});

$tmpl1->compile;
$tmpl2->compile;

$t1 = $tmpl1->fill_in(PACKAGE => 'T4');
$t2 = $tmpl2->fill_in(PACKAGE => 'T5');
$t3 = $tmpl2->fill_in(PREPEND => q{$foo = "fillin"}, PACKAGE => 'T6');

is $t1, 'The value of $foo is: Emptyclass global';
is $t2, 'The value of $foo is: template';
is $t3, 'The value of $foo is: fillin';

$tmpl1 = Emptyclass2->new(
    TYPE   => 'STRING',
    SOURCE => $tin);

$tmpl2 = Emptyclass2->new(
    TYPE    => 'STRING',
    SOURCE  => $tin,
    PREPEND => q{$foo = "template"});

$tmpl1->compile;
$tmpl2->compile;

$t1 = $tmpl1->fill_in(PACKAGE => 'T4');
$t2 = $tmpl2->fill_in(PACKAGE => 'T5');
$t3 = $tmpl2->fill_in(PREPEND => q{$foo = "fillin"}, PACKAGE => 'T6');

is $t1, 'The value of $foo is: global';
is $t2, 'The value of $foo is: template';
is $t3, 'The value of $foo is: fillin';
