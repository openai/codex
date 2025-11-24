package platform::Cygwin;

use strict;
use warnings;
use Carp;

use vars qw(@ISA);

require platform::mingw;
@ISA = qw(platform::mingw);

# Assume someone set @INC right before loading this module
use configdata;

sub sharedname {
    my $class = shift;
    my $lib = platform::mingw->sharedname(@_);
    $lib =~ s|^lib|cyg| if defined $lib;
    return $lib;
}

1;
