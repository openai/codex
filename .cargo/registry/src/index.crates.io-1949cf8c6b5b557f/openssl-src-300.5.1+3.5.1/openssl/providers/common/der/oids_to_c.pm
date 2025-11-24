#! /usr/bin/env perl
# Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;
use warnings;

package oids_to_c;

use Carp;
use File::Spec;
use OpenSSL::OID;

my $OID_name_re = qr/([a-z](?:[-_A-Za-z0-9]*[A-Za-z0-9])?)/;
my $OID_value_re = qr/(\{.*?\})/s;
my $OID_def_re = qr/
                       ${OID_name_re} \s+ OBJECT \s+ IDENTIFIER \s*
                       ::=
                       \s* ${OID_value_re}
                   /x;

sub filter_to_H {
    my ($name, $comment) = @{ shift() };
    my @oid_nums = @_;
    my $oid_size = scalar @oid_nums;

    (my $C_comment = $comment) =~ s|^| * |msg;
    $C_comment = "\n/*\n${C_comment}\n */" if $C_comment ne '';
    (my $C_name = $name) =~ s|-|_|g;
    my $C_bytes_size = 2 + scalar @_;
    my $C_bytes = join(', ', map { sprintf("0x%02X", $_) } @oid_nums );

    return <<"_____";
$C_comment
#define DER_OID_V_${C_name} DER_P_OBJECT, $oid_size, ${C_bytes}
#define DER_OID_SZ_${C_name} ${C_bytes_size}
extern const unsigned char ossl_der_oid_${C_name}[DER_OID_SZ_${C_name}];
_____
}

sub filter_to_C {
    my ($name, $comment) = @{ shift() };
    my @oid_nums = @_;
    my $oid_size = scalar @oid_nums;

    croak "Unsupported OID size (>127 bytes)" if $oid_size > 127;

    (my $C_comment = $comment) =~ s|^| * |msg;
    $C_comment = "\n/*\n${C_comment}\n */" if $C_comment ne '';
    (my $C_name = $name) =~ s|-|_|g;
    my $C_bytes_size = 2 + $oid_size;

    return <<"_____";
$C_comment
const unsigned char ossl_der_oid_${C_name}[DER_OID_SZ_${C_name}] = {
    DER_OID_V_${C_name}
};
_____
}

sub _process {
    my %opts = %{ pop @_ } if ref $_[$#_] eq 'HASH';

    # To maintain input order
    my @OID_names = ();

    foreach my $file (@_) {
        my $input = File::Spec->catfile($opts{dir}, $file);
        open my $fh, $input or die "Reading $input: $!\n";

        my $text = join('',
                        map {
                            s|--.*(\R)$|$1|;
                            $_;
                        } <$fh>);
        # print STDERR "-----BEGIN DEBUG-----\n";
        # print STDERR $text;
        # print STDERR "-----END DEBUG-----\n";
        use re 'debugcolor';
        while ($text =~ m/${OID_def_re}/sg) {
            my $comment = $&;
            my $name = $1;
            my $value = $2;

            # print STDERR "-----BEGIN DEBUG $name-----\n";
            # print STDERR $value,"\n";
            # print STDERR "-----END DEBUG $name-----\n";
            register_oid($name, $value);
            push @OID_names, [ $name, $comment ];
        }
    }

    return @OID_names;
}

sub process_leaves {
    my %opts = %{ $_[$#_] } if ref $_[$#_] eq 'HASH';
    my @OID_names = _process @_;

    my $text = '';
    my %leaves = map { $_ => 1 } registered_oid_leaves;
    foreach (grep { defined $leaves{$_->[0]} } @OID_names) {
        my $lines = $opts{filter}->($_, encode_oid($_->[0]));
        $text .= $lines;
    }
    return $text;
}

1;
