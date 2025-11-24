# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

package TLSProxy::Certificate;

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
        TLSProxy::Message::MT_CERTIFICATE,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens);

    $self->{first_certificate} = "";
    $self->{extension_data} = "";
    $self->{remaining_certdata} = "";

    return $self;
}

sub parse
{
    my $self = shift;

    if (TLSProxy::Proxy->is_tls13()) {
        my $context_len = unpack('C', $self->data);
        my $context = substr($self->data, 1, $context_len);

        my $remdata = substr($self->data, 1 + $context_len);

        my ($hicertlistlen, $certlistlen) = unpack('Cn', $remdata);
        $certlistlen += ($hicertlistlen << 16);

        $remdata = substr($remdata, 3);

        die "Invalid Certificate List length"
            if length($remdata) != $certlistlen;

        my ($hicertlen, $certlen) = unpack('Cn', $remdata);
        $certlen += ($hicertlen << 16);

        die "Certificate too long" if ($certlen + 3) > $certlistlen;

        $remdata = substr($remdata, 3);

        my $certdata = substr($remdata, 0, $certlen);

        $remdata = substr($remdata, $certlen);

        my $extensions_len = unpack('n', $remdata);
        $remdata = substr($remdata, 2);

        die "Extensions too long"
            if ($certlen + 3 + $extensions_len + 2) > $certlistlen;

        my $extension_data = "";
        if ($extensions_len != 0) {
            $extension_data = substr($remdata, 0, $extensions_len);

            if (length($extension_data) != $extensions_len) {
                die "Invalid extension length\n";
            }
        }
        my %extensions = ();
        while (length($extension_data) >= 4) {
            my ($type, $size) = unpack("nn", $extension_data);
            my $extdata = substr($extension_data, 4, $size);
            $extension_data = substr($extension_data, 4 + $size);
            $extensions{$type} = $extdata;
        }
        $remdata = substr($remdata, $extensions_len);

        $self->context($context);
        $self->first_certificate($certdata);
        $self->extension_data(\%extensions);
        $self->remaining_certdata($remdata);

        print "    Context:".$context."\n";
        print "    Certificate List Len:".$certlistlen."\n";
        print "    Certificate Len:".$certlen."\n";
        print "    Extensions Len:".$extensions_len."\n";
    } else {
        my ($hicertlistlen, $certlistlen) = unpack('Cn', $self->data);
        $certlistlen += ($hicertlistlen << 16);

        my $remdata = substr($self->data, 3);

        die "Invalid Certificate List length"
            if length($remdata) != $certlistlen;

        my ($hicertlen, $certlen) = unpack('Cn', $remdata);
        $certlen += ($hicertlen << 16);

        die "Certificate too long" if ($certlen + 3) > $certlistlen;

        $remdata = substr($remdata, 3);

        my $certdata = substr($remdata, 0, $certlen);

        $remdata = substr($remdata, $certlen);

        $self->first_certificate($certdata);
        $self->remaining_certdata($remdata);

        print "    Certificate List Len:".$certlistlen."\n";
        print "    Certificate Len:".$certlen."\n";
    }
}

#Reconstruct the on-the-wire message data following changes
sub set_message_contents
{
    my $self = shift;
    my $data;
    my $extensions = "";

    if (TLSProxy::Proxy->is_tls13()) {
        foreach my $key (keys %{$self->extension_data}) {
            my $extdata = ${$self->extension_data}{$key};
            $extensions .= pack("n", $key);
            $extensions .= pack("n", length($extdata));
            $extensions .= $extdata;
        }
        $data = pack('C', length($self->context()));
        $data .= $self->context;
        my $certlen = length($self->first_certificate);
        my $certlistlen = $certlen + length($extensions)
                          + length($self->remaining_certdata);
        my $hi = $certlistlen >> 16;
        $certlistlen = $certlistlen & 0xffff;
        $data .= pack('Cn', $hi, $certlistlen);
        $hi = $certlen >> 16;
        $certlen = $certlen & 0xffff;
        $data .= pack('Cn', $hi, $certlen);
        $data .= pack('n', length($extensions));
        $data .= $extensions;
        $data .= $self->remaining_certdata();
        $self->data($data);
    } else {
        my $certlen = length($self->first_certificate);
        my $certlistlen = $certlen + length($self->remaining_certdata);
        my $hi = $certlistlen >> 16;
        $certlistlen = $certlistlen & 0xffff;
        $data .= pack('Cn', $hi, $certlistlen);
        $hi = $certlen >> 16;
        $certlen = $certlen & 0xffff;
        $data .= pack('Cn', $hi, $certlen);
        $data .= $self->remaining_certdata();
        $self->data($data);
    }
}

#Read/write accessors
sub context
{
    my $self = shift;
    if (@_) {
      $self->{context} = shift;
    }
    return $self->{context};
}
sub first_certificate
{
    my $self = shift;
    if (@_) {
      $self->{first_certificate} = shift;
    }
    return $self->{first_certificate};
}
sub remaining_certdata
{
    my $self = shift;
    if (@_) {
      $self->{remaining_certdata} = shift;
    }
    return $self->{remaining_certdata};
}
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
