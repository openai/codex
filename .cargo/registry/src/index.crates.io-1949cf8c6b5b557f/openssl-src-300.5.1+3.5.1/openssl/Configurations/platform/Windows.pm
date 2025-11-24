package platform::Windows;

use strict;
use warnings;
use Carp;

use vars qw(@ISA);

require platform::BASE;
@ISA = qw(platform::BASE);

# Assume someone set @INC right before loading this module
use configdata;

sub binext              { '.exe' }
sub dsoext              { '.dll' }
sub shlibext            { '.dll' }
sub libext              { '.lib' }
sub defext              { '.def' }
sub objext              { '.obj' }
sub depext              { '.d' }
sub asmext              { '.asm' }

# Other extra that aren't defined in platform::BASE
sub resext              { '.res' }
sub shlibextimport      { '.lib' }
sub shlibvariant        { $target{shlib_variant} || '' }

sub staticname {
    # Non-installed libraries are *always* static, and their names remain
    # the same, except for the mandatory extension
    my $in_libname = platform::BASE->staticname($_[1]);
    return $in_libname
        if $unified_info{attributes}->{libraries}->{$_[1]}->{noinst};

    # To make sure not to clash with an import library, we make the static
    # variant of our installed libraries get '_static' added to their names.
    return platform::BASE->staticname($_[1])
        . ($disabled{shared} ? '' : '_static');
}

# To mark forward compatibility, we include the OpenSSL major release version
# number in the installed shared library names.
(my $sover_filename = $config{shlib_version}) =~ s|\.|_|g;
sub shlib_version_as_filename {
    return $sover_filename
}
sub sharedname {
    return platform::BASE::__concat(platform::BASE->sharedname($_[1]),
                                    "-",
                                    $_[0]->shlib_version_as_filename(),
                                    ($_[0]->shlibvariant() // ''));
}

sub sharedname_import {
    return platform::BASE::__isshared($_[1]) ? $_[1] : undef;
}

sub sharedlib_import {
    return platform::BASE::__concat($_[0]->sharedname_import($_[1]),
                                    $_[0]->shlibextimport());
}

1;
