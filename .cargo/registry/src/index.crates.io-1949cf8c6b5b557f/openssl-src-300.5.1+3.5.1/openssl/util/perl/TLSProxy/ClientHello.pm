# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

package TLSProxy::ClientHello;

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
        TLSProxy::Message::MT_CLIENT_HELLO,
        $msgseq,
        $msgfrag,
        $msgfragoffs,
        $data,
        $records,
        $startoffset,
        $message_frag_lens);

    $self->{isdtls} = $isdtls;
    $self->{client_version} = 0;
    $self->{random} = [];
    $self->{session_id_len} = 0;
    $self->{session} = "";
    $self->{legacy_cookie_len} = 0; #DTLS only
    $self->{legacy_cookie} = ""; #DTLS only
    $self->{ciphersuite_len} = 0;
    $self->{ciphersuites} = [];
    $self->{comp_meth_len} = 0;
    $self->{comp_meths} = [];
    $self->{extensions_len} = 0;
    $self->{extension_data} = "";

    return $self;
}

sub parse
{
    my $self = shift;
    my $ptr = 2;
    my ($client_version) = unpack('n', $self->data);
    my $random = substr($self->data, $ptr, 32);
    $ptr += 32;
    my $session_id_len = unpack('C', substr($self->data, $ptr));
    $ptr++;
    my $session = substr($self->data, $ptr, $session_id_len);
    $ptr += $session_id_len;
    my $legacy_cookie_len = 0;
    my $legacy_cookie = "";
    if($self->{isdtls}) {
        $legacy_cookie_len = unpack('C', substr($self->data, $ptr));
        $ptr++;
        $legacy_cookie = substr($self->data, $ptr, $legacy_cookie_len);
        $ptr += $legacy_cookie_len;
    }
    my $ciphersuite_len = unpack('n', substr($self->data, $ptr));
    $ptr += 2;
    my @ciphersuites = unpack('n*', substr($self->data, $ptr,
                                           $ciphersuite_len));
    $ptr += $ciphersuite_len;
    my $comp_meth_len = unpack('C', substr($self->data, $ptr));
    $ptr++;
    my @comp_meths = unpack('C*', substr($self->data, $ptr, $comp_meth_len));
    $ptr += $comp_meth_len;
    my $extensions_len = unpack('n', substr($self->data, $ptr));
    $ptr += 2;
    #For now we just deal with this as a block of data. In the future we will
    #want to parse this
    my $extension_data = substr($self->data, $ptr);

    if (length($extension_data) != $extensions_len) {
        die "Invalid extension length\n";
    }
    my %extensions = ();
    while (length($extension_data) >= 4) {
        my ($type, $size) = unpack("nn", $extension_data);
        my $extdata = substr($extension_data, 4, $size);
        $extension_data = substr($extension_data, 4 + $size);
        $extensions{$type} = $extdata;
    }

    $self->client_version($client_version);
    $self->random($random);
    $self->session_id_len($session_id_len);
    $self->session($session);
    $self->legacy_cookie_len($legacy_cookie_len);
    $self->legacy_cookie($legacy_cookie);
    $self->ciphersuite_len($ciphersuite_len);
    $self->ciphersuites(\@ciphersuites);
    $self->comp_meth_len($comp_meth_len);
    $self->comp_meths(\@comp_meths);
    $self->extensions_len($extensions_len);
    $self->extension_data(\%extensions);

    $self->process_extensions();

    print "    Client Version:".$TLSProxy::Record::tls_version{$client_version}."\n";
    print "    Session ID Len:".$session_id_len."\n";
    if($self->{isdtls}) {
        print "    Legacy Cookie Len:".$legacy_cookie_len."\n";
    }
    print "    Ciphersuite len:".$ciphersuite_len."\n";
    print "    Compression Method Len:".$comp_meth_len."\n";
    print "    Extensions Len:".$extensions_len."\n";
}

