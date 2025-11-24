# Copyright 2016 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

package OpenSSL::Test::Simple;

use strict;
use warnings;

use Exporter;
use vars qw($VERSION @ISA @EXPORT @EXPORT_OK %EXPORT_TAGS);
$VERSION = "0.2";
@ISA = qw(Exporter);
@EXPORT = qw(simple_test);

=head1 NAME

OpenSSL::Test::Simple - a few very simple test functions

=head1 SYNOPSIS

  use OpenSSL::Test::Simple;

  simple_test("my_test_name", "destest", "des");

=head1 DESCRIPTION

Sometimes, the functions in L<OpenSSL::Test> are quite tedious for some
repetitive tasks.  This module provides functions to make life easier.
You could call them hacks if you wish.

=cut

use OpenSSL::Test;
use OpenSSL::Test::Utils;

=over 4

=item B<simple_test NAME, PROGRAM, ALGORITHM>

Runs a test named NAME, running the program PROGRAM with no arguments,
to test the algorithm ALGORITHM.

A complete recipe looks like this:

  use OpenSSL::Test::Simple;

  simple_test("test_bf", "bftest", "bf");

=back

=cut

# args:
#  name			(used with setup())
#  algorithm		(used to check if it's at all supported)
#  name of binary	(the program that does the actual test)
sub simple_test {
    my ($name, $prgr, @algos) = @_;

    setup($name);

    if (scalar(disabled(@algos))) {
	if (scalar(@algos) == 1) {
	    plan skip_all => $algos[0]." is not supported by this OpenSSL build";
	} else {
	    my $last = pop @algos;
	    plan skip_all => join(", ", @algos)." and $last are not supported by this OpenSSL build";
	}
    }

    plan tests => 1;

    ok(run(test([$prgr])), "running $prgr");
}

=head1 SEE ALSO

L<OpenSSL::Test>

=head1 AUTHORS

Richard Levitte E<lt>levitte@openssl.orgE<gt> with inspiration
from Rich Salz E<lt>rsalz@openssl.orgE<gt>.

=cut

1;
