#!perl
#
# Test for comments within an inline code block

use strict;
use warnings;
use Test::More tests => 2;

use_ok 'Text::Template' or exit 1;

my $tmpl = Text::Template->new(
    TYPE => 'STRING',
    SOURCE => "Hello {\$name#comment}");

my $vars = { name => 'Bob' };

is $tmpl->fill_in(HASH => $vars), 'Hello Bob';
