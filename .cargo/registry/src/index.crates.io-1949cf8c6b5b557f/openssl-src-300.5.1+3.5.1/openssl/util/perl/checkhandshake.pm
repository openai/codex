#! /usr/bin/env perl
# Copyright 2015-2018 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

package checkhandshake;

use OpenSSL::Test qw/:DEFAULT cmdstr srctop_file srctop_dir bldtop_dir/;
use OpenSSL::Test::Utils;
use TLSProxy::Proxy;

use Exporter;
our @ISA = 'Exporter';
our @EXPORT = qw(@handmessages @extensions checkhandshake);

use constant {
    DEFAULT_HANDSHAKE => 1,
    OCSP_HANDSHAKE => 2,
    RESUME_HANDSHAKE => 4,
    CLIENT_AUTH_HANDSHAKE => 8,
    RENEG_HANDSHAKE => 16,
    NPN_HANDSHAKE => 32,
    EC_HANDSHAKE => 64,
    HRR_HANDSHAKE => 128,
    HRR_RESUME_HANDSHAKE => 256,
    CERT_COMP_SRV_HANDSHAKE => 512,
    CERT_COMP_CLI_HANDSHAKE => 1024,
    CERT_COMP_BOTH_HANDSHAKE => 2048,

    ALL_HANDSHAKES => 4095
};

use constant {
    #DEFAULT also includes SESSION_TICKET_SRV_EXTENSION and SERVER_NAME_CLI
    DEFAULT_EXTENSIONS => 0x00000007,
    SESSION_TICKET_SRV_EXTENSION => 0x00000002,
    SERVER_NAME_CLI_EXTENSION => 0x00000004,
    SERVER_NAME_SRV_EXTENSION => 0x00000008,
    STATUS_REQUEST_CLI_EXTENSION => 0x00000010,
    STATUS_REQUEST_SRV_EXTENSION => 0x00000020,
    ALPN_CLI_EXTENSION => 0x00000040,
    ALPN_SRV_EXTENSION => 0x00000080,
    SCT_CLI_EXTENSION => 0x00000100,
    SCT_SRV_EXTENSION => 0x00000200,
    RENEGOTIATE_CLI_EXTENSION => 0x00000400,
    NPN_CLI_EXTENSION => 0x00000800,
    NPN_SRV_EXTENSION => 0x00001000,
    SRP_CLI_EXTENSION => 0x00002000,
    #Client side for ec point formats is a default extension
    EC_POINT_FORMAT_SRV_EXTENSION => 0x00004000,
    PSK_CLI_EXTENSION => 0x00008000,
    PSK_SRV_EXTENSION => 0x00010000,
    KEY_SHARE_SRV_EXTENSION => 0x00020000,
    PSK_KEX_MODES_EXTENSION => 0x00040000,
    KEY_SHARE_HRR_EXTENSION => 0x00080000,
    SUPPORTED_GROUPS_SRV_EXTENSION => 0x00100000,
    POST_HANDSHAKE_AUTH_CLI_EXTENSION => 0x00200000,
    CERT_COMP_CLI_EXTENSION => 0x00400000,
    CERT_COMP_SRV_EXTENSION => 0x00800000
};

our @handmessages = ();
our @extensions = ();

