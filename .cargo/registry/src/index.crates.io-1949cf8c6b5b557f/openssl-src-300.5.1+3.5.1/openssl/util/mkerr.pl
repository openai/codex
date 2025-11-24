#! /usr/bin/env perl
# Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;
use warnings;

use File::Basename;
use File::Spec::Functions qw(abs2rel rel2abs);

use lib ".";
use configdata;

my $config       = "crypto/err/openssl.ec";
my $debug        = 0;
my $internal     = 0;
my $nowrite      = 0;
my $rebuild      = 0;
my $reindex      = 0;
my $static       = 0;
my $unref        = 0;
my %modules         = ();

my $errors       = 0;
my @t            = localtime();
my $YEAR         = $t[5] + 1900;

sub phase
{
    my $text = uc(shift);
    print STDERR "\n---\n$text\n" if $debug;
}

sub help
{
    print STDERR <<"EOF";
mkerr.pl [options] [files...]

Options:

    -conf FILE  Use the named config file FILE instead of the default.

    -debug      Verbose output debugging on stderr.

    -internal   Generate code that is to be built as part of OpenSSL itself.
                Also scans internal list of files.

    -module M   Only useful with -internal!
                Only write files for library module M.  Whether files are
                actually written or not depends on other options, such as
                -rebuild.
                Note: this option is cumulative.  If not given at all, all
                internal modules will be considered.

    -nowrite    Do not write the header/source files, even if changed.

    -rebuild    Rebuild all header and C source files, even if there
                were no changes.

    -reindex    Ignore previously assigned values (except for R records in
                the config file) and renumber everything starting at 100.

    -static     Make the load/unload functions static.

    -unref      List all unreferenced function and reason codes on stderr;
                implies -nowrite.

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
        $unref = 1;
    } elsif ( $arg eq "-internal" ) {
        $internal = 1;
    } elsif ( $arg eq "-nowrite" ) {
        $nowrite = 1;
    } elsif ( $arg eq "-rebuild" ) {
        $rebuild = 1;
    } elsif ( $arg eq "-reindex" ) {
        $reindex = 1;
    } elsif ( $arg eq "-static" ) {
        $static = 1;
    } elsif ( $arg eq "-unref" ) {
        $unref = 1;
        $nowrite = 1;
    } elsif ( $arg eq "-module" ) {
        shift @ARGV;
        $modules{uc $ARGV[0]} = 1;
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
    die "Cannot mix -internal and -static\n" if $static;
    die "Extra parameters given.\n" if @ARGV;
    @source = ( glob('crypto/*.c'), glob('crypto/*/*.c'),
                glob('ssl/*.c'), glob('ssl/*/*.c'), glob('ssl/*/*/*.c'),
                glob('providers/*.c'), glob('providers/*/*.c'),
                glob('providers/*/*/*.c') );
} else {
    die "-module isn't useful without -internal\n" if scalar keys %modules > 0;
    @source = @ARGV;
}

# Data parsed out of the config and state files.
my %hpubinc;    # lib -> public header
my %libpubinc;  # public header -> lib
my %hprivinc;   # lib -> private header
my %libprivinc; # private header -> lib
my %cskip;      # error_file -> lib
my %errorfile;  # lib -> error file name
my %rmax;       # lib -> max assigned reason code
my %rassigned;  # lib -> colon-separated list of assigned reason codes
my %rnew;       # lib -> count of new reason codes
my %rextra;     # "extra" reason code -> lib
my %rcodes;     # reason-name -> value
my $statefile;  # state file with assigned reason and function codes
my %strings;    # define -> text

