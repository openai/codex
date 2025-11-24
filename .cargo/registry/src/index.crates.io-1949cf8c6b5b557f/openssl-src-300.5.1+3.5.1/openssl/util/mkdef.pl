#! /usr/bin/env perl
# Copyright 2018-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# Generate a linker version script suitable for the given platform
# from a given ordinals file.

use strict;
use warnings;

use Getopt::Long;
use FindBin;
use lib "$FindBin::Bin/perl";

use OpenSSL::Ordinals;

use lib '.';
use configdata;

use File::Spec::Functions;
use lib catdir($config{sourcedir}, 'Configurations');
use platform;

my $name = undef;               # internal library/module name
my $ordinals_file = undef;      # the ordinals file to use
my $version = undef;            # the version to use for the library
my $OS = undef;                 # the operating system family
my $type = 'lib';               # either lib or dso
my $verbose = 0;
my $ctest = 0;
my $debug = 0;

# For VMS, some modules may have case insensitive names
my $case_insensitive = 0;

GetOptions('name=s'     => \$name,
           'ordinals=s' => \$ordinals_file,
           'version=s'  => \$version,
           'OS=s'       => \$OS,
           'type=s'     => \$type,
           'ctest'      => \$ctest,
           'verbose'    => \$verbose,
           # For VMS
           'case-insensitive' => \$case_insensitive)
    or die "Error in command line arguments\n";

die "Please supply arguments\n"
    unless $name && $ordinals_file && $OS;
die "--type argument must be equal to 'lib' or 'dso'"
    if $type ne 'lib' && $type ne 'dso';

