# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

package TLSProxy::EncryptedExtensions;

use vars '@ISA';
push @ISA, 'TLSProxy::Message';

sub new
{
    my $class = shift;
    my ($isdtls,
        $server,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens) = @_;

    my $self = $class->SUPER::new(
        $isdtls,
        $server,
        TLSProxy::Message::MT_ENCRYPTED_EXTENSIONS,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens);

    $self->{extension_data} = "";

    return $self;
}

sub parse
{
    my $self = shift;

    my $extensions_len = unpack('n', $self->data);
    if (!defined $extensions_len) {
        $extensions_len = 0;
    }

    my $extension_data;
    if ($extensions_len != 0) {
        $extension_data = substr($self->data, 2);

        if (length($extension_data) != $extensions_len) {
            die "Invalid extension length\n";
        }
    } else {
        if (length($self->data) != 2) {
            die "Invalid extension length\n";
        }
        $extension_data = "";
    }
    my %extensions = ();
    while (length($extension_data) >= 4) {
        my ($type, $size) = unpack("nn", $extension_data);
        my $extdata = substr($extension_data, 4, $size);
        $extension_data = substr($extension_data, 4 + $size);
        $extensions{$type} = $extdata;
    }

    $self->extension_data(\%extensions);

    print "    Extensions Len:".$extensions_len."\n";
}

#Reconstruct the on-the-wire message data following changes
sub set_message_contents
{
    my $self = shift;
    my $data;
    my $extensions = "";

    foreach my $key (keys %{$self->extension_data}) {
        my $extdata = ${$self->extension_data}{$key};
        $extensions .= pack("n", $key);
        $extensions .= pack("n", length($extdata));
        $extensions .= $extdata;
    }

    $data = pack('n', length($extensions));
    $data .= $extensions;
    $self->data($data);
}

#Read/write accessors
sub extension_data
{
    my $self = shift;
    if (@_) {
        $self->{extension_data} = shift;
    }
    return $self->{extension_data};
}
sub set_extension
{
    my ($self, $ext_type, $ext_data) = @_;
    $self->{extension_data}{$ext_type} = $ext_data;
}
sub delete_extension
{
    my ($self, $ext_type) = @_;
    delete $self->{extension_data}{$ext_type};
}
1;
