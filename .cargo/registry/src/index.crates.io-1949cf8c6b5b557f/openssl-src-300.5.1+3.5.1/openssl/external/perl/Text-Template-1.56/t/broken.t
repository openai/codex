#!perl
# test apparatus for Text::Template module

use strict;
use warnings;
use Test::More tests => 7;

use_ok 'Text::Template' or exit 1;

# (1) basic error delivery
{
    my $r = Text::Template->new(
        TYPE   => 'string',
        SOURCE => '{1/0}',)->fill_in();
    is $r, q{Program fragment delivered error ``Illegal division by zero at template line 1.''};
}

# (2) BROKEN sub called in ->new?
{
    my $r = Text::Template->new(
        TYPE   => 'string',
        SOURCE => '{1/0}',
        BROKEN => sub { '---' },)->fill_in();
    is $r, q{---};
}

# (3) BROKEN sub called in ->fill_in?
{
    my $r = Text::Template->new(
        TYPE   => 'string',
        SOURCE => '{1/0}',)->fill_in(BROKEN => sub { '---' });
    is $r, q{---};
}

# (4) BROKEN sub passed correct args when called in ->new?
{
    my $r = Text::Template->new(
        TYPE   => 'string',
        SOURCE => '{1/0}',
        BROKEN => sub {
            my %a = @_;
            qq{$a{lineno},$a{error},$a{text}};
        },)->fill_in();
    is $r, qq{1,Illegal division by zero at template line 1.\n,1/0};
}

# (5) BROKEN sub passed correct args when called in ->fill_in?
{
    my $r = Text::Template->new(
        TYPE   => 'string',
        SOURCE => '{1/0}',
        )->fill_in(
        BROKEN => sub {
            my %a = @_;
            qq{$a{lineno},$a{error},$a{text}};
        });
    is $r, qq{1,Illegal division by zero at template line 1.\n,1/0};
}

# BROKEN sub handles undef
{
    my $r = Text::Template->new(TYPE => 'string', SOURCE => 'abc{1/0}defg')
        ->fill_in(BROKEN => sub { undef });

    is $r, 'abc';
}
