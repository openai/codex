# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

use TLSProxy::Proxy;

package TLSProxy::Record;

my $server_encrypting = 0;
my $client_encrypting = 0;
my $etm = 0;

use constant DTLS_RECORD_HEADER_LENGTH => 13;
use constant TLS_RECORD_HEADER_LENGTH => 5;

#Record types
use constant {
    RT_APPLICATION_DATA => 23,
    RT_HANDSHAKE => 22,
    RT_ALERT => 21,
    RT_CCS => 20,
    RT_UNKNOWN => 100
};

my %record_type = (
    RT_APPLICATION_DATA, "APPLICATION DATA",
    RT_HANDSHAKE, "HANDSHAKE",
    RT_ALERT, "ALERT",
    RT_CCS, "CCS",
    RT_UNKNOWN, "UNKNOWN"
);

use constant {
    VERS_DTLS_1_2 => 0xfefd,
    VERS_DTLS_1 => 0xfeff,
    VERS_TLS_1_4 => 0x0305,
    VERS_TLS_1_3 => 0x0304,
    VERS_TLS_1_2 => 0x0303,
    VERS_TLS_1_1 => 0x0302,
    VERS_TLS_1_0 => 0x0301,
    VERS_SSL_3_0 => 0x0300,
    VERS_SSL_LT_3_0 => 0x02ff
};

our %tls_version = (
    VERS_DTLS_1_2, "DTLS1.2",
    VERS_DTLS_1, "DTLS1",
    VERS_TLS_1_3, "TLS1.3",
    VERS_TLS_1_2, "TLS1.2",
    VERS_TLS_1_1, "TLS1.1",
    VERS_TLS_1_0, "TLS1.0",
    VERS_SSL_3_0, "SSL3",
    VERS_SSL_LT_3_0, "SSL<3"
);

