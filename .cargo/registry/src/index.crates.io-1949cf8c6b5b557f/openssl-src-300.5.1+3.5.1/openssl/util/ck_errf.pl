#! /usr/bin/env perl
# Copyright 1995-2018 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# This is just a quick script to scan for cases where the 'error'
# function name in a XXXerr() macro is wrong.
#
# Run in the top level by going
# perl util/ck_errf.pl */*.c */*/*.c
#

use strict;
use warnings;

my $config;
my $err_strict = 0;
my $debug      = 0;
my $internal   = 0;

sub help
{
    print STDERR <<"EOF";
mkerr.pl [options] [files...]

Options:

    -conf FILE  Use the named config file FILE instead of the default.

    -debug      Verbose output debugging on stderr.

    -internal   Generate code that is to be built as part of OpenSSL itself.
                Also scans internal list of files.

    -strict     If any error was found, fail with exit code 1, otherwise 0.

    -help       Show this help text.

    ...         Additional arguments are added to the file list to scan,
                if '-internal' was NOT specified on the command line.

EOF
}

while ( @ARGV ) {
    my $arg = $ARGV[0];
    last unless $arg =~ /-.*/;
    $arg = $1 if $arg =~ /-(-.*)/;
    if ( $arg eq "-conf" ) {
        $config = $ARGV[1];
        shift @ARGV;
    } elsif ( $arg eq "-debug" ) {
        $debug = 1;
    } elsif ( $arg eq "-internal" ) {
        $internal = 1;
    } elsif ( $arg eq "-strict" ) {
        $err_strict = 1;
    } elsif ( $arg =~ /-*h(elp)?/ ) {
        &help();
        exit;
    } elsif ( $arg =~ /-.*/ ) {
        die "Unknown option $arg; use -h for help.\n";
    }
    shift @ARGV;
}

my @source;
if ( $internal ) {
    die "Extra parameters given.\n" if @ARGV;
    $config = "crypto/err/openssl.ec" unless defined $config;
    @source = ( glob('crypto/*.c'), glob('crypto/*/*.c'),
                glob('ssl/*.c'), glob('ssl/*/*.c'), glob('providers/*.c'),
                glob('providers/*/*.c'), glob('providers/*/*/*.c') );
} else {
    die "Configuration file not given.\nSee '$0 -help' for information\n"
        unless defined $config;
    @source = @ARGV;
}

# To detect if there is any error generation for a libcrypto/libssl libs
# we don't know, we need to find out what libs we do know.  That list is
# readily available in crypto/err/openssl.ec, in form of lines starting
# with "L ".  Note that we always rely on the modules SYS and ERR to be
# generally available.
my %libs       = ( SYS => 1, ERR => 1 );
open my $cfh, $config or die "Trying to read $config: $!\n";
while (<$cfh>) {
    s|\R$||;                    # Better chomp
    next unless m|^L ([0-9A-Z_]+)\s|;
    next if $1 eq "NONE";
    $libs{$1} = 1;
}

my $bad = 0;
foreach my $file (@source) {
    open( IN, "<$file" ) || die "Can't open $file, $!";
    my $func = "";
    while (<IN>) {
        if ( !/;$/ && /^\**([a-zA-Z_].*[\s*])?([A-Za-z_0-9]+)\(.*([),]|$)/ ) {
            /^([^()]*(\([^()]*\)[^()]*)*)\(/;
            $1 =~ /([A-Za-z_0-9]*)$/;
            $func = $1;
            $func =~ tr/A-Z/a-z/;
        }
        if ( /([A-Z0-9_]+[A-Z0-9])err\(([^,]+)/ && !/ckerr_ignore/ ) {
            my $errlib = $1;
            my $n      = $2;

            unless ( $libs{$errlib} ) {
                print "$file:$.:$errlib not listed in $config\n";
                $libs{$errlib} = 1; # To not display it again
                $bad = 1;
            }

            if ( $func eq "" ) {
                print "$file:$.:???:$n\n";
                $bad = 1;
                next;
            }

            if ( $n !~ /^(.+)_F_(.+)$/ ) {
                #print "check -$file:$.:$func:$n\n";
                next;
            }
            my $lib = $1;
            $n   = $2;

            if ( $lib ne $errlib ) {
                print "$file:$.:$func:$n [${errlib}err]\n";
                $bad = 1;
                next;
            }

            $n =~ tr/A-Z/a-z/;
            if ( $n ne $func && $errlib ne "SYS" ) {
                print "$file:$.:$func:$n\n";
                $bad = 1;
                next;
            }

            #		print "$func:$1\n";
        }
    }
    close(IN);
}

if ( $bad && $err_strict ) {
    print STDERR "FATAL: error discrepancy\n";
    exit 1;
}