# Read and parse the config file
open(IN, "$config") || die "Can't open config file $config, $!,";
while ( <IN> ) {
    next if /^#/ || /^$/;
    if ( /^L\s+(\S+)\s+(\S+)\s+(\S+)(?:\s+(\S+))?\s+$/ ) {
        my $lib = $1;
        my $pubhdr = $2;
        my $err = $3;
        my $privhdr = $4 // 'NONE';
        $hpubinc{$lib}   = $pubhdr;
        $libpubinc{$pubhdr} = $lib;
        $hprivinc{$lib}   = $privhdr;
        $libprivinc{$privhdr} = $lib;
        $cskip{$err}  = $lib;
        $errorfile{$lib} = $err;
        next if $err eq 'NONE';
        $rmax{$lib}      = 100;
        $rassigned{$lib} = ":";
        $rnew{$lib}      = 0;
        die "Public header file must be in include/openssl ($pubhdr is not)\n"
            if ($internal
                && $pubhdr ne 'NONE'
                && $pubhdr !~ m|^include/openssl/|);
        die "Private header file may only be specified with -internal ($privhdr given)\n"
            unless ($privhdr eq 'NONE' || $internal);
    } elsif ( /^R\s+(\S+)\s+(\S+)/ ) {
        $rextra{$1} = $2;
        $rcodes{$1} = $2;
    } elsif ( /^S\s+(\S+)/ ) {
        $statefile = $1;
    } else {
        die "Illegal config line $_\n";
    }
}
close IN;

if ( ! $statefile ) {
    $statefile = $config;
    $statefile =~ s/.ec/.txt/;
}

# The statefile has all the previous assignments.
&phase("Reading state");
my $skippedstate = 0;
if ( ! $reindex && $statefile ) {
    open(STATE, "<$statefile") || die "Can't open $statefile, $!";

    # Scan function and reason codes and store them: keep a note of the
    # maximum code used.
    while ( <STATE> ) {
        next if /^#/ || /^$/;
        my $name;
        my $code;
        if ( /^(.+):(\d+):\\$/ ) {
            $name = $1;
            $code = $2;
            my $next = <STATE>;
            $next =~ s/^\s*(.*)\s*$/$1/;
            die "Duplicate define $name" if exists $strings{$name};
            $strings{$name} = $next;
        } elsif ( /^(\S+):(\d+):(.*)$/ ) {
            $name = $1;
            $code = $2;
            die "Duplicate define $name" if exists $strings{$name};
            $strings{$name} = $3;
        } else {
            die "Bad line in $statefile:\n$_\n";
        }
        my $lib = $name;
        $lib =~ s/^((?:OSSL_|OPENSSL_)?[^_]{2,}).*$/$1/;
        $lib = "SSL" if $lib =~ /TLS/;
        if ( !defined $errorfile{$lib} ) {
            print "Skipping $_";
            $skippedstate++;
            next;
        }
        next if $errorfile{$lib} eq 'NONE';
        if ( $name =~ /^(?:OSSL_|OPENSSL_)?[A-Z0-9]{2,}_R_/ ) {
            die "$lib reason code $code collision at $name\n"
                if $rassigned{$lib} =~ /:$code:/;
            $rassigned{$lib} .= "$code:";
            if ( !exists $rextra{$name} ) {
                $rmax{$lib} = $code if $code > $rmax{$lib};
            }
            $rcodes{$name} = $code;
        } elsif ( $name =~ /^(?:OSSL_|OPENSSL_)?[A-Z0-9]{2,}_F_/ ) {
            # We do nothing with the function codes, just let them go away
        } else {
            die "Bad line in $statefile:\n$_\n";
        }
    }
    close(STATE);

    if ( $debug ) {
        foreach my $lib ( sort keys %rmax ) {
            print STDERR "Reason codes for ${lib}:\n";
            if ( $rassigned{$lib} =~ m/^:(.*):$/ ) {
                my @rassigned = sort { $a <=> $b } split( ":", $1 );
                print STDERR "  ", join(' ', @rassigned), "\n";
            } else {
                print STDERR "  --none--\n";
            }
        }
    }
}

# Scan each C source file and look for reason codes.  This is done by
# looking for strings that "look like" reason codes: basically anything
# consisting of all uppercase and numerics which _R_ in it and which has
# the name of an error library at the start.  Should there be anything else,
# such as a type name, we add exceptions here.
# If a code doesn't exist in list compiled from headers then mark it
# with the value "X" as a place holder to give it a value later.
# Store all reason codes found in and %usedreasons so all those unreferenced
# can be printed out.
&phase("Scanning source");
my %usedreasons;
foreach my $file ( @source ) {
    # Don't parse the error source file.
    next if exists $cskip{$file};
    open( IN, "<$file" ) || die "Can't open $file, $!,";
    my $func;
    my $linenr = 0;
    print STDERR "$file:\n" if $debug;
    while ( <IN> ) {

        # skip obsoleted source files entirely!
        last if /^#error\s+obsolete/;
        $linenr++;

        if ( /(((?:OSSL_|OPENSSL_)?[A-Z0-9]{2,})_R_[A-Z0-9_]+)/ ) {
            next unless exists $errorfile{$2};
            next if $errorfile{$2} eq 'NONE';
            $usedreasons{$1} = 1;
            if ( !exists $rcodes{$1} ) {
                print STDERR "  New reason $1\n" if $debug;
                $rcodes{$1} = "X";
                $rnew{$2}++;
            }
            print STDERR "  Reason $1 = $rcodes{$1}\n" if $debug;
        }
    }
    close IN;
}
print STDERR "\n" if $debug;

