package platform::Windows::MSVC;

use strict;
use warnings;
use Carp;

use vars qw(@ISA);

require platform::Windows;
@ISA = qw(platform::Windows);

# Assume someone set @INC right before loading this module
use configdata;

sub pdbext              { '.pdb' }

# It's possible that this variant of |sharedname| should be in Windows.pm.
# However, this variant was VC only in 1.1.1, so we maintain that here until
# further notice.
sub sharedname {
    return platform::BASE::__concat(platform::BASE->sharedname($_[1]),
                                    "-",
                                    $_[0]->shlib_version_as_filename(),
                                    ($target{multilib} // '' ),
                                    ($_[0]->shlibvariant() // ''));
}

sub staticlibpdb {
    return platform::BASE::__concat($_[0]->staticname($_[1]), $_[0]->pdbext());
}

sub sharedlibpdb {
    return platform::BASE::__concat($_[0]->sharedname($_[1]), $_[0]->pdbext());
}

sub dsopdb {
    return platform::BASE::__concat($_[0]->dsoname($_[1]), $_[0]->pdbext());
}

sub binpdb {
    return platform::BASE::__concat($_[0]->binname($_[1]), $_[0]->pdbext());
}

1;
