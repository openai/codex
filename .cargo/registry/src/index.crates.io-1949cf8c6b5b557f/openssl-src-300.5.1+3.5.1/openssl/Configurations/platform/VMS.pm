package platform::VMS;

use strict;
use warnings;
use Carp;

use vars qw(@ISA);

require platform::BASE;
@ISA = qw(platform::BASE);

# Assume someone set @INC right before loading this module
use configdata;

# VMS has a cultural standard where all installed libraries are prefixed.
# For OpenSSL, the choice is 'ossl$' (this prefix was claimed in a
# conversation with VSI, Tuesday January 26 2016)
sub osslprefix          { 'OSSL$' }

sub binext              { '.EXE' }
sub dsoext              { '.EXE' }
sub shlibext            { '.EXE' }
sub libext              { '.OLB' }
sub defext              { '.OPT' }
sub objext              { '.OBJ' }
sub depext              { '.D' }
sub asmext              { '.ASM' }

# Other extra that aren't defined in platform::BASE
sub shlibvariant        { $target{shlib_variant} || '' }

sub optext              { '.OPT' }
sub optname             { return $_[1] }
sub opt                 { return $_[0]->optname($_[1]) . $_[0]->optext() }

# Other projects include the pointer size in the name of installed libraries,
# so we do too.
sub staticname {
    # Non-installed libraries are *always* static, and their names remain
    # the same, except for the mandatory extension
    my $in_libname = platform::BASE->staticname($_[1]);
    return $in_libname
        if $unified_info{attributes}->{libraries}->{$_[1]}->{noinst};

    return platform::BASE::__concat($_[0]->osslprefix(),
                                    platform::BASE->staticname($_[1]),
                                    $target{pointer_size});
}

# To enable installation of multiple major OpenSSL releases, we include the
# version number in installed shared library names.
my $sover_filename =
    join('', map { sprintf "%02d", $_ } split(m|\.|, $config{shlib_version}));
sub shlib_version_as_filename {
    return $sover_filename;
}
sub sharedname {
    return platform::BASE::__concat($_[0]->osslprefix(),
                                    platform::BASE->sharedname($_[1]),
                                    $_[0]->shlib_version_as_filename(),
                                    ($_[0]->shlibvariant() // ''),
                                    "_shr$target{pointer_size}");
}

1;
