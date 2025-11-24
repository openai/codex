# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

package TLSProxy::CertificateVerify;

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
        TLSProxy::Message::MT_CERTIFICATE_VERIFY,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens);

    $self->{sigalg} = -1;
    $self->{signature} = "";

    return $self;
}

sub parse
{
    my $self = shift;

    my $sigalg = -1;
    my $remdata = $self->data;
    my $record = ${$self->records}[0];

    if (TLSProxy::Proxy->is_tls13()
            || $record->version() == TLSProxy::Record::VERS_TLS_1_2
            || $record->version() == TLSProxy::Record::VERS_DTLS_1_2) {
        $sigalg = unpack('n', $remdata);
        $remdata = substr($remdata, 2);
    }

    my $siglen = unpack('n', substr($remdata, 0, 2));
    my $sig = substr($remdata, 2);

    die "Invalid CertificateVerify signature length" if length($sig) != $siglen;

    print "    SigAlg:".$sigalg."\n";
    print "    Signature Len:".$siglen."\n";

    $self->sigalg($sigalg);
    $self->signature($sig);
}

#Reconstruct the on-the-wire message data following changes
sub set_message_contents
{
    my $self = shift;
    my $data = "";
    my $sig = $self->signature();
    my $olddata = $self->data();

    $data .= pack("n", $self->sigalg()) if ($self->sigalg() != -1);
    $data .= pack("n", length($sig));
    $data .= $sig;

    $self->data($data);
}

#Read/write accessors
sub sigalg
{
    my $self = shift;
    if (@_) {
      $self->{sigalg} = shift;
    }
    return $self->{sigalg};
}
sub signature
{
    my $self = shift;
    if (@_) {
      $self->{signature} = shift;
    }
    return $self->{signature};
}
1;
