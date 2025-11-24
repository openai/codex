#! /usr/bin/env perl
# Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# All variables are supposed to come from Makefile, in environment variable
# form, or passed as variable assignments on the command line.
# The result is a Perl module creating the package OpenSSL::safe::installdata.

use 5.10.0;
use strict;
use warnings;
use Carp;

use File::Spec;
#use List::Util qw(pairs);
sub _pairs (@);

# These are expected to be set up as absolute directories
my @absolutes = qw(PREFIX libdir);
# These may be absolute directories, and if not, they are expected to be set up
# as subdirectories to PREFIX or LIBDIR.  The order of the pairs is important,
# since the LIBDIR subdirectories depend on the calculation of LIBDIR from
# PREFIX.
my @subdirs = _pairs (PREFIX => [ qw(BINDIR LIBDIR INCLUDEDIR APPLINKDIR) ],
                      LIBDIR => [ qw(ENGINESDIR MODULESDIR PKGCONFIGDIR
                                     CMAKECONFIGDIR) ]);
# For completeness, other expected variables
my @others = qw(VERSION LDLIBS);

my %all = ( );
foreach (@absolutes) { $all{$_} = 1 }
foreach (@subdirs) { foreach (@{$_->[1]}) { $all{$_} = 1 } }
foreach (@others) { $all{$_} = 1 }
print STDERR "DEBUG: all keys: ", join(", ", sort keys %all), "\n";

my %keys = ();
my %values = ();
foreach (@ARGV) {
    (my $k, my $v) = m|^([^=]*)=(.*)$|;
    $keys{$k} = 1;
    push @{$values{$k}}, $v;
}

# warn if there are missing values, and also if there are unexpected values
foreach my $k (sort keys %all) {
    warn "No value given for $k\n" unless $keys{$k};
}
foreach my $k (sort keys %keys) {
    warn "Unknown variable $k\n" unless $all{$k};
}

# This shouldn't be needed, but just in case we get relative paths that
# should be absolute, make sure they actually are.
foreach my $k (@absolutes) {
    my $v = $values{$k} || [ '.' ];
    die "Can't have more than one $k\n" if scalar @$v > 1;
    print STDERR "DEBUG: $k = $v->[0] => ";
    $v = [ map { File::Spec->rel2abs($_) } @$v ];
    $values{$k} = $v;
    print STDERR "$k = $v->[0]\n";
}

# Absolute paths for the subdir variables are computed.  This provides
# the usual form of values for names that have become norm, known as GNU
# installation paths.
# For the benefit of those that need it, the subdirectories are preserved
# as they are, using the same variable names, suffixed with '_REL_{var}',
# if they are indeed subdirectories.  The '{var}' part of the name tells
# which other variable value they are relative to.
foreach my $pair (@subdirs) {
    my ($var, $subdir_vars) = @$pair;
    foreach my $k (@$subdir_vars) {
        my $kr = "${k}_REL_${var}";
        my $v2 = $values{$k} || [ '.' ];
        $values{$k} = [];       # We're rebuilding it
        print STDERR "DEBUG: $k = ",
            (scalar @$v2 > 1 ? "[ " . join(", ", @$v2) . " ]" : $v2->[0]),
            " => ";
        foreach my $v (@$v2) {
            if (File::Spec->file_name_is_absolute($v)) {
                push @{$values{$k}}, $v;
                push @{$values{$kr}},
                    File::Spec->abs2rel($v, $values{$var}->[0]);
            } else {
                push @{$values{$kr}}, $v;
                push @{$values{$k}},
                    File::Spec->rel2abs($v, $values{$var}->[0]);
            }
        }
        print STDERR join(", ",
                          map {
                              my $v = $values{$_};
                              "$_ = " . (scalar @$v > 1
                                         ? "[ " . join(", ", @$v) . " ]"
                                         : $v->[0]);
                          } ($k, $kr)),
            "\n";
    }
}

print <<_____;
package OpenSSL::safe::installdata;

use strict;
use warnings;
use Exporter;
our \@ISA = qw(Exporter);
our \@EXPORT = qw(
_____

foreach my $k (@absolutes) {
    print "    \@$k\n";
}
foreach my $pair (@subdirs) {
    my ($var, $subdir_vars) = @$pair;
    foreach my $k (@$subdir_vars) {
        my $k2 = "${k}_REL_${var}";
        print "    \@$k \@$k2\n";
    }
}

print <<_____;
    \$VERSION \@LDLIBS
);

_____

foreach my $k (@absolutes) {
    print "our \@$k" . ' ' x (27 - length($k)) . "= ( '",
        join("', '", @{$values{$k}}),
        "' );\n";
}
foreach my $pair (@subdirs) {
    my ($var, $subdir_vars) = @$pair;
    foreach my $k (@$subdir_vars) {
        my $k2 = "${k}_REL_${var}";
        print "our \@$k" . ' ' x (27 - length($k)) . "= ( '",
            join("', '", @{$values{$k}}),
            "' );\n";
        print "our \@$k2" . ' ' x (27 - length($k2)) . "= ( '",
            join("', '", @{$values{$k2}}),
            "' );\n";
    }
}

print <<_____;
our \$VERSION                    = '$values{VERSION}->[0]';
our \@LDLIBS                     =
    # Unix and Windows use space separation, VMS uses comma separation
    \$^O eq 'VMS'
    ? split(/ *, */, '$values{LDLIBS}->[0]')
    : split(/ +/, '$values{LDLIBS}->[0]');

1;
_____

######## Helpers

# _pairs LIST
#
# This operates on an even-sized list, and returns a list of "ARRAY"
# references, each containing two items from the given LIST.
#
# It is a quick cheap reimplementation of List::Util::pairs(), a function
# we cannot use, because it only appeared in perl v5.19.3, and we claim to
# support perl versions all the way back to v5.10.

sub _pairs (@) {
    croak "Odd number of arguments" if @_ & 1;

    my @pairlist = ();

    while (@_) {
        my $x = [ shift, shift ];
        push @pairlist, $x;
    }
    return @pairlist;
}
