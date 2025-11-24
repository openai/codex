# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

package TLSProxy::ServerKeyExchange;

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
        TLSProxy::Message::MT_SERVER_KEY_EXCHANGE,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens);

    #DHE
    $self->{p} = "";
    $self->{g} = "";
    $self->{pub_key} = "";
    $self->{sigalg} = -1;
    $self->{sig} = "";

    return $self;
}

sub parse
{
    my $self = shift;
    my $sigalg = -1;

    #Minimal SKE parsing. Only supports one known DHE ciphersuite at the moment
    return if TLSProxy::Proxy->ciphersuite()
                 != TLSProxy::Message::CIPHER_ADH_AES_128_SHA
              && TLSProxy::Proxy->ciphersuite()
                 != TLSProxy::Message::CIPHER_DHE_RSA_AES_128_SHA;

    my $p_len = unpack('n', $self->data);
    my $ptr = 2;
    my $p = substr($self->data, $ptr, $p_len);
    $ptr += $p_len;

    my $g_len = unpack('n', substr($self->data, $ptr));
    $ptr += 2;
    my $g = substr($self->data, $ptr, $g_len);
    $ptr += $g_len;

    my $pub_key_len = unpack('n', substr($self->data, $ptr));
    $ptr += 2;
    my $pub_key = substr($self->data, $ptr, $pub_key_len);
    $ptr += $pub_key_len;

    #We assume its signed
    my $record = ${$self->records}[0];

    if (TLSProxy::Proxy->is_tls13()
            || $record->version() == TLSProxy::Record::VERS_TLS_1_2) {
        $sigalg = unpack('n', substr($self->data, $ptr));
        $ptr += 2;
    }
    my $sig = "";
    if (defined $sigalg) {
        my $sig_len = unpack('n', substr($self->data, $ptr));
        if (defined $sig_len) {
            $ptr += 2;
            $sig = substr($self->data, $ptr, $sig_len);
            $ptr += $sig_len;
        }
    }

    $self->p($p);
    $self->g($g);
    $self->pub_key($pub_key);
    $self->sigalg($sigalg) if defined $sigalg;
    $self->signature($sig);
}


#Reconstruct the on-the-wire message data following changes
sub set_message_contents
{
    my $self = shift;
    my $data;

    $data = pack('n', length($self->p));
    $data .= $self->p;
    $data .= pack('n', length($self->g));
    $data .= $self->g;
    $data .= pack('n', length($self->pub_key));
    $data .= $self->pub_key;
    $data .= pack('n', $self->sigalg) if ($self->sigalg != -1);
    if (length($self->signature) > 0) {
        $data .= pack('n', length($self->signature));
        $data .= $self->signature;
    }

    $self->data($data);
}

#Read/write accessors
#DHE
sub p
{
    my $self = shift;
    if (@_) {
      $self->{p} = shift;
    }
    return $self->{p};
}
sub g
{
    my $self = shift;
    if (@_) {
      $self->{g} = shift;
    }
    return $self->{g};
}
sub pub_key
{
    my $self = shift;
    if (@_) {
      $self->{pub_key} = shift;
    }
    return $self->{pub_key};
}
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
      $self->{sig} = shift;
    }
    return $self->{sig};
}
1;