#Class method to extract records from a packet of data
sub get_records
{
    my $class = shift;
    my $server = shift;
    my $flight = shift;
    my $packet = shift;
    my $isdtls = shift;
    my $partial = "";
    my @record_list = ();
    my @message_list = ();
    my $record_hdr_len = $isdtls ? DTLS_RECORD_HEADER_LENGTH
                                 : TLS_RECORD_HEADER_LENGTH;

    my $recnum = 1;
    while (length ($packet) > 0) {
        print " Record $recnum ", $server ? "(server -> client)\n"
                                          : "(client -> server)\n";

        my $content_type;
        my $version;
        my $len;
        my $epoch;
        my $seq;

        if ($isdtls) {
            my $seqhi;
            my $seqmi;
            my $seqlo;
            #Get the record header (unpack can't fail if $packet is too short)
            ($content_type, $version, $epoch,
                $seqhi, $seqmi, $seqlo, $len) = unpack('Cnnnnnn', $packet);
            $seq = ($seqhi << 32) | ($seqmi << 16) | $seqlo
        } else {
            #Get the record header (unpack can't fail if $packet is too short)
            ($content_type, $version, $len) = unpack('Cnn', $packet);
        }

        if (length($packet) < $record_hdr_len + ($len // 0)) {
            print "Partial data : ".length($packet)." bytes\n";
            $partial = $packet;
            last;
        }

        my $data = substr($packet, $record_hdr_len, $len);

        print "  Content type: ".$record_type{$content_type}."\n";
        print "  Version: $tls_version{$version}\n";
        if($isdtls) {
            print "  Epoch: $epoch\n";
            print "  Sequence: $seq\n";
        }
        print "  Length: $len\n";

        my $record;
        if ($isdtls) {
            $record = TLSProxy::Record->new_dtls(
                $flight,
                $content_type,
                $version,
                $epoch,
                $seq,
                $len,
                0,
                $len,       # len_real
                $len,       # decrypt_len
                $data,      # data
                $data       # decrypt_data
            );
        } else {
            $record = TLSProxy::Record->new(
                $flight,
                $content_type,
                $version,
                $len,
                0,
                $len,  # len_real
                $len,  # decrypt_len
                $data, # data
                $data  # decrypt_data
            );
        }

        if ($content_type != RT_CCS
                && (!TLSProxy::Proxy->is_tls13()
                    || $content_type != RT_ALERT)) {
            if (($server && $server_encrypting)
                     || (!$server && $client_encrypting)) {
                if (!TLSProxy::Proxy->is_tls13() && $etm) {
                    $record->decryptETM();
                } else {
                    $record->decrypt();
                }
                $record->encrypted(1);

                if (TLSProxy::Proxy->is_tls13()) {
                    print "  Inner content type: "
                          .$record_type{$record->content_type()}."\n";
                }
            }
        }

        push @record_list, $record;

        #Now figure out what messages are contained within this record
        my @messages = TLSProxy::Message->get_messages($server, $record, $isdtls);
        push @message_list, @messages;

        $packet = substr($packet, $record_hdr_len + $len);
        $recnum++;
    }

    return (\@record_list, \@message_list, $partial);
}

sub clear
{
    $server_encrypting = 0;
    $client_encrypting = 0;
}

#Class level accessors
sub server_encrypting
{
    my $class = shift;
    if (@_) {
      $server_encrypting = shift;
    }
    return $server_encrypting;
}
sub client_encrypting
{
    my $class = shift;
    if (@_) {
      $client_encrypting= shift;
    }
    return $client_encrypting;
}
#Enable/Disable Encrypt-then-MAC
sub etm
{
    my $class = shift;
    if (@_) {
      $etm = shift;
    }
    return $etm;
}

sub new_dtls
{
    my $class = shift;
    my ($flight,
        $content_type,
        $version,
        $epoch,
        $seq,
        $len,
        $sslv2,
        $len_real,
        $decrypt_len,
        $data,
        $decrypt_data) = @_;
    return $class->init(1,
        $flight,
        $content_type,
        $version,
        $epoch,
        $seq,
        $len,
        $sslv2,
        $len_real,
        $decrypt_len,
        $data,
        $decrypt_data);
}

sub new
{
    my $class = shift;
    my ($flight,
        $content_type,
        $version,
        $len,
        $sslv2,
        $len_real,
        $decrypt_len,
        $data,
        $decrypt_data) = @_;
    return $class->init(
        0,
        $flight,
        $content_type,
        $version,
        0, #epoch
        0, #seq
        $len,
        $sslv2,
        $len_real,
        $decrypt_len,
        $data,
        $decrypt_data);
}

sub init
{
    my $class = shift;
    my ($isdtls,
        $flight,
        $content_type,
        $version,
        $epoch,
        $seq,
        $len,
        $sslv2,
        $len_real,
        $decrypt_len,
        $data,
        $decrypt_data) = @_;

    my $self = {
        isdtls => $isdtls,
        flight => $flight,
        content_type => $content_type,
        version => $version,
        epoch => $epoch,
        seq => $seq,
        len => $len,
        sslv2 => $sslv2,
        len_real => $len_real,
        decrypt_len => $decrypt_len,
        data => $data,
        decrypt_data => $decrypt_data,
        orig_decrypt_data => $decrypt_data,
        sent => 0,
        encrypted => 0,
        outer_content_type => RT_APPLICATION_DATA
    };

    return bless $self, $class;
}

#Decrypt using encrypt-then-MAC
sub decryptETM
{
    my ($self) = shift;

    my $data = $self->data;

    if($self->version >= VERS_TLS_1_1()) {
        #TLS1.1+ has an explicit IV. Throw it away
        $data = substr($data, 16);
    }

    #Throw away the MAC (assumes MAC is 20 bytes for now. FIXME)
    $data = substr($data, 0, length($data) - 20);

    #Find out what the padding byte is
    my $padval = unpack("C", substr($data, length($data) - 1));

    #Throw away the padding
    $data = substr($data, 0, length($data) - ($padval + 1));

    $self->decrypt_data($data);
    $self->decrypt_len(length($data));

    return $data;
}

#Standard decrypt
sub decrypt()
{
    my ($self) = shift;
    my $mactaglen = 20;
    my $data = $self->data;

    #Throw away any IVs
    if (TLSProxy::Proxy->is_tls13()) {
        #A TLS1.3 client, when processing the server's initial flight, could
        #respond with either an encrypted or an unencrypted alert.
        if ($self->content_type() == RT_ALERT) {
            #TODO(TLS1.3): Eventually it is sufficient just to check the record
            #content type. If an alert is encrypted it will have a record
            #content type of application data. However we haven't done the
            #record layer changes yet, so it's a bit more complicated. For now
            #we will additionally check if the data length is 2 (1 byte for
            #alert level, 1 byte for alert description). If it is, then this is
            #an unencrypted alert, so don't try to decrypt
            return $data if (length($data) == 2);
        }
        $mactaglen = 16;
    } elsif ($self->version >= VERS_TLS_1_1()) {
        #16 bytes for a standard IV
        $data = substr($data, 16);

        #Find out what the padding byte is
        my $padval = unpack("C", substr($data, length($data) - 1));

        #Throw away the padding
        $data = substr($data, 0, length($data) - ($padval + 1));
    }

    #Throw away the MAC or TAG
    $data = substr($data, 0, length($data) - $mactaglen);

    if (TLSProxy::Proxy->is_tls13()) {
        #Get the content type
        my $content_type = unpack("C", substr($data, length($data) - 1));
        $self->content_type($content_type);
        $data = substr($data, 0, length($data) - 1);
    }

    $self->decrypt_data($data);
    $self->decrypt_len(length($data));

    return $data;
}

#Reconstruct the on-the-wire record representation
sub reconstruct_record
{
    my $self = shift;
    my $server = shift;
    my $data;

    #We only replay the records in the same direction
    if ($self->{sent} || ($self->flight & 1) != $server) {
        return "";
    }
    $self->{sent} = 1;

    if ($self->sslv2) {
        $data = pack('n', $self->len | 0x8000);
    } else {
        if($self->{isdtls}) {
            my $seqhi = ($self->seq >> 32) & 0xffff;
            my $seqmi = ($self->seq >> 16) & 0xffff;
            my $seqlo = ($self->seq >> 0) & 0xffff;
            $data = pack('Cnnnnnn', $self->content_type, $self->version,
                         $self->epoch, $seqhi, $seqmi, $seqlo, $self->len);
        } else {
            if (TLSProxy::Proxy->is_tls13() && $self->encrypted) {
                $data = pack('Cnn', $self->outer_content_type, $self->version,
                             $self->len);
            }
            else {
                $data = pack('Cnn', $self->content_type, $self->version,
                             $self->len);
            }
        }

    }
    $data .= $self->data;

    return $data;
}

#Read only accessors
sub flight
{
    my $self = shift;
    return $self->{flight};
}
sub sslv2
{
    my $self = shift;
    return $self->{sslv2};
}
sub len_real
{
    my $self = shift;
    return $self->{len_real};
}
sub orig_decrypt_data
{
    my $self = shift;
    return $self->{orig_decrypt_data};
}

#Read/write accessors
sub decrypt_len
{
    my $self = shift;
    if (@_) {
      $self->{decrypt_len} = shift;
    }
    return $self->{decrypt_len};
}
sub data
{
    my $self = shift;
    if (@_) {
      $self->{data} = shift;
    }
    return $self->{data};
}
sub decrypt_data
{
    my $self = shift;
    if (@_) {
      $self->{decrypt_data} = shift;
    }
    return $self->{decrypt_data};
}
sub len
{
    my $self = shift;
    if (@_) {
      $self->{len} = shift;
    }
    return $self->{len};
}
sub version
{
    my $self = shift;
    if (@_) {
      $self->{version} = shift;
    }
    return $self->{version};
}
sub content_type
{
    my $self = shift;
    if (@_) {
      $self->{content_type} = shift;
    }
    return $self->{content_type};
}
sub epoch
{
    my $self = shift;
    if (@_) {
        $self->{epoch} = shift;
    }
    return $self->{epoch};
}
sub seq
{
    my $self = shift;
    if (@_) {
        $self->{seq} = shift;
    }
    return $self->{seq};
}
sub encrypted
{
    my $self = shift;
    if (@_) {
      $self->{encrypted} = shift;
    }
    return $self->{encrypted};
}
sub outer_content_type
{
    my $self = shift;
    if (@_) {
      $self->{outer_content_type} = shift;
    }
    return $self->{outer_content_type};
}
sub is_fatal_alert
{
    my $self = shift;
    my $server = shift;

    if (($self->{flight} & 1) == $server && $self->{content_type} == RT_ALERT) {
        my ($level, $description) = unpack('CC', $self->decrypt_data);
        return $description if ($level == 2);
    }
    return 0;
}
1;