sub checkhandshake($$$$)
{
    my ($proxy, $handtype, $exttype, $testname) = @_;

    subtest $testname => sub {
        my $loop = 0;
        my $numtests;
        my $extcount;
        my $clienthelloseen = 0;

        my $lastmt = 0;
        my $numsh = 0;
        if (TLSProxy::Proxy::is_tls13()) {
            #How many ServerHellos are we expecting?
            for ($numtests = 0; $handmessages[$loop][1] != 0; $loop++) {
                next if (($handmessages[$loop][1] & $handtype) == 0);
                $numsh++ if ($lastmt != TLSProxy::Message::MT_SERVER_HELLO
                             && $handmessages[$loop][0] == TLSProxy::Message::MT_SERVER_HELLO);
                $lastmt = $handmessages[$loop][0];
            }
        }

        #First count the number of tests
        my $nextmess = 0;
        my $message = undef;
        my $chnum = 0;
        my $shnum = 0;
        if (!TLSProxy::Proxy::is_tls13()) {
            # In non-TLSv1.3 we always treat reneg CH and SH like the first CH
            # and SH
            $chnum = 1;
            $shnum = 1;
        }
        #If we're only expecting one ServerHello out of two then we skip the
        #first ServerHello in the list completely
        $shnum++ if ($numsh == 1 && TLSProxy::Proxy::is_tls13());
        $loop = 0;
        for ($numtests = 0; $handmessages[$loop][1] != 0; $loop++) {
            next if (($handmessages[$loop][1] & $handtype) == 0);
            if (scalar @{$proxy->message_list} > $nextmess) {
                $message = ${$proxy->message_list}[$nextmess];
                $nextmess++;
            } else {
                $message = undef;
            }
            $numtests++;

            next if (!defined $message);
            if (TLSProxy::Proxy::is_tls13()) {
                $chnum++ if $message->mt() == TLSProxy::Message::MT_CLIENT_HELLO;
                $shnum++ if $message->mt() == TLSProxy::Message::MT_SERVER_HELLO;
            }
            next if ($message->mt() != TLSProxy::Message::MT_CLIENT_HELLO
                    && $message->mt() != TLSProxy::Message::MT_SERVER_HELLO
                    && $message->mt() !=
                       TLSProxy::Message::MT_ENCRYPTED_EXTENSIONS
                    && $message->mt() != TLSProxy::Message::MT_CERTIFICATE
                    && $message->mt() != TLSProxy::Message::MT_CERTIFICATE_REQUEST);

            next if $message->mt() == TLSProxy::Message::MT_CERTIFICATE
                    && !TLSProxy::Proxy::is_tls13();

            my $extchnum = 1;
            my $extshnum = 1;
            for (my $extloop = 0;
                    $extensions[$extloop][3] != 0;
                    $extloop++) {
                $extchnum = 2 if $extensions[$extloop][0] != TLSProxy::Message::MT_CLIENT_HELLO
                                 && TLSProxy::Proxy::is_tls13();
                $extshnum = 2 if $extensions[$extloop][0] != TLSProxy::Message::MT_SERVER_HELLO
                                 && $extchnum == 2;
                next if $extensions[$extloop][0] == TLSProxy::Message::MT_CLIENT_HELLO
                                 && $extchnum != $chnum;
                next if $extensions[$extloop][0] == TLSProxy::Message::MT_SERVER_HELLO
                                 && $extshnum != $shnum;
                next if ($message->mt() != $extensions[$extloop][0]);
                next if ($message->server() != $extensions[$extloop][2]);
                $numtests++;
            }
            $numtests++;
        }

        plan tests => $numtests;

        $nextmess = 0;
        $message = undef;
        if (TLSProxy::Proxy::is_tls13()) {
            $chnum = 0;
            $shnum = 0;
        } else {
            # In non-TLSv1.3 we always treat reneg CH and SH like the first CH
            # and SH
            $chnum = 1;
            $shnum = 1;
        }
        #If we're only expecting one ServerHello out of two then we skip the
        #first ServerHello in the list completely
        $shnum++ if ($numsh == 1 && TLSProxy::Proxy::is_tls13());
        for ($loop = 0; $handmessages[$loop][1] != 0; $loop++) {
            next if (($handmessages[$loop][1] & $handtype) == 0);
            if (scalar @{$proxy->message_list} > $nextmess) {
                $message = ${$proxy->message_list}[$nextmess];
                $nextmess++;
            } else {
                $message = undef;
            }
            if (!defined $message) {
                fail("Message type check. Got nothing, expected "
                     .$handmessages[$loop][0]);
                next;
            } else {
                ok($message->mt == $handmessages[$loop][0],
                   "Message type check. Got ".$message->mt
                   .", expected ".$handmessages[$loop][0]);
            }
            if (TLSProxy::Proxy::is_tls13()) {
                $chnum++ if $message->mt() == TLSProxy::Message::MT_CLIENT_HELLO;
                $shnum++ if $message->mt() == TLSProxy::Message::MT_SERVER_HELLO;
            }

            next if ($message->mt() != TLSProxy::Message::MT_CLIENT_HELLO
                    && $message->mt() != TLSProxy::Message::MT_SERVER_HELLO
                    && $message->mt() !=
                       TLSProxy::Message::MT_ENCRYPTED_EXTENSIONS
                    && $message->mt() != TLSProxy::Message::MT_CERTIFICATE
                    && $message->mt() != TLSProxy::Message::MT_CERTIFICATE_REQUEST);

            next if $message->mt() == TLSProxy::Message::MT_CERTIFICATE
                    && !TLSProxy::Proxy::is_tls13();

            if ($message->mt() == TLSProxy::Message::MT_CLIENT_HELLO) {
                #Add renegotiate extension we will expect if renegotiating
                $exttype |= RENEGOTIATE_CLI_EXTENSION
                    if ($clienthelloseen && !TLSProxy::Proxy::is_tls13());
                $clienthelloseen = 1;
            }
            #Now check that we saw the extensions we expected
            my $msgexts = $message->extension_data();
            my $extchnum = 1;
            my $extshnum = 1;
            for (my $extloop = 0, $extcount = 0; $extensions[$extloop][3] != 0;
                                $extloop++) {
                #In TLSv1.3 we can have two ClientHellos if there has been a
                #HelloRetryRequest, and they may have different extensions. Skip
                #if these are extensions for a different ClientHello
                $extchnum = 2 if $extensions[$extloop][0] != TLSProxy::Message::MT_CLIENT_HELLO
                                 && TLSProxy::Proxy::is_tls13();
                $extshnum = 2 if $extensions[$extloop][0] != TLSProxy::Message::MT_SERVER_HELLO
                                 && $extchnum == 2;
                next if $extensions[$extloop][0] == TLSProxy::Message::MT_CLIENT_HELLO
                                 && $extchnum != $chnum;
                next if $extensions[$extloop][0] == TLSProxy::Message::MT_SERVER_HELLO
                                 && $extshnum != $shnum;
                next if ($message->mt() != $extensions[$extloop][0]);
                next if ($message->server() != $extensions[$extloop][2]);
                ok (($extensions[$extloop][3] & $exttype) == 0
                      || defined ($msgexts->{$extensions[$extloop][1]}),
                    "Extension presence check (Message: ".$message->mt()
                    ." Extension: ".($extensions[$extloop][3] & $exttype).", "
                    .$extloop.")");
                $extcount++ if (($extensions[$extloop][3] & $exttype) != 0);
            }
            ok($extcount == keys %$msgexts, "Extensions count mismatch ("
                                            .$extcount.", ".(keys %$msgexts)
                                            .")");
        }
    }
}

1;
