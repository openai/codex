#! /usr/bin/env perl
# Copyright 2015-2022 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;
use warnings;

# Recognise VERBOSE aka V which is common on other projects.
# Additionally, recognise VERBOSE_FAILURE aka VF aka REPORT_FAILURES
# and recognise VERBOSE_FAILURE_PROGRESS aka VFP aka REPORT_FAILURES_PROGRESS.
BEGIN {
    $ENV{HARNESS_VERBOSE} = "yes" if $ENV{VERBOSE} || $ENV{V};
    $ENV{HARNESS_VERBOSE_FAILURE} = "yes"
        if $ENV{VERBOSE_FAILURE} || $ENV{VF} || $ENV{REPORT_FAILURES};
    $ENV{HARNESS_VERBOSE_FAILURE_PROGRESS} = "yes"
        if ($ENV{VERBOSE_FAILURE_PROGRESS} || $ENV{VFP}
            || $ENV{REPORT_FAILURES_PROGRESS});
}

use File::Spec::Functions qw/catdir catfile curdir abs2rel rel2abs/;
use File::Basename;
use FindBin;
use lib "$FindBin::Bin/../util/perl";
use OpenSSL::Glob;

my $srctop = $ENV{SRCTOP} || $ENV{TOP};
my $bldtop = $ENV{BLDTOP} || $ENV{TOP};
my $recipesdir = catdir($srctop, "test", "recipes");
my $libdir = rel2abs(catdir($srctop, "util", "perl"));
my $jobs = $ENV{HARNESS_JOBS} // 1;

$ENV{OPENSSL_CONF} = rel2abs(catfile($srctop, "apps", "openssl.cnf"));
$ENV{OPENSSL_CONF_INCLUDE} = rel2abs(catdir($bldtop, "test"));
$ENV{OPENSSL_MODULES} = rel2abs(catdir($bldtop, "providers"));
$ENV{OPENSSL_ENGINES} = rel2abs(catdir($bldtop, "engines"));
$ENV{CTLOG_FILE} = rel2abs(catfile($srctop, "test", "ct", "log_list.cnf"));

# On platforms that support this, this will ensure malloc returns data that is
# set to a non-zero value. Can be helpful for detecting uninitialized reads in
# some situations.
$ENV{'MALLOC_PERTURB_'} = '128' if !defined $ENV{'MALLOC_PERTURB_'};

my %tapargs =
    ( verbosity         => $ENV{HARNESS_VERBOSE} ? 1 : 0,
      lib               => [ $libdir ],
      switches          => '-w',
      merge             => 1,
      timer             => $ENV{HARNESS_TIMER} ? 1 : 0,
    );

if ($jobs > 1) {
    if ($ENV{HARNESS_VERBOSE}) {
        print "Warning: HARNESS_JOBS > 1 ignored with HARNESS_VERBOSE\n";
    } else {
        $tapargs{jobs} = $jobs;
        print "Using HARNESS_JOBS=$jobs\n";
    }
}

# Additional OpenSSL special TAP arguments.  Because we can't pass them via
# TAP::Harness->new(), they will be accessed directly, see the
# TAP::Parser::OpenSSL implementation further down
my %openssl_args = ();

$openssl_args{'failure_verbosity'} = $ENV{HARNESS_VERBOSE} ? 0 :
    $ENV{HARNESS_VERBOSE_FAILURE_PROGRESS} ? 2 :
    1; # $ENV{HARNESS_VERBOSE_FAILURE}
print "Warning: HARNESS_VERBOSE overrides HARNESS_VERBOSE_FAILURE*\n"
    if ($ENV{HARNESS_VERBOSE} && ($ENV{HARNESS_VERBOSE_FAILURE}
                                  || $ENV{HARNESS_VERBOSE_FAILURE_PROGRESS}));
print "Warning: HARNESS_VERBOSE_FAILURE_PROGRESS overrides HARNESS_VERBOSE_FAILURE\n"
    if ($ENV{HARNESS_VERBOSE_FAILURE_PROGRESS} && $ENV{HARNESS_VERBOSE_FAILURE});

my $outfilename = $ENV{HARNESS_TAP_COPY};
open $openssl_args{'tap_copy'}, ">$outfilename"
    or die "Trying to create $outfilename: $!\n"
    if defined $outfilename;

my @alltests = find_matching_tests("*");
my %tests = ();

sub reorder {
    my $key = pop;

    # for parallel test runs, do slow tests first
    if ($jobs > 1 && $key =~ m/test_ssl_new|test_fuzz/) {
        $key =~ s/(\d+)-/01-/;
    }
    return $key;
}

