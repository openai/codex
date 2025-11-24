package platform::Unix;

use strict;
use warnings;
use Carp;

use vars qw(@ISA);

require platform::BASE;
@ISA = qw(platform::BASE);

# Assume someone set @INC right before loading this module
use configdata;

sub binext              { $target{exe_extension} || '' }
sub dsoext              { $target{dso_extension} || platform->shlibextsimple()
                              || '.so' }
# Because these are also used in scripts and not just Makefile, we must
# convert $(SHLIB_VERSION_NUMBER) to the actual number.
sub shlibext            { (my $x = $target{shared_extension}
                               || '.so.$(SHLIB_VERSION_NUMBER)')
                              =~ s|\.\$\(SHLIB_VERSION_NUMBER\)
                                  |.$config{shlib_version}|x;
                          $x; }
sub libext              { $target{lib_extension} || '.a' }
sub defext              { $target{def_extension} || '.ld' }
sub objext              { $target{obj_extension} || '.o' }
sub depext              { $target{obj_extension} || '.d' }

# Other extra that aren't defined in platform::BASE
sub shlibextsimple      { (my $x = $target{shared_extension} || '.so')
                              =~ s|\.\$\(SHLIB_VERSION_NUMBER\)||;
                          $x; }
sub shlibvariant        { $target{shlib_variant} || "" }
sub makedepcmd          { $disabled{makedepend} ? undef : $config{makedepcmd} }

# No conversion of assembler extension on Unix
sub asm {
    return $_[1];
}

# At some point, we might decide that static libraries are called something
# other than the default...
sub staticname {
    # Non-installed libraries are *always* static, and their names remain
    # the same, except for the mandatory extension
    my $in_libname = platform::BASE->staticname($_[1]);
    return $in_libname
        if $unified_info{attributes}->{libraries}->{$_[1]}->{noinst};

    # We currently return the same name anyway...  but we might choose to
    # append '_static' or '_a' some time in the future.
    return platform::BASE->staticname($_[1]);
}

sub sharedname {
    return platform::BASE::__concat(platform::BASE->sharedname($_[1]),
                                    ($_[0]->shlibvariant() // ''));
}

sub sharedname_simple {
    return platform::BASE::__isshared($_[1]) ? $_[1] : undef;
}

sub sharedlib_simple {
    # This function returns the simplified shared library name (no version
    # or variant in the shared library file name) if the simple variants of
    # the base name or the suffix differ from the full variants of the same.

    # Note: if $_[1] isn't a shared library name, then $_[0]->sharedname()
    # and $_[0]->sharedname_simple() will return undef.  This needs being
    # accounted for.
    my $name = $_[0]->sharedname($_[1]);
    my $simplename = $_[0]->sharedname_simple($_[1]);
    my $ext = $_[0]->shlibext();
    # Allow override of the extension passed in as parameter
    my $simpleext = $_[2];
    $simpleext = $_[0]->shlibextsimple() unless defined $simpleext;

    return undef unless defined $simplename && defined $name;
    return undef if ($name eq $simplename && $ext eq $simpleext);
    return platform::BASE::__concat($simplename, $simpleext);
}

sub sharedlib_import {
    return undef;
}

1;
