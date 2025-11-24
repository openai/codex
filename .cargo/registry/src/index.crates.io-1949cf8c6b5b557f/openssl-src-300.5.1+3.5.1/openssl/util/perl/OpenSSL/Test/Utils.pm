# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

package OpenSSL::Test::Utils;

use strict;
use warnings;

use Exporter;
use vars qw($VERSION @ISA @EXPORT @EXPORT_OK %EXPORT_TAGS);
$VERSION = "0.1";
@ISA = qw(Exporter);
@EXPORT = qw(alldisabled anydisabled disabled config available_protocols
             have_IPv4 have_IPv6);

=head1 NAME

OpenSSL::Test::Utils - test utility functions

=head1 SYNOPSIS

  use OpenSSL::Test::Utils;

  my @tls = available_protocols("tls");
  my @dtls = available_protocols("dtls");
  alldisabled("dh", "dsa");
  anydisabled("dh", "dsa");

  config("fips");

  have_IPv4();
  have_IPv6();

=head1 DESCRIPTION

This module provides utility functions for the testing framework.

=cut

use OpenSSL::Test qw/:DEFAULT bldtop_file/;

=over 4

=item B<available_protocols STRING>

Returns a list of strings for all the available SSL/TLS versions if
STRING is "tls", or for all the available DTLS versions if STRING is
"dtls".  Otherwise, it returns the empty list.  The strings in the
returned list can be used with B<alldisabled> and B<anydisabled>.

=item B<alldisabled ARRAY>

=item B<anydisabled ARRAY>

In an array context returns an array with each element set to 1 if the
corresponding feature is disabled and 0 otherwise.

In a scalar context, alldisabled returns 1 if all of the features in
ARRAY are disabled, while anydisabled returns 1 if any of them are
disabled.

=item B<config STRING>

Returns an item from the %config hash in \$TOP/configdata.pm.

=item B<have_IPv4>

=item B<have_IPv6>

Return true if IPv4 / IPv6 is possible to use on the current system.
Additionally, B<have_IPv6> also checks how OpenSSL was configured,
i.e. if IPv6 was explicitly disabled with -DOPENSSL_USE_IPv6=0.

=back

=cut

our %available_protocols;
our %disabled;
our %config;
our %target;
my $configdata_loaded = 0;

sub load_configdata {
    # We eval it so it doesn't run at compile time of this file.
    # The latter would have bldtop_file() complain that setup() hasn't
    # been run yet.
    my $configdata = bldtop_file("configdata.pm");
    eval { require $configdata;
	   %available_protocols = %configdata::available_protocols;
	   %disabled = %configdata::disabled;
	   %config = %configdata::config;
	   %target = %configdata::target;
    };
    $configdata_loaded = 1;
}

# args
#  list of 1s and 0s, coming from check_disabled()
sub anyof {
    my $x = 0;
    foreach (@_) { $x += $_ }
    return $x > 0;
}

# args
#  list of 1s and 0s, coming from check_disabled()
sub allof {
    my $x = 1;
    foreach (@_) { $x *= $_ }
    return $x > 0;
}

# args
#  list of strings, all of them should be names of features
#  that can be disabled.
# returns a list of 1s (if the corresponding feature is disabled)
#  and 0s (if it isn't)
sub check_disabled {
    return map { exists $disabled{lc $_} ? 1 : 0 } @_;
}

# Exported functions #################################################

# args:
#  list of features to check
sub anydisabled {
    load_configdata() unless $configdata_loaded;
    my @ret = check_disabled(@_);
    return @ret if wantarray;
    return anyof(@ret);
}

# args:
#  list of features to check
sub alldisabled {
    load_configdata() unless $configdata_loaded;
    my @ret = check_disabled(@_);
    return @ret if wantarray;
    return allof(@ret);
}

# !!! Kept for backward compatibility
# args:
#  single string
sub disabled {
    anydisabled(@_);
}

sub available_protocols {
    load_configdata() unless $configdata_loaded;
    my $protocol_class = shift;
    if (exists $available_protocols{lc $protocol_class}) {
	return @{$available_protocols{lc $protocol_class}}
    }
    return ();
}

sub config {
    load_configdata() unless $configdata_loaded;
    return $config{$_[0]};
}

# IPv4 / IPv6 checker
my $have_IPv4 = -1;
my $have_IPv6 = -1;
my $IP_factory;
sub check_IP {
    my $listenaddress = shift;

    eval {
        require IO::Socket::IP;
        my $s = IO::Socket::IP->new(
            LocalAddr => $listenaddress,
            LocalPort => 0,
            Listen=>1,
            );
        $s or die "\n";
        $s->close();
    };
    if ($@ eq "") {
        return 1;
    }

    eval {
        require IO::Socket::INET6;
        my $s = IO::Socket::INET6->new(
            LocalAddr => $listenaddress,
            LocalPort => 0,
            Listen=>1,
            );
        $s or die "\n";
        $s->close();
    };
    if ($@ eq "") {
        return 1;
    }

    eval {
        require IO::Socket::INET;
        my $s = IO::Socket::INET->new(
            LocalAddr => $listenaddress,
            LocalPort => 0,
            Listen=>1,
            );
        $s or die "\n";
        $s->close();
    };
    if ($@ eq "") {
        return 1;
    }

    return 0;
}

sub have_IPv4 {
    if ($have_IPv4 < 0) {
        $have_IPv4 = check_IP("127.0.0.1");
    }
    return $have_IPv4;
}

sub have_IPv6 {
    if ($have_IPv6 < 0) {
        load_configdata() unless $configdata_loaded;
        # If OpenSSL is configured with IPv6 explicitly disabled, no IPv6
        # related tests should be performed.  In other words, pretend IPv6
        # isn't present.
        $have_IPv6 = 0
            if grep { $_ eq 'OPENSSL_USE_IPV6=0' } @{$config{CPPDEFINES}};
        # Similarly, if a config target has explicitly disabled IPv6, no
        # IPv6 related tests should be performed.
        $have_IPv6 = 0
            if grep { $_ eq 'OPENSSL_USE_IPV6=0' } @{$target{defines}};
    }
    if ($have_IPv6 < 0) {
        $have_IPv6 = check_IP("::1");
    }
    return $have_IPv6;
}

=head1 SEE ALSO

L<OpenSSL::Test>

=head1 AUTHORS

Stephen Henson E<lt>steve@openssl.orgE<gt> and
Richard Levitte E<lt>levitte@openssl.orgE<gt>

=cut

1;