my $initial_arg = 1;
foreach my $arg (@ARGV ? @ARGV : ('alltests')) {
    if ($arg eq 'list') {
        foreach (@alltests) {
            (my $x = basename($_)) =~ s|^[0-9][0-9]-(.*)\.t$|$1|;
            print $x,"\n";
        }
        exit 0;
    }
    if ($arg eq 'alltests') {
        warn "'alltests' encountered, ignoring everything before that...\n"
            unless $initial_arg;
        %tests = map { $_ => 1 } @alltests;
    } elsif ($arg =~ m/^(-?)(.*)/) {
        my $sign = $1;
        my $test = $2;
        my @matches = find_matching_tests($test);

        # If '-foo' is the first arg, it's short for 'alltests -foo'
        if ($sign eq '-' && $initial_arg) {
            %tests = map { $_ => 1 } @alltests;
        }

        if (scalar @matches == 0) {
            warn "Test $test found no match, skipping ",
                ($sign eq '-' ? "removal" : "addition"),
                "...\n";
        } else {
            foreach $test (@matches) {
                if ($sign eq '-') {
                    delete $tests{$test};
                } else {
                    $tests{$test} = 1;
                }
            }
        }
    } else {
        warn "I don't know what '$arg' is about, ignoring...\n";
    }

    $initial_arg = 0;
}

# prep recipes are mandatory and need to be always run first
my @preps = glob(catfile($recipesdir,"00-prep_*.t"));
foreach my $test (@preps) {
    delete $tests{$test};
}

sub find_matching_tests {
    my ($glob) = @_;

    if ($glob =~ m|^[\d\[\]\?\-]+$|) {
        return glob(catfile($recipesdir,"$glob-*.t"));
    }

    return glob(catfile($recipesdir,"*-$glob.t"));
}

# The following is quite a bit of hackery to adapt to both TAP::Harness
# and Test::Harness, depending on what's available.
# The TAP::Harness hack allows support for HARNESS_VERBOSE_FAILURE* and
# HARNESS_TAP_COPY, while the Test::Harness hack can't, because the pre
# TAP::Harness Test::Harness simply doesn't have support for this sort of
# thing.
#
# We use eval to avoid undue interruption if TAP::Harness isn't present.

my $package;
my $eres;

$eres = eval {
    package TAP::Parser::OpenSSL;
    use parent -norequire, 'TAP::Parser';
    require TAP::Parser;

    sub new {
        my $class = shift;
        my %opts = %{ shift() };
        my $failure_verbosity = $openssl_args{failure_verbosity};
        my @plans = (); # initial level, no plan yet
        my $output_buffer = "";
        my $in_indirect = 0;

        # We rely heavily on perl closures to make failure verbosity work
        # We need to do so, because there's no way to safely pass extra
        # objects down all the way to the TAP::Parser::Result object
        my @failure_output = ();
        my %callbacks = ();
        if ($failure_verbosity > 0 || defined $openssl_args{tap_copy}) {
            $callbacks{ALL} = sub { # on each line of test output
                my $self = shift;
                my $fh = $openssl_args{tap_copy};
                print $fh $self->as_string, "\n"
                    if defined $fh;

                my $failure_verbosity = $openssl_args{failure_verbosity};
                if ($failure_verbosity > 0) {
                    my $is_plan = $self->is_plan;
                    my $tests_planned = $is_plan && $self->tests_planned;
                    my $is_test = $self->is_test;
                    my $is_ok = $is_test && $self->is_ok;

                    # workaround for parser not coping with sub-test indentation
                    if ($self->is_unknown) {
                        my $level = $#plans;
                        my $indent = $level < 0 ? "" : " " x ($level * 4);

                        ($is_plan, $tests_planned) = (1, $1)
                            if ($self->as_string =~ m/^$indent    1\.\.(\d+)/);
                        ($is_test, $is_ok) = (1, !$1)
                            if ($self->as_string =~ m/^$indent(not )?ok /);
                    }

                    if ($is_plan) {
                        push @plans, $tests_planned;
                        $output_buffer = ""; # ignore comments etc. until plan
                    } elsif ($is_test) { # result of a test
                        pop @plans if @plans && --($plans[-1]) <= 0;
                        if ($output_buffer =~ /.*Indirect leak of.*/ == 1) {
                            my @asan_array = split("\n", $output_buffer);
                            foreach (@asan_array) {
                                if ($_ =~ /.*Indirect leak of.*/ == 1) {
                                    if ($in_indirect != 1) {
                                        print "::group::Indirect Leaks\n";
                                    }
                                    $in_indirect = 1;
                                }
                                print "$_\n";
                                if ($_ =~ /.*Indirect leak of.*/ != 1) {
                                    if ($_ =~ /^    #.*/ == 0) {
                                        if ($in_indirect != 0) {
                                            print "\n::endgroup::\n";
                                        }
                                        $in_indirect = 0;
                                    }
                                }
                            }
                        } else {
                            print $output_buffer if !$is_ok;
                        }
                        print "\n".$self->as_string
                            if !$is_ok || $failure_verbosity == 2;
                        print "\n# ------------------------------------------------------------------------------" if !$is_ok;
                        $output_buffer = "";
                    } elsif ($self->as_string ne "") {
                        # typically is_comment or is_unknown
                        $output_buffer .= "\n".$self->as_string;
                    }
                }
            }
        }

        if ($failure_verbosity > 0) {
            $callbacks{EOF} = sub {
                my $self = shift;

                # We know we are a TAP::Parser::Aggregator object
                if (scalar $self->failed > 0 && @failure_output) {
                    # We add an extra empty line, because in the case of a
                    # progress counter, we're still at the end of that progress
                    # line.
                    print $_, "\n" foreach (("", @failure_output));
                }
                # Echo any trailing comments etc.
                print "$output_buffer";
            };
        }

        if (keys %callbacks) {
            # If %opts already has a callbacks element, the order here
            # ensures we do not override it
            %opts = ( callbacks => { %callbacks }, %opts );
        }

        return $class->SUPER::new({ %opts });
    }

    package TAP::Harness::OpenSSL;
    use parent -norequire, 'TAP::Harness';
    require TAP::Harness;

    package main;

    $tapargs{parser_class} = "TAP::Parser::OpenSSL";
    $package = 'TAP::Harness::OpenSSL';
};

