
#! /usr/bin/env perl
# Copyright 2018-2021 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;
use warnings;

use Getopt::Long;
use FindBin;
use lib "$FindBin::Bin/perl";

use OpenSSL::Ordinals;
use OpenSSL::ParseC;

my $ordinals_file = undef;      # the ordinals file to use
my $symhacks_file = undef;      # a symbol hacking file (optional)
my $version = undef;            # the version to use for added symbols
my $checkexist = 0;             # (unsure yet)
my $warnings = 1;
my $renumber = 0;
my $verbose = 0;
my $debug = 0;

GetOptions('ordinals=s' => \$ordinals_file,
           'symhacks=s' => \$symhacks_file,
           'version=s'  => \$version,
           'exist'      => \$checkexist,
           'renumber'   => \$renumber,
           'warnings!'  => \$warnings,
           'verbose'    => \$verbose,
           'debug'      => \$debug)
    or die "Error in command line arguments\n";

die "Please supply ordinals file\n"
    unless $ordinals_file;

my $ordinals = OpenSSL::Ordinals->new(from => $ordinals_file,
                                      warnings => $warnings,
                                      verbose => $verbose,
                                      debug => $debug);
$ordinals->set_version($version);

my %orig_names = ();
%orig_names = map { $_->name() => 1 }
    $ordinals->items(comparator => sub { $_[0] cmp $_[1] },
                     filter => sub { $_->exists() })
    if $checkexist;

# Invalidate all entries, they get revalidated when we re-check below
$ordinals->invalidate();

foreach my $f (($symhacks_file // (), @ARGV)) {
    print STDERR $f," ","-" x (69 - length($f)),"\n" if $verbose;
    open IN, $f or die "Couldn't open $f: $!\n";
    foreach (parse(<IN>, { filename => $f,
                           warnings => $warnings,
                           verbose => $verbose,
                           debug => $debug })) {
        $_->{value} = $_->{value}||"";
        next if grep { $_ eq 'CONST_STRICT' } @{$_->{conds}};
        printf STDERR "%s> %s%s : %s\n",
            $_->{type},
            $_->{name},
            ($_->{type} eq 'M' && defined $symhacks_file && $f eq $symhacks_file
             ? ' = ' . $_->{value}
             : ''),
            join(', ', @{$_->{conds}})
            if $verbose;
        if ($_->{type} eq 'M'
                && defined $symhacks_file
                && $f eq $symhacks_file
                && $_->{value} =~ /^\w(?:\w|\d)*/) {
            $ordinals->add_alias($f, $_->{value}, $_->{name}, @{$_->{conds}});
        } else {
            next if $_->{returntype} =~ /\b(?:ossl_)inline/;
            my $type = {
                F => 'FUNCTION',
                V => 'VARIABLE',
            } -> {$_->{type}};
            if ($type) {
                $ordinals->add($f, $_->{name}, $type, @{$_->{conds}});
            }
        }
    }
    close IN;
}

$ordinals->renumber() if $renumber;

if ($checkexist) {
    my %new_names = map { $_->name() => 1 }
        $ordinals->items(comparator => sub { $_[0] cmp $_[1] },
                         filter => sub { $_->exists() });
    # Eliminate common names
    foreach (keys %orig_names) {
        next unless exists $new_names{$_};
        delete $orig_names{$_};
        delete $new_names{$_};
    }
    if (%orig_names) {
        print "The following symbols do not seem to exist in code:\n";
        foreach (sort keys %orig_names) {
            print "\t$_\n";
        }
    }
    if (%new_names) {
        print "The following existing symbols are not in ordinals file:\n";
        foreach (sort keys %new_names) {
            print "\t$_\n";
        }
    }
} else {
    my $dropped = 0;
    my $unassigned;
    my $filter = sub {
        my $item = shift;
        my $result = $item->number() ne '?' || $item->exists();
        $dropped++ unless $result;
        return $result;
    };
    $ordinals->rewrite(filter => $filter);
    my %stats = $ordinals->stats();
    print STDERR
        "${ordinals_file}: $stats{modified} old symbols have updated info\n"
        if $stats{modified};
    if ($stats{new}) {
        print STDERR "${ordinals_file}: Added $stats{new} new symbols\n";
    } else {
        print STDERR "${ordinals_file}: No new symbols added\n";
    }
    if ($dropped) {
        print STDERR "${ordinals_file}: Dropped $dropped new symbols\n";
    }
    $stats{unassigned} = 0 unless defined $stats{unassigned};
    $unassigned = $stats{unassigned} - $dropped;
    if ($unassigned) {
        my $symbol = $unassigned == 1 ? "symbol" : "symbols";
        my $is = $unassigned == 1 ? "is" : "are";
        print STDERR "${ordinals_file}: $unassigned $symbol $is without ordinal number\n";
    }
}
