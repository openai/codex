package platform::mingw;

use strict;
use warnings;
use Carp;

use vars qw(@ISA);

require platform::Unix;
@ISA = qw(platform::Unix);

# Assume someone set @INC right before loading this module
use configdata;

sub binext              { '.exe' }
sub objext              { '.obj' }
sub libext              { '.a' }
sub dsoext              { '.dll' }
sub defext              { '.def' }

# Other extra that aren't defined in platform::BASE
sub resext              { '.res.obj' }
sub shlibext            { '.dll' }
sub shlibextimport      { $target{shared_import_extension} || '.dll.a' }
sub shlibextsimple      { undef }
sub makedepcmd          { $disabled{makedepend} ? undef : $config{makedepcmd} }

(my $sover_filename = $config{shlib_version}) =~ s|\.|_|g;
sub shlib_version_as_filename {
    return $sover_filename;
}
sub sharedname {
    return platform::BASE::__concat(platform::BASE->sharedname($_[1]),
                                    "-",
                                    $_[0]->shlib_version_as_filename(),
                                    ($config{target} eq "mingw64"
                                         ? "-x64" : ""));
}

# With Mingw and other DLL producers, there isn't any "simpler" shared
# library name.  However, there is a static import library.
sub sharedlib_simple {
    return undef;
}

sub sharedlib_import {
    return platform::BASE::__concat(platform::BASE->sharedname($_[1]),
                                    $_[0]->shlibextimport());
}

1;
