#! /usr/bin/env perl
# Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# Reads one or more template files and runs it through Text::Template
#
# It is assumed that this scripts is called with -Mconfigdata, a module
# that holds configuration data in %config

use strict;
use warnings;

use FindBin;
use lib "$FindBin::Bin/perl";
use OpenSSL::fallback "$FindBin::Bin/../external/perl/MODULES.txt";
use Getopt::Std;
use OpenSSL::Template;

# We expect to get a lot of information from configdata, so check that
# it was part of our commandline.
die "You must run this script with -Mconfigdata\n"
    if !exists($config{target});

# Check options ######################################################

# -o ORIGINATOR
#		declares ORIGINATOR as the originating script.
# -i .ext       Like Perl's edit-in-place -i flag
my %opts = ();
getopt('oi', \%opts);

my @autowarntext = (
    "WARNING: do not edit!",
    "Generated"
        . (defined($opts{o}) ? " by $opts{o}" : "")
        . (scalar(@ARGV) > 0 ? " from " .join(", ", @ARGV) : "")
);

if (defined($opts{s})) {
    local $/ = undef;
    open VARS, $opts{s} or die "Couldn't open $opts{s}, $!";
    my $contents = <VARS>;
    close VARS;
    eval $contents;
    die $@ if $@;
}
die "Must have input files"
   if defined($opts{i}) and scalar(@ARGV) == 0;

# Template setup #####################################################

my @template_settings =
    @ARGV
    ? map { { TYPE => 'FILE', SOURCE => $_, FILENAME => $_ } } @ARGV
    : ( { TYPE => 'FILEHANDLE', SOURCE => \*STDIN, FILENAME => '<stdin>' } );

# Error callback; print message, set status, return "stop processing"
my $failed = 0;
sub errorcallback {
    my %args = @_;
    print STDERR $args{error};
    $failed++;
    return undef;
}

# Engage! ############################################################

my $prepend = <<"_____";
use File::Spec::Functions;
use lib '$FindBin::Bin/../Configurations';
use lib '$config{builddir}';
use platform;
_____

foreach (@template_settings) {
    my $template = OpenSSL::Template->new(%$_);
    die "Couldn't create template: $Text::Template::ERROR"
        if !defined($template);

    my $result = $template->fill_in(%$_,
                       HASH => { config => \%config,
                                 target => \%target,
                                 disabled => \%disabled,
                                 withargs => \%withargs,
                                 unified_info => \%unified_info,
                                 autowarntext => \@autowarntext },
                       BROKEN => \&errorcallback,
                       PREPEND => $prepend,
                       # To ensure that global variables and functions
                       # defined in one template stick around for the
                       # next, making them combinable
                       PACKAGE => 'OpenSSL::safe');
    exit 1 if $failed;

    if (defined($opts{i})) {
        my $in = $_->{FILENAME};
        my $out = $in;
        $out =~ s/$opts{i}$//;
        die "Cannot replace file in-place $in"
            if $in eq $out;
        open OFH, ">$out"
            or die "Can't open $out, $!";
        print OFH $result;
        close OFH;
    } else {
        print $result;
    }
}
