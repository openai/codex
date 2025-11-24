#!perl
#
# Test for breakage of Dist::Milla in v1.46
#

use strict;
use warnings;
use Text::Template;

BEGIN {
    # Minimum Test::More version; 0.94+ is required for `done_testing`
    unless (eval { require Test::More; "$Test::More::VERSION" >= 0.94; }) {
        Test::More::plan(skip_all => '[ Test::More v0.94+ ] is required for testing');
    }

    Test::More->import;
}

my $tmpl = Text::Template->new(
    TYPE       => 'STRING',
    SOURCE     => q| {{ '{{$NEXT}}' }} |,
    DELIMITERS => [ '{{', '}}' ]);

is $tmpl->fill_in, ' {{$NEXT}} ';

done_testing;