# When building a "variant" shared library, with a custom SONAME, also customize
# all the symbol versions.  This produces a shared object that can coexist
# without conflict in the same address space as a default build, or an object
# with a different variant tag.
#
# For example, with a target definition that includes:
#
#         shlib_variant => "-opt",
#
# we build the following objects:
#
# $ perl -le '
#     for (@ARGV) {
#         if ($l = readlink) {
#             printf "%s -> %s\n", $_, $l
#         } else {
#             print
#         }
#     }' *.so*
# libcrypto-opt.so.1.1
# libcrypto.so -> libcrypto-opt.so.1.1
# libssl-opt.so.1.1
# libssl.so -> libssl-opt.so.1.1
#
# whose SONAMEs and dependencies are:
#
# $ for l in *.so; do
#     echo $l
#     readelf -d $l | egrep 'SONAME|NEEDED.*(ssl|crypto)'
#   done
# libcrypto.so
#  0x000000000000000e (SONAME)             Library soname: [libcrypto-opt.so.1.1]
# libssl.so
#  0x0000000000000001 (NEEDED)             Shared library: [libcrypto-opt.so.1.1]
#  0x000000000000000e (SONAME)             Library soname: [libssl-opt.so.1.1]
#
# We case-fold the variant tag to uppercase and replace all non-alnum
# characters with "_".  This yields the following symbol versions:
#
# $ nm libcrypto.so | grep -w A
# 0000000000000000 A OPENSSL_OPT_1_1_0
# 0000000000000000 A OPENSSL_OPT_1_1_0a
# 0000000000000000 A OPENSSL_OPT_1_1_0c
# 0000000000000000 A OPENSSL_OPT_1_1_0d
# 0000000000000000 A OPENSSL_OPT_1_1_0f
# 0000000000000000 A OPENSSL_OPT_1_1_0g
# $ nm libssl.so | grep -w A
# 0000000000000000 A OPENSSL_OPT_1_1_0
# 0000000000000000 A OPENSSL_OPT_1_1_0d
#
(my $SO_VARIANT = uc($target{"shlib_variant"} // '')) =~ s/\W/_/g;

my $libname = $type eq 'lib' ? platform->sharedname($name) : platform->dsoname($name);

my %OS_data = (
    solaris     => { writer     => \&writer_linux,
                     sort       => sorter_linux(),
                     platforms  => { UNIX                       => 1 } },
    "solaris-gcc" => 'solaris', # alias
    linux       => 'solaris',   # alias
    "bsd-gcc"   => 'solaris',   # alias
    aix         => { writer     => \&writer_aix,
                     sort       => sorter_unix(),
                     platforms  => { UNIX                       => 1 } },
    "aix-solib" => 'aix',       # alias
    VMS         => { writer     => \&writer_VMS,
                     sort       => OpenSSL::Ordinals::by_number(),
                     platforms  => { VMS                        => 1 } },
    vms         => 'VMS',       # alias
    WINDOWS     => { writer     => \&writer_windows,
                     sort       => OpenSSL::Ordinals::by_name(),
                     platforms  => { WIN32                      => 1,
                                     _WIN32                     => 1 } },
    windows     => 'WINDOWS',   # alias
    WIN32       => 'WINDOWS',   # alias
    win32       => 'WIN32',     # alias
    32          => 'WIN32',     # alias
    NT          => 'WIN32',     # alias
    nt          => 'WIN32',     # alias
    mingw       => 'WINDOWS',   # alias
    nonstop     => { writer     => \&writer_nonstop,
                     sort       => OpenSSL::Ordinals::by_name(),
                     platforms  => { TANDEM                     => 1 } },
   );

do {
    die "Unknown operating system family $OS\n"
        unless exists $OS_data{$OS};
    $OS = $OS_data{$OS};
} while(ref($OS) eq '');

my %disabled_uc = map { my $x = uc $_; $x =~ s|-|_|g; $x => 1 } keys %disabled;

my %ordinal_opts = ();
$ordinal_opts{sort} = $OS->{sort} if $OS->{sort};
$ordinal_opts{filter} =
    sub {
        my $item = shift;
        return
            $item->exists()
            && platform_filter($item)
            && feature_filter($item);
    };
my $ordinals = OpenSSL::Ordinals->new(from => $ordinals_file);

my $writer = $OS->{writer};
$writer = \&writer_ctest if $ctest;

$writer->($ordinals->items(%ordinal_opts));

exit 0;

sub platform_filter {
    my $item = shift;
    my %platforms = ( $item->platforms() );

    # True if no platforms are defined
    return 1 if scalar keys %platforms == 0;

    # For any item platform tag, return the equivalence with the
    # current platform settings if it exists there, return 0 otherwise
    # if the item platform tag is true
    for (keys %platforms) {
        if (exists $OS->{platforms}->{$_}) {
            return $platforms{$_} == $OS->{platforms}->{$_};
        }
        if ($platforms{$_}) {
            return 0;
        }
    }

    # Found no match?  Then it's a go
    return 1;
}

sub feature_filter {
    my $item = shift;
    my @features = ( $item->features() );

    # True if no features are defined
    return 1 if scalar @features == 0;

    my $verdict = ! grep { $disabled_uc{$_} } @features;

    if ($disabled{deprecated}) {
        foreach (@features) {
            next unless /^DEPRECATEDIN_(\d+)_(\d+)(?:_(\d+))?$/;
            my $symdep = $1 * 10000 + $2 * 100 + ($3 // 0);
            $verdict = 0 if $config{api} >= $symdep;
            print STDERR "DEBUG: \$symdep = $symdep, \$verdict = $verdict\n"
                if $debug && $1 == 0;
        }
    }

    return $verdict;
}

sub sorter_unix {
    my $by_name = OpenSSL::Ordinals::by_name();
    my %weight = (
        'FUNCTION'      => 1,
        'VARIABLE'      => 2
       );

    return sub {
        my $item1 = shift;
        my $item2 = shift;

        my $verdict = $weight{$item1->type()} <=> $weight{$item2->type()};
        if ($verdict == 0) {
            $verdict = $by_name->($item1, $item2);
        }
        return $verdict;
    };
}

sub sorter_linux {
    my $by_version = OpenSSL::Ordinals::by_version();
    my $by_unix = sorter_unix();

    return sub {
        my $item1 = shift;
        my $item2 = shift;

        my $verdict = $by_version->($item1, $item2);
        if ($verdict == 0) {
            $verdict = $by_unix->($item1, $item2);
        }
        return $verdict;
    };
}

sub writer_linux {
    my $thisversion = '';
    my $currversion_s = '';
    my $prevversion_s = '';
    my $indent = 0;

    for (@_) {
        if ($thisversion && $_->version() ne $thisversion) {
            die "$ordinals_file: It doesn't make sense to have both versioned ",
                "and unversioned symbols"
                if $thisversion eq '*';
            print <<"_____";
}${prevversion_s};
_____
            $prevversion_s = " OPENSSL${SO_VARIANT}_$thisversion";
            $thisversion = '';  # Trigger start of next section
        }
        unless ($thisversion) {
            $indent = 0;
            $thisversion = $_->version();
            $currversion_s = '';
            $currversion_s = "OPENSSL${SO_VARIANT}_$thisversion "
                if $thisversion ne '*';
            print <<"_____";
${currversion_s}{
    global:
_____
        }
        print '        ', $_->name(), ";\n";
    }

    print <<"_____";
    local: *;
}${prevversion_s};
_____
}

sub writer_aix {
    for (@_) {
        print $_->name(),"\n";
    }
}

sub writer_nonstop {
    for (@_) {
        print "-export ",$_->name(),"\n";
    }
}

sub writer_windows {
    print <<"_____";
;
; Definition file for the DLL version of the $libname library from OpenSSL
;

LIBRARY         "$libname"

EXPORTS
_____
    for (@_) {
        print "    ",$_->name();
        if (platform->can('export2internal')) {
            print "=". platform->export2internal($_->name());
        }
        print "\n";
    }
}

sub collect_VMS_mixedcase {
    return [ 'SPARE', 'SPARE' ] unless @_;

    my $s = shift;
    my $s_uc = uc($s);
    my $type = shift;

    return [ "$s=$type", 'SPARE' ] if $s_uc eq $s;
    return [ "$s_uc/$s=$type", "$s=$type" ];
}

sub collect_VMS_uppercase {
    return [ 'SPARE' ] unless @_;

    my $s = shift;
    my $s_uc = uc($s);
    my $type = shift;

    return [ "$s_uc=$type" ];
}

sub writer_VMS {
    my @slot_collection = ();
    my $collector =
        $case_insensitive ? \&collect_VMS_uppercase : \&collect_VMS_mixedcase;

    my $last_num = 0;
    foreach (@_) {
        my $this_num = $_->number();
        $this_num = $last_num + 1 if $this_num =~ m|^\?|;

        while (++$last_num < $this_num) {
            push @slot_collection, $collector->(); # Just occupy a slot
        }
        my $type = {
            FUNCTION    => 'PROCEDURE',
            VARIABLE    => 'DATA'
           } -> {$_->type()};
        push @slot_collection, $collector->($_->name(), $type);
    }

    print <<"_____" if defined $version;
IDENTIFICATION=$version
_____
    print <<"_____" unless $case_insensitive;
CASE_SENSITIVE=YES
_____
    print <<"_____";
SYMBOL_VECTOR=(-
_____
    # It's uncertain how long aggregated lines the linker can handle,
    # but it has been observed that at least 1024 characters is ok.
    # Either way, this means that we need to keep track of the total
    # line length of each "SYMBOL_VECTOR" statement.  Fortunately, we
    # can have more than one of those...
    my $symvtextcount = 16;     # The length of "SYMBOL_VECTOR=("
    while (@slot_collection) {
        my $set = shift @slot_collection;
        my $settextlength = 0;
        foreach (@$set) {
            $settextlength +=
                + 3             # two space indentation and comma
                + length($_)
                + 1             # postdent
                ;
        }
        $settextlength--;       # only one space indentation on the first one
        my $firstcomma = ',';

        if ($symvtextcount + $settextlength > 1024) {
            print <<"_____";
)
SYMBOL_VECTOR=(-
_____
            $symvtextcount = 16; # The length of "SYMBOL_VECTOR=("
        }
        if ($symvtextcount == 16) {
            $firstcomma = '';
        }

        my $indent = ' '.$firstcomma;
        foreach (@$set) {
            print <<"_____";
$indent$_ -
_____
            $symvtextcount += length($indent) + length($_) + 1;
            $indent = '  ,';
        }
    }
    print <<"_____";
)
_____

    if (defined $version) {
        $version =~ /^(\d+)\.(\d+)\.(\d+)/;
        my $libvmajor = $1;
        my $libvminor = $2 * 100 + $3;
        print <<"_____";
GSMATCH=LEQUAL,$libvmajor,$libvminor
_____
    }
}

sub writer_ctest {
    print <<'_____';
/*
 * Test file to check all DEF file symbols are present by trying
 * to link to all of them. This is *not* intended to be run!
 */

int main()
{
_____

    my $last_num = 0;
    for (@_) {
        my $this_num = $_->number();
        $this_num = $last_num + 1 if $this_num =~ m|^\?|;

        if ($_->type() eq 'VARIABLE') {
            print "\textern int ", $_->name(), '; /* type unknown */ /* ',
                  $this_num, ' ', $_->version(), " */\n";
        } else {
            print "\textern int ", $_->name(), '(); /* type unknown */ /* ',
                  $this_num, ' ', $_->version(), " */\n";
        }

        $last_num = $this_num;
    }
    print <<'_____';
}
_____
}
