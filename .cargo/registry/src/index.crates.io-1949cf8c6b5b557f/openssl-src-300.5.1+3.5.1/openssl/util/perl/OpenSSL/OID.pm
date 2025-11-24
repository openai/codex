# Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# Author note: this is originally RL::ASN1::OID,
# repurposed by the author for OpenSSL use.

package OpenSSL::OID;

use 5.10.0;
use strict;
use warnings;
use Carp;

use Exporter;
use vars qw($VERSION @ISA @EXPORT @EXPORT_OK %EXPORT_TAGS);
@ISA = qw(Exporter);
@EXPORT = qw(parse_oid encode_oid register_oid
             registered_oid_arcs registered_oid_leaves);
@EXPORT_OK = qw(encode_oid_nums);

# Unfortunately, the pairwise List::Util functionality came with perl
# v5.19.3, and I want to target absolute compatibility with perl 5.10
# and up.  That means I have to implement quick pairwise functions here.

#use List::Util;
sub _pairs (@);
sub _pairmap (&@);

=head1 NAME

OpenSSL::OID - an OBJECT IDENTIFIER parser / encoder

=head1 VERSION

Version 0.1

=cut

our $VERSION = '0.1';


=head1 SYNOPSIS

    use OpenSSL::OID;

    # This gives the array ( 1 2 840 113549 1 1 )
    my @nums = parse_oid('{ pkcs-1 1 }');

    # This gives the array of DER encoded bytes for the OID, i.e.
    # ( 42, 134, 72, 134, 247, 13, 1, 1 )
    my @bytes = encode_oid('{ pkcs-1 1 }');

    # This registers a name with an OID.  It's saved internally and
    # serves as repository of names for further parsing, such as 'pkcs-1'
    # in the strings used above.
    register_object('pkcs-1', '{ pkcs 1 }');


    use OpenSSL::OID qw(:DEFAULT encode_oid_nums);

    # This does the same as encode_oid(), but takes the output of
    # parse_oid() as input.
    my @bytes = encode_oid_nums(@nums);

=head1 EXPORT

The functions parse_oid and encode_oid are exported by default.
The function encode_oid_nums() can be exported explicitly.

=cut

######## REGEXPS

# ASN.1 object identifiers come in two forms: 1) the bracketed form
#(referred to as ObjectIdentifierValue in X.690), 2) the dotted form
#(referred to as XMLObjIdentifierValue in X.690)
#
# examples of 1 (these are all the OID for rsaEncrypted):
#
# { iso (1) 2 840 11349 1 1 }
# { pkcs 1 1 }
# { pkcs1 1 }
#
# examples of 2:
#
# 1.2.840.113549.1.1
# pkcs.1.1
# pkcs1.1
#
my $identifier_re = qr/[a-z](?:[-_A-Za-z0-9]*[A-Za-z0-9])?/;
# The only difference between $objcomponent_re and $xmlobjcomponent_re is
# the separator in the top branch.  Each component is always parsed in two
# groups, so we get a pair of values regardless.  That's the reason for the
# empty parentheses.
# Because perl doesn't try to do an exhaustive try of every branch it rather
# stops on the first that matches, we need to have them in order of longest
# to shortest where there may be ambiguity.
my $objcomponent_re = qr/(?|
                             (${identifier_re}) \s* \((\d+)\)
                         |
                             (${identifier_re}) ()
                         |
                             ()(\d+)
                         )/x;
my $xmlobjcomponent_re = qr/(?|
                                (${identifier_re}) \. \((\d+)\)
                            |
                                (${identifier_re}) ()
                            |
                                () (\d+)
                            )/x;

my $obj_re =
    qr/(?: \{ \s* (?: ${objcomponent_re} \s+ )* ${objcomponent_re} \s* \} )/x;
my $xmlobj_re =
    qr/(?: (?: ${xmlobjcomponent_re} \. )* ${xmlobjcomponent_re} )/x;

######## NAME TO OID REPOSITORY

# Recorded OIDs, to support things like '{ pkcs1 1 }'
# Do note that we don't currently support relative OIDs
#
# The key is the identifier.
#
# The value is a hash, composed of:
# type => 'arc' | 'leaf'
# nums => [ LIST ]
# Note that the |type| always starts as a 'leaf', and may change to an 'arc'
# on the fly, as new OIDs are parsed.
my %name2oid = ();

