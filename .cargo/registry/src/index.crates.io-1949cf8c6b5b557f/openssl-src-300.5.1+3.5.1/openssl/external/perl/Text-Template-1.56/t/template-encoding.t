#!perl

use utf8;
use strict;
use warnings;
use Test::More;
use Encode;
use File::Temp;

# Non-CORE module(s)
unless (eval { require Test::More::UTF8; 1; } ) {
    plan skip_all => '[ Test::More::UTF8 ] is required for testing';
}

plan tests => 3;

use_ok 'Text::Template' or exit 1;

my $tmp_fh = File::Temp->new;

print $tmp_fh encode('UTF-8', "\x{4f60}\x{597d} {{\$name}}");

$tmp_fh->flush;

# UTF-8 encoded template file
my $str = Text::Template->new(
    TYPE     => 'FILE',
    SOURCE   => $tmp_fh->filename,
    ENCODING => 'UTF-8'
)->fill_in(HASH => { name => 'World' });

is $str, "\x{4f60}\x{597d} World";

$tmp_fh = File::Temp->new;

print $tmp_fh encode('iso-8859-1', "Ol\x{e1} {{\$name}}");

$tmp_fh->flush;

# ISO-8859-1 encoded template file
$str = Text::Template->new(
    TYPE     => 'FILE',
    SOURCE   => $tmp_fh->filename,
    ENCODING => 'iso-8859-1'
)->fill_in(HASH => { name => 'World' });

is $str, "Ol\x{e1} World";
