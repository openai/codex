#!perl

use strict;
use warnings;
use Text::Template;

# Minimum Test::More version; 0.94+ is required for `done_testing`
BEGIN {
    unless (eval { require Test::More; "$Test::More::VERSION" >= 0.94; }) {
        Test::More::plan(skip_all => '[ Test::More v0.94+ ] is required for testing');
    }

    Test::More->import;

    # Non-CORE module(s)
    unless (eval { require Test::Warnings; 1; }) {
        plan(skip_all => '[ Test::Warnings ] is required for testing');
    }

    Test::Warnings->import;
}

my $template = <<'EOT';
{{
if ($good =~ /good/) {
    'This template should not produce warnings.'.$bad;
}
}}
EOT

$template = Text::Template->new(type => 'STRING', source => $template);
isa_ok $template, 'Text::Template';

my $result = $template->fill_in(HASH => { good => 'good' });

$result =~ s/(?:^\s+)|(?:\s+$)//gs;
is $result, 'This template should not produce warnings.';

# see https://github.com/mschout/perl-text-template/issues/10
$template = Text::Template->new(type => 'STRING', package => 'MY', source => '');
$template->fill_in(package => 'MY', hash => { include => sub { 'XX' } });

$template = Text::Template->new(type => 'STRING', package => 'MY', source => '');
$template->fill_in(package => 'MY', hash => { include => sub { 'XX' } });

done_testing;