unless (defined $eres) {
    $eres = eval {
        # Fake TAP::Harness in case it's not loaded
        package TAP::Harness::fake;
        use parent 'Test::Harness';

        sub new {
            my $class = shift;
            my %args = %{ shift() };

            return bless { %args }, $class;
        }

        sub runtests {
            my $self = shift;

            # Pre TAP::Harness Test::Harness doesn't support [ filename, name ]
            # elements, so convert such elements to just be the filename
            my @args = map { ref($_) eq 'ARRAY' ? $_->[0] : $_ } @_;

            my @switches = ();
            if ($self->{switches}) {
                push @switches, $self->{switches};
            }
            if ($self->{lib}) {
                foreach (@{$self->{lib}}) {
                    my $l = $_;

                    # It seems that $switches is getting interpreted with 'eval'
                    # or something like that, and that we need to take care of
                    # backslashes or they will disappear along the way.
                    $l =~ s|\\|\\\\|g if $^O eq "MSWin32";
                    push @switches, "-I$l";
                }
            }

            $Test::Harness::switches = join(' ', @switches);
            Test::Harness::runtests(@args);
        }

        package main;
        $package = 'TAP::Harness::fake';
    };
}

unless (defined $eres) {
    print $@,"\n" if $@;
    print $!,"\n" if $!;
    exit 127;
}

my $harness = $package->new(\%tapargs);
my $ret =
    $harness->runtests(map { [ abs2rel($_, rel2abs(curdir())), basename($_) ] }
                       @preps);

if (ref($ret) ne "TAP::Parser::Aggregator" || !$ret->has_errors) {
    $ret =
        $harness->runtests(map { [ abs2rel($_, rel2abs(curdir())), basename($_) ] }
                           sort { reorder($a) cmp reorder($b) } keys %tests);
}

# If this is a TAP::Parser::Aggregator, $ret->has_errors is the count of
# tests that failed.  We don't bother with that exact number, just exit
# with an appropriate exit code when it isn't zero.
if (ref($ret) eq "TAP::Parser::Aggregator") {
    exit 0 unless $ret->has_errors;
    exit 1 unless $^O eq 'VMS';
    # On VMS, perl converts an exit 1 to SS$_ABORT (%SYSTEM-F-ABORT), which
    # is a bit harsh.  As per perl recommendations, we explicitly use the
    # same VMS status code as typical C programs would for exit(1), except
    # we set the error severity rather than success.
    # Ref: https://perldoc.perl.org/perlport#exit
    #      https://perldoc.perl.org/perlvms#$?
    exit  0x35a000              # C facility code
        + 8                     # 1 << 3 (to make space for the 3 severity bits)
        + 2                     # severity: E(rror)
        + 0x10000000;           # bit 28 set => the shell stays silent
}

# If this isn't a TAP::Parser::Aggregator, it's the pre-TAP test harness,
# which simply dies at the end if any test failed, so we don't need to bother
# with any exit code in that case.
