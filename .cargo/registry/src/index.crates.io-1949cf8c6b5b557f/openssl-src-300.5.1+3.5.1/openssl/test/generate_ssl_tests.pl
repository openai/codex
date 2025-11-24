#! /usr/bin/env perl
# Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

## SSL testcase generator

use strict;
use warnings;

use Cwd qw/abs_path/;
use File::Basename;
use File::Spec::Functions;

use OpenSSL::Test qw/srctop_dir srctop_file run test/;
use OpenSSL::Test::Utils;

use FindBin;
use lib "$FindBin::Bin/../util/perl";
use OpenSSL::fallback "$FindBin::Bin/../external/perl/MODULES.txt";
use Text::Template 1.46;

my $input_file;
my $provider;

BEGIN {
    #Input file may be relative to cwd, but setup below changes the cwd, so
    #figure out the absolute path first
    $input_file = abs_path(shift);
    $provider = shift // '';

    OpenSSL::Test::setup("no_test_here", quiet => 1);
}

use lib "$FindBin::Bin/ssl-tests";

use vars qw/@ISA/;
push (@ISA, qw/Text::Template/);

use ssltests_base;

sub print_templates {
    my $source = srctop_file("test", "ssl_test.tmpl");
    my $template = Text::Template->new(TYPE => 'FILE', SOURCE => $source);

    print "# Generated with generate_ssl_tests.pl\n\n";

    my $num = scalar @ssltests::tests;

    # Add the implicit base configuration.
    foreach my $test (@ssltests::tests) {
        $test->{"server"} = { (%ssltests::base_server, %{$test->{"server"}}) };
        if (defined $test->{"server2"}) {
            $test->{"server2"} = { (%ssltests::base_server, %{$test->{"server2"}}) };
        } else {
            if ($test->{"server"}->{"extra"} &&
                defined $test->{"server"}->{"extra"}->{"ServerNameCallback"}) {
                # Default is the same as server.
                $test->{"reuse_server2"} = 1;
            }
            # Do not emit an empty/duplicate "server2" section.
            $test->{"server2"} = { };
        }
        if (defined $test->{"resume_server"}) {
            $test->{"resume_server"} = { (%ssltests::base_server, %{$test->{"resume_server"}}) };
        } else {
            if (defined $test->{"test"}->{"HandshakeMode"} &&
                 $test->{"test"}->{"HandshakeMode"} eq "Resume") {
                # Default is the same as server.
                $test->{"reuse_resume_server"} = 1;
            }
            # Do not emit an empty/duplicate "resume-server" section.
            $test->{"resume_server"} = { };
        }
        $test->{"client"} = { (%ssltests::base_client, %{$test->{"client"}}) };
        if (defined $test->{"resume_client"}) {
            $test->{"resume_client"} = { (%ssltests::base_client, %{$test->{"resume_client"}}) };
        } else {
            if (defined $test->{"test"}->{"HandshakeMode"} &&
                 $test->{"test"}->{"HandshakeMode"} eq "Resume") {
                # Default is the same as client.
                $test->{"reuse_resume_client"} = 1;
            }
            # Do not emit an empty/duplicate "resume-client" section.
            $test->{"resume_client"} = { };
        }
    }

    # ssl_test expects to find a
    #
    # num_tests = n
    #
    # directive in the file. It'll then look for configuration directives
    # for n tests, that each look like this:
    #
    # test-n = test-section
    #
    # [test-section]
    # (SSL modules for client and server configuration go here.)
    #
    # [test-n]
    # (Test configuration goes here.)
    print "num_tests = $num\n\n";

    # The conf module locations must come before everything else, because
    # they look like
    #
    # test-n = test-section
    #
    # and you can't mix and match them with sections.
    my $idx = 0;

    foreach my $test (@ssltests::tests) {
        my $testname = "${idx}-" . $test->{'name'};
        print "test-$idx = $testname\n";
        $idx++;
    }

    $idx = 0;

    foreach my $test (@ssltests::tests) {
        my $testname = "${idx}-" . $test->{'name'};
        my $text = $template->fill_in(
            HASH => [{ idx => $idx, testname => $testname } , $test],
            DELIMITERS => [ "{-", "-}" ]);
        print "# ===========================================================\n\n";
        print "$text\n";
        $idx++;
    }
}

# Shamelessly copied from Configure.
sub read_config {
    my $fname = shift;
    my $provider = shift;

    my $fips_mode = $provider eq "fips";
    local $ssltests::fips_3_4 = 0;
    local $ssltests::fips_3_5 = 0;

    if ($fips_mode) {
        my $provconf = srctop_file("test", "fips-and-base.cnf");
        my $exit;

        run(test(["fips_version_test", "-config", $provconf, ">=3.4.0"]),
                 capture => 1, statusvar => \$exit);
        $ssltests::fips_3_4 = $exit;
        run(test(["fips_version_test", "-config", $provconf, ">=3.5.0"]),
                 capture => 1, statusvar => \$exit);
        $ssltests::fips_3_5 = $exit;
    }

    local $ssltests::fips_mode = $fips_mode;
    local $ssltests::no_deflt_libctx =
        $provider eq "default" || $provider eq "fips";
    open(INPUT, "< $fname") or die "Can't open input file '$fname'!\n";
    local $/ = undef;
    my $content = <INPUT>;
    close(INPUT);
    eval $content;
    warn $@ if $@;
}

# Reads the tests into ssltests::tests.
read_config($input_file, $provider);
print_templates();

1;
