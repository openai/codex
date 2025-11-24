package platform::AIX;

use strict;
use warnings;
use Carp;

use vars qw(@ISA);

require platform::Unix;
@ISA = qw(platform::Unix);

# Assume someone set @INC right before loading this module
use configdata;

sub dsoext              { '.so' }
sub shlibextsimple      { return '.so' if $target{shared_target} eq "aix-solib";
			  '.a'}

# In shared mode, the default static library names clashes with the final
# "simple" full shared library name, so we add '_a' to the basename of the
# static libraries in that case, unless in solib mode (using only .so
# files for shared libraries, and not packaging them inside archives)
sub staticname {
    return platform::Unix->staticname($_[1]) if $target{shared_target} eq "aix-solib";

    # Non-installed libraries are *always* static, and their names remain
    # the same, except for the mandatory extension
    my $in_libname = platform::BASE->staticname($_[1]);
    return $in_libname
        if $unified_info{attributes}->{libraries}->{$_[1]}->{noinst};

    return platform::BASE->staticname($_[1]) . ($disabled{shared} ? '' : '_a');
}

# In solib mode, we do not install the simple symlink (we install the import
# library).  In regular mode, we install the symlink.
sub sharedlib_simple {
    return undef if $target{shared_target} eq "aix-solib";
    return platform::Unix->sharedlib_simple($_[1], $_[0]->shlibextsimple());
}

# In solib mode, we install the import library.  In regular mode, we have
# no import library.
sub sharedlib_import {
    return platform::Unix->sharedlib_simple($_[1]) if $target{shared_target} eq "aix-solib";
    return undef;
}
