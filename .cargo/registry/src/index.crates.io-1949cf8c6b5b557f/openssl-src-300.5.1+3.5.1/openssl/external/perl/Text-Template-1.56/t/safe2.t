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
    plan tests => 12;
}

use_ok 'Text::Template' or exit 1;

my $c = Safe->new or die;

# Test handling of packages and importing.
$c->reval('$P = "safe root"');
our $P = 'main';
$Q::P = $Q::P = 'Q';

# How to effectively test the gensymming?

my $t = Text::Template->new(
    TYPE   => 'STRING',
    SOURCE => 'package is {$P}') or die;

# (1) Default behavior: Inherit from calling package, `main' in this case.
my $text = $t->fill_in();
is $text, 'package is main';

# (2) When a package is specified, we should use that package instead.
$text = $t->fill_in(PACKAGE => 'Q');
is $text, 'package is Q';

# (3) When no package is specified in safe mode, we should use the
# default safe root.
$text = $t->fill_in(SAFE => $c);
is $text, 'package is safe root';

# (4) When a package is specified in safe mode, we should use the
# default safe root, after aliasing to the specified package
TODO: {
    local $TODO = "test fails when tested with TAP/Devel::Cover" if defined $Devel::Cover::VERSION;
    $text = $t->fill_in(SAFE => $c, PACKAGE => 'Q');
    is $text, 'package is Q';
}

# Now let's see if hash vars are installed properly into safe templates
$t = Text::Template->new(
    TYPE   => 'STRING',
    SOURCE => 'hash is {$H}') or die;

# (5) First in default mode
$text = $t->fill_in(HASH => { H => 'good5' });
is $text, 'hash is good5';

# suppress "once" warnings
$Q::H = $Q2::H = undef;

# (6) Now in packages
$text = $t->fill_in(HASH => { H => 'good6' }, PACKAGE => 'Q');
is $text, 'hash is good6';

# (7) Now in the default root of the safe compartment
TODO: {
    local $TODO = "test fails when tested with TAP/Devel::Cover" if defined $Devel::Cover::VERSION;
    $text = $t->fill_in(HASH => { H => 'good7' }, SAFE => $c);
    is $text, 'hash is good7';
}

# (8) Now in the default root after aliasing to a package that
# got the hash stuffed in
our $H;
TODO: {
    local $TODO = "test fails when tested with TAP/Devel::Cover" if defined $Devel::Cover::VERSION;
    $text = $t->fill_in(HASH => { H => 'good8' }, SAFE => $c, PACKAGE => 'Q2');
    is $text, 'hash is good8';
}

# Now let's make sure that none of the packages leaked on each other.
# (9) This var should NOT have been installed into the main package
ok !defined $H;
$H = $H;

# (11) this value overwrote the one from test 6.
is $Q::H, 'good7';

# (12)
is $Q2::H, 'good8';