########

=head1 SUBROUTINES/METHODS

=over 4

=item parse_oid()

TBA

=cut

sub parse_oid {
    my $input = shift;

    croak "Invalid extra arguments" if (@_);

    # The components become a list of ( identifier, number ) pairs,
    # where they can also be the empty string if they are not present
    # in the input.
    my @components;
    if ($input =~ m/^\s*(${obj_re})\s*$/x) {
        my $oid = $1;
        @components = ( $oid =~ m/${objcomponent_re}\s*/g );
    } elsif ($input =~ m/^\s*(${xmlobj_re})\s*$/) {
        my $oid = $1;
        @components = ( $oid =~ m/${xmlobjcomponent_re}\.?/g );
    }

    croak "Invalid ASN.1 object '$input'" unless @components;
    die "Internal error when parsing '$input'"
        unless scalar(@components) % 2 == 0;

    # As we currently only support a name without number as first
    # component, the easiest is to have a direct look at it and
    # hack it.
    my @first = _pairmap {
        my ($a, $b) = @$_;
        return $b if $b ne '';
        return @{$name2oid{$a}->{nums}} if $a ne '' && defined $name2oid{$a};
        croak "Undefined identifier $a" if $a ne '';
        croak "Empty OID element (how's that possible?)";
    } ( @components[0..1] );

    my @numbers =
        (
         @first,
         _pairmap {
             my ($a, $b) = @$_;
             return $b if $b ne '';
             croak "Unsupported relative OID $a" if $a ne '';
             croak "Empty OID element (how's that possible?)";
         } @components[2..$#components]
        );

    # If the first component has an identifier and there are other
    # components following it, we change the type of that identifier
    # to 'arc'.
    if (scalar @components > 2
        && $components[0] ne ''
        && defined $name2oid{$components[0]}) {
        $name2oid{$components[0]}->{type} = 'arc';
    }

    return @numbers;
}

=item encode_oid()

=cut

# Forward declaration
sub encode_oid_nums;
sub encode_oid {
    return encode_oid_nums parse_oid @_;
}

=item register_oid()

=cut

sub register_oid {
    my $name = shift;
    my @nums = parse_oid @_;

    if (defined $name2oid{$name}) {
        my $str1 = join(',', @nums);
        my $str2 = join(',', @{$name2oid{$name}->{nums}});

        croak "Invalid redefinition of $name with different value"
            unless $str1 eq $str2;
    } else {
        $name2oid{$name} = { type => 'leaf', nums => [ @nums ] };
    }
}

=item registered_oid_arcs()

=item registered_oid_leaves()

=cut

sub _registered_oids {
    my $type = shift;

    return grep { $name2oid{$_}->{type} eq $type } keys %name2oid;
}

sub registered_oid_arcs {
    return _registered_oids( 'arc' );
}

sub registered_oid_leaves {
    return _registered_oids( 'leaf' );
}

=item encode_oid_nums()

=cut

# Internal helper.  It takes a numeric OID component and generates the
# DER encoding for it.
sub _gen_oid_bytes {
    my $num = shift;
    my $cnt = 0;

    return ( $num ) if $num < 128;
    return ( ( map { $_ | 0x80 } _gen_oid_bytes($num >> 7) ), $num & 0x7f );
}

sub encode_oid_nums {
    my @numbers = @_;

    croak 'Invalid OID values: ( ', join(', ', @numbers), ' )'
        if (scalar @numbers < 2
            || $numbers[0] < 0 || $numbers[0] > 2
            || $numbers[1] < 0 || $numbers[1] > 39);

    my $first = shift(@numbers) * 40 + shift(@numbers);
    @numbers = ( $first, map { _gen_oid_bytes($_) } @numbers );

    return @numbers;
}

=back

=head1 AUTHOR

Richard levitte, C<< <richard at levitte.org> >>

=cut

######## Helpers

sub _pairs (@) {
    croak "Odd number of arguments" if @_ & 1;

    my @pairlist = ();

    while (@_) {
        my $x = [ shift, shift ];
        push @pairlist, $x;
    }
    return @pairlist;
}

sub _pairmap (&@) {
    my $block = shift;
    map { $block->($_) } _pairs @_;
}

1; # End of OpenSSL::OID