#Perform any actions necessary based on the extensions we've seen
sub process_extensions
{
    my $self = shift;
    my %extensions = %{$self->extension_data};

    #Clear any state from a previous run
    TLSProxy::Record->etm(0);

    if (exists $extensions{TLSProxy::Message::EXT_ENCRYPT_THEN_MAC}) {
        TLSProxy::Record->etm(1);
    }
}

sub extension_contents
{
    my $self = shift;
    my $key = shift;
    my $extension = "";

    my $extdata = ${$self->extension_data}{$key};
    $extension .= pack("n", $key);
    $extension .= pack("n", length($extdata));
    $extension .= $extdata;
    return $extension;
}

#Reconstruct the on-the-wire message data following changes
sub set_message_contents
{
    my $self = shift;
    my $data;
    my $extensions = "";

    $data = pack('n', $self->client_version);
    $data .= $self->random;
    $data .= pack('C', $self->session_id_len);
    $data .= $self->session;
    if($self->{isdtls}){
        $data .= pack('C', $self->legacy_cookie_len);
        if($self->legacy_cookie_len > 0) {
            $data .= $self->legacy_cookie;
        }
    }
    $data .= pack('n', $self->ciphersuite_len);
    $data .= pack("n*", @{$self->ciphersuites});
    $data .= pack('C', $self->comp_meth_len);
    $data .= pack("C*", @{$self->comp_meths});

    foreach my $key (keys %{$self->extension_data}) {
        next if ($key == TLSProxy::Message::EXT_PSK);
        $extensions .= $self->extension_contents($key);
        #Add extension twice if we are duplicating that extension
        $extensions .= $self->extension_contents($key) if ($key == $self->dupext);
    }
    #PSK extension always goes last...
    if (defined ${$self->extension_data}{TLSProxy::Message::EXT_PSK}) {
        $extensions .= $self->extension_contents(TLSProxy::Message::EXT_PSK);
    }
    #unless we have EXT_FORCE_LAST
    if (defined ${$self->extension_data}{TLSProxy::Message::EXT_FORCE_LAST}) {
        $extensions .= $self->extension_contents(TLSProxy::Message::EXT_FORCE_LAST);
    }

    $data .= pack('n', length($extensions));
    $data .= $extensions;

    $self->data($data);
}

#Read/write accessors
sub client_version
{
    my $self = shift;
    if (@_) {
      $self->{client_version} = shift;
    }
    return $self->{client_version};
}
sub random
{
    my $self = shift;
    if (@_) {
      $self->{random} = shift;
    }
    return $self->{random};
}
sub session_id_len
{
    my $self = shift;
    if (@_) {
      $self->{session_id_len} = shift;
    }
    return $self->{session_id_len};
}
sub session
{
    my $self = shift;
    if (@_) {
      $self->{session} = shift;
    }
    return $self->{session};
}
sub legacy_cookie_len
{
    my $self = shift;
    if (@_) {
        $self->{legacy_cookie_len} = shift;
    }
    return $self->{legacy_cookie_len};
}
sub legacy_cookie
{
    my $self = shift;
    if (@_) {
        $self->{legacy_cookie} = shift;
    }
    return $self->{legacy_cookie};
}
sub ciphersuite_len
{
    my $self = shift;
    if (@_) {
      $self->{ciphersuite_len} = shift;
    }
    return $self->{ciphersuite_len};
}
sub ciphersuites
{
    my $self = shift;
    if (@_) {
      $self->{ciphersuites} = shift;
    }
    return $self->{ciphersuites};
}
sub comp_meth_len
{
    my $self = shift;
    if (@_) {
      $self->{comp_meth_len} = shift;
    }
    return $self->{comp_meth_len};
}
sub comp_meths
{
    my $self = shift;
    if (@_) {
      $self->{comp_meths} = shift;
    }
    return $self->{comp_meths};
}
sub extensions_len
{
    my $self = shift;
    if (@_) {
      $self->{extensions_len} = shift;
    }
    return $self->{extensions_len};
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