# Now process each library in turn.
&phase("Writing files");
my $newstate = 0;
foreach my $lib ( keys %errorfile ) {
    next if ! $rnew{$lib} && ! $rebuild;
    next if scalar keys %modules > 0 && !$modules{$lib};
    next if $nowrite;
    print STDERR "$lib: $rnew{$lib} new reasons\n" if $rnew{$lib};
    $newstate = 1;

    # If we get here then we have some new error codes so we
    # need to rebuild the header file and C file.

    # Make a sorted list of error and reason codes for later use.
    my @reasons  = sort grep( /^${lib}_/, keys %rcodes );

    # indent level for innermost preprocessor lines
    my $indent = " ";

    # Flag if the sub-library is disablable
    # There are a few exceptions, where disabling the sub-library
    # doesn't actually remove the whole sub-library, but rather implements
    # it with a NULL backend.
    my $disablable =
        ($lib ne "SSL" && $lib ne "ASYNC" && $lib ne "DSO"
         && (grep { $lib eq uc $_ } @disablables, @disablables_int));

    # Rewrite the internal header file if there is one ($internal only!)

    if ($hprivinc{$lib} ne 'NONE') {
        my $hfile = $hprivinc{$lib};
        my $guard = $hfile;

        if ($guard =~ m|^include/|) {
            $guard = $';
        } else {
            $guard = basename($guard);
        }
        $guard = "OSSL_" . join('_', split(m|[./]|, uc $guard));

        open( OUT, ">$hfile" ) || die "Can't write to $hfile, $!,";
        print OUT <<"EOF";
/*
 * Generated by util/mkerr.pl DO NOT EDIT
 * Copyright 2020-$YEAR The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the \"License\").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef $guard
# define $guard
# pragma once

# include <openssl/opensslconf.h>
# include <openssl/symhacks.h>

# ifdef  __cplusplus
extern \"C\" {
# endif

EOF
        $indent = ' ';
        if ($disablable) {
            print OUT <<"EOF";
# ifndef OPENSSL_NO_${lib}

EOF
            $indent = "  ";
        }
        print OUT <<"EOF";
int ossl_err_load_${lib}_strings(void);
EOF

        # If this library doesn't have a public header file, we write all
        # definitions that would end up there here instead
        if ($hpubinc{$lib} eq 'NONE') {
            print OUT "\n/*\n * $lib reason codes.\n */\n";
            foreach my $i ( @reasons ) {
                my $z = 48 - length($i);
                $z = 0 if $z < 0;
                if ( $rcodes{$i} eq "X" ) {
                    $rassigned{$lib} =~ m/^:([^:]*):/;
                    my $findcode = $1;
                    $findcode = $rmax{$lib} if !defined $findcode;
                    while ( $rassigned{$lib} =~ m/:$findcode:/ ) {
                        $findcode++;
                    }
                    $rcodes{$i} = $findcode;
                    $rassigned{$lib} .= "$findcode:";
                    print STDERR "New Reason code $i\n" if $debug;
                }
                printf OUT "#${indent}define $i%s $rcodes{$i}\n", " " x $z;
            }
            print OUT "\n";
        }

        # This doesn't go all the way down to zero, to allow for the ending
        # brace for 'extern "C" {'.
        while (length($indent) > 1) {
            $indent = substr $indent, 0, -1;
            print OUT "#${indent}endif\n";
        }

        print OUT <<"EOF";

# ifdef  __cplusplus
}
# endif
#endif
EOF
        close OUT;
    }

    # Rewrite the public header file

    if ($hpubinc{$lib} ne 'NONE') {
        my $extra_include =
            $internal
            ? ($lib ne 'SSL'
               ? "# include <openssl/cryptoerr_legacy.h>\n"
               : "# include <openssl/sslerr_legacy.h>\n")
            : '';
        my $hfile = $hpubinc{$lib};
        my $guard = $hfile;
        $guard =~ s|^include/||;
        $guard = join('_', split(m|[./]|, uc $guard));
        $guard = "OSSL_" . $guard unless $internal;

        open( OUT, ">$hfile" ) || die "Can't write to $hfile, $!,";
        print OUT <<"EOF";
/*
 * Generated by util/mkerr.pl DO NOT EDIT
 * Copyright 1995-$YEAR The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the \"License\").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef $guard
# define $guard
# pragma once

# include <openssl/opensslconf.h>
# include <openssl/symhacks.h>
$extra_include

EOF
        $indent = ' ';
        if ( $internal ) {
            if ($disablable) {
                print OUT <<"EOF";
# ifndef OPENSSL_NO_${lib}

EOF
                $indent .= ' ';
            }
        } else {
            print OUT <<"EOF";
# define ${lib}err(f, r) ERR_${lib}_error(0, (r), OPENSSL_FILE, OPENSSL_LINE)
# define ERR_R_${lib}_LIB ERR_${lib}_lib()

EOF
            if ( ! $static ) {
                print OUT <<"EOF";

# ifdef  __cplusplus
extern \"C\" {
# endif
int ERR_load_${lib}_strings(void);
void ERR_unload_${lib}_strings(void);
void ERR_${lib}_error(int function, int reason, const char *file, int line);
# ifdef  __cplusplus
}
# endif
EOF
            }
        }

        print OUT "\n/*\n * $lib reason codes.\n */\n";
        foreach my $i ( @reasons ) {
            my $z = 48 - length($i);
            $z = 0 if $z < 0;
            if ( $rcodes{$i} eq "X" ) {
                $rassigned{$lib} =~ m/^:([^:]*):/;
                my $findcode = $1;
                $findcode = $rmax{$lib} if !defined $findcode;
                while ( $rassigned{$lib} =~ m/:$findcode:/ ) {
                    $findcode++;
                }
                $rcodes{$i} = $findcode;
                $rassigned{$lib} .= "$findcode:";
                print STDERR "New Reason code $i\n" if $debug;
            }
            printf OUT "#${indent}define $i%s $rcodes{$i}\n", " " x $z;
        }
        print OUT "\n";

        while (length($indent) > 0) {
            $indent = substr $indent, 0, -1;
            print OUT "#${indent}endif\n";
        }
        close OUT;
    }

    # Rewrite the C source file containing the error details.

    if ($errorfile{$lib} ne 'NONE') {
        # First, read any existing reason string definitions:
        my $cfile = $errorfile{$lib};
        my $pack_lib = $internal ? "ERR_LIB_${lib}" : "0";
        my $hpubincf = $hpubinc{$lib};
        my $hprivincf = $hprivinc{$lib};
        my $includes = '';
        if ($internal) {
            if ($hpubincf ne 'NONE') {
                $hpubincf =~ s|^include/||;
                $includes .= "#include <${hpubincf}>\n";
            }
            if ($hprivincf =~ m|^include/|) {
                $hprivincf = $';
            } else {
                $hprivincf = abs2rel(rel2abs($hprivincf),
                                     rel2abs(dirname($cfile)));
            }
            $includes .= "#include \"${hprivincf}\"\n";
        } else {
            $includes .= "#include \"${hpubincf}\"\n";
        }

        open( OUT, ">$cfile" )
            || die "Can't open $cfile for writing, $!, stopped";

        my $const = $internal ? 'const ' : '';

        print OUT <<"EOF";
/*
 * Generated by util/mkerr.pl DO NOT EDIT
 * Copyright 1995-$YEAR The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/err.h>
$includes
EOF
        $indent = '';
        if ( $internal ) {
            if ($disablable) {
                print OUT <<"EOF";
#ifndef OPENSSL_NO_${lib}

EOF
                $indent .= ' ';
            }
        }
        print OUT <<"EOF";
#${indent}ifndef OPENSSL_NO_ERR

static ${const}ERR_STRING_DATA ${lib}_str_reasons[] = {
EOF

        # Add each reason code.
        foreach my $i ( @reasons ) {
            my $rn;
            if ( exists $strings{$i} ) {
                $rn = $strings{$i};
                $rn = "" if $rn eq '*';
            } else {
                $i =~ /^${lib}_R_(\S+)$/;
                $rn = $1;
                $rn =~ tr/_[A-Z]/ [a-z]/;
                $strings{$i} = $rn;
            }
            my $lines;
            $lines = "    {ERR_PACK($pack_lib, 0, $i), \"$rn\"},";
            $lines = "    {ERR_PACK($pack_lib, 0, $i),\n     \"$rn\"},"
                if length($lines) > 80;
            print OUT "$lines\n";
        }
        print OUT <<"EOF";
    {0, NULL}
};

#${indent}endif
EOF
        if ( $internal ) {
            print OUT <<"EOF";

int ossl_err_load_${lib}_strings(void)
{
#${indent}ifndef OPENSSL_NO_ERR
    if (ERR_reason_error_string(${lib}_str_reasons[0].error) == NULL)
        ERR_load_strings_const(${lib}_str_reasons);
#${indent}endif
    return 1;
}
EOF
        } else {
            my $st = $static ? "static " : "";
            print OUT <<"EOF";

static int lib_code = 0;
static int error_loaded = 0;

${st}int ERR_load_${lib}_strings(void)
{
    if (lib_code == 0)
        lib_code = ERR_get_next_error_library();

    if (!error_loaded) {
#ifndef OPENSSL_NO_ERR
        ERR_load_strings(lib_code, ${lib}_str_reasons);
#endif
        error_loaded = 1;
    }
    return 1;
}

${st}void ERR_unload_${lib}_strings(void)
{
    if (error_loaded) {
#ifndef OPENSSL_NO_ERR
        ERR_unload_strings(lib_code, ${lib}_str_reasons);
#endif
        error_loaded = 0;
    }
}

${st}void ERR_${lib}_error(int function, int reason, const char *file, int line)
{
    if (lib_code == 0)
        lib_code = ERR_get_next_error_library();
    ERR_raise(lib_code, reason);
    ERR_set_debug(file, line, NULL);
}

${st}int ERR_${lib}_lib(void)
{
    if (lib_code == 0)
        lib_code = ERR_get_next_error_library();
    return lib_code;
}
EOF

        }

        while (length($indent) > 1) {
            $indent = substr $indent, 0, -1;
            print OUT "#${indent}endif\n";
        }
        if ($internal && $disablable) {
            print OUT <<"EOF";
#else
NON_EMPTY_TRANSLATION_UNIT
#endif
EOF
        }
        close OUT;
    }
}

&phase("Ending");
# Make a list of unreferenced reason codes
if ( $unref ) {
    my @runref;
    foreach ( keys %rcodes ) {
        push( @runref, $_ ) unless exists $usedreasons{$_};
    }
    if ( @runref ) {
        print STDERR "The following reason codes were not referenced:\n";
        foreach ( sort @runref ) {
            print STDERR "  $_\n";
        }
    }
}

die "Found $errors errors, quitting" if $errors;

# Update the state file
if ( $newstate )  {
    open(OUT, ">$statefile.new")
        || die "Can't write $statefile.new, $!";
    print OUT <<"EOF";
# Copyright 1999-$YEAR The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
EOF
    print OUT "\n#Reason codes\n";
    foreach my $i ( sort keys %rcodes ) {
        my $short = "$i:$rcodes{$i}:";
        my $t = exists $strings{$i} ? "$strings{$i}" : "";
        $t = "\\\n\t" . $t if length($short) + length($t) > 80;
        print OUT "$short$t\n";
    }
    close(OUT);
    if ( $skippedstate ) {
        print "Skipped state, leaving update in $statefile.new";
    } else {
        rename "$statefile", "$statefile.old"
            || die "Can't backup $statefile to $statefile.old, $!";
        rename "$statefile.new", "$statefile"
            || die "Can't rename $statefile to $statefile.new, $!";
    }
}

exit;
