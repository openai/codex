# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

package TLSProxy::NewSessionTicket;

use vars '@ISA';
push @ISA, 'TLSProxy::Message';

sub new_dtls
{
    my $class = shift;

    my ($server,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens) = @_;

    return $class->init(
        1,
        $server,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens
    )
}

sub new
{
    my $class = shift;

    my ($server,
        $data,
        $records,
        $startoffset,
        $message_frag_lens) = @_;

    return $class->init(
        0,
        $server,
        0, # msgseq
        0, # msgfrag
        0, # $msgfragoffs
        $data,
        $records,
        $startoffset,
        $message_frag_lens
    )
}

sub init{
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
        TLSProxy::Message::MT_NEW_SESSION_TICKET,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens);

    $self->{ticket_lifetime_hint} = 0;
    $self->{ticket} = "";

    return $self;
}

sub parse
{
    my $self = shift;

    my $ticket_lifetime_hint = unpack('N', $self->data);
    my $ticket_len = unpack('n', $self->data);
    my $ticket = substr($self->data, 6, $ticket_len);

    $self->ticket_lifetime_hint($ticket_lifetime_hint);
    $self->ticket($ticket);
}


#Reconstruct the on-the-wire message data following changes
sub set_message_contents
{
    my $self = shift;
    my $data;

    $data = pack('N', $self->ticket_lifetime_hint);
    $data .= pack('n', length($self->ticket));
    $data .= $self->ticket;

    $self->data($data);
}

#Read/write accessors
sub ticket_lifetime_hint
{
    my $self = shift;
    if (@_) {
      $self->{ticket_lifetime_hint} = shift;
    }
    return $self->{ticket_lifetime_hint};
}
sub ticket
{
    my $self = shift;
    if (@_) {
      $self->{ticket} = shift;
    }
    return $self->{ticket};
}
1;
