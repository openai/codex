# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

package TLSProxy::HelloVerifyRequest;

use TLSProxy::Record;

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
        TLSProxy::Message::MT_HELLO_VERIFY_REQUEST,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens);

    $self->{server_version} = 0;
    $self->{cookie_len} = 0;
    $self->{cookie} = "";

    return $self;
}

sub parse
{
    my $self = shift;

    my ($server_version) = unpack('n', $self->data);
    my $ptr = 2;
    my $cookie_len = unpack('C', substr($self->data, $ptr));
    $ptr++;
    my $cookie = substr($self->data, $ptr, $cookie_len);

    $self->server_version($server_version);
    $self->cookie_len($cookie_len);
    $self->cookie($cookie);

    $self->process_data();

    print "    Server Version:".$TLSProxy::Record::tls_version{$server_version}."\n";
    print "    Cookie Len:".$cookie_len."\n";
}

#Perform any actions necessary based on the data we've seen
sub process_data
{
    my $self = shift;
    #Intentional no-op
}

#Reconstruct the on-the-wire message data following changes
sub set_message_contents
{
    my $self = shift;
    my $data;

    $data = pack('n', $self->server_version);
    $data .= pack('C', $self->cookie_len);
    $data .= $self->cookie;

    $self->data($data);
}

#Read/write accessors
sub server_version
{
    my $self = shift;
    if (@_) {
      $self->{server_version} = shift;
    }
    return $self->{server_version};
}
sub cookie_len
{
    my $self = shift;
    if (@_) {
      $self->{cookie_len} = shift;
    }
    return $self->{cookie_len};
}
sub cookie
{
    my $self = shift;
    if (@_) {
      $self->{cookie} = shift;
    }
    return $self->{cookie};
}
1;
