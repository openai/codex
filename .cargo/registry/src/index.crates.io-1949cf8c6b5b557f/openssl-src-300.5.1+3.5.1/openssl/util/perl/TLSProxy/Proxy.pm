# Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;
use POSIX ":sys_wait_h";

package TLSProxy::Proxy;

use File::Spec;
use IO::Socket;
use IO::Select;
use TLSProxy::Record;
use TLSProxy::Message;
use TLSProxy::ClientHello;
use TLSProxy::ServerHello;
use TLSProxy::HelloVerifyRequest;
use TLSProxy::EncryptedExtensions;
use TLSProxy::Certificate;
use TLSProxy::CertificateRequest;
use TLSProxy::CertificateVerify;
use TLSProxy::ServerKeyExchange;
use TLSProxy::NewSessionTicket;
use TLSProxy::NextProto;

my $have_IPv6;
my $useINET6;
my $IP_factory;

BEGIN
{
    # IO::Socket::IP is on the core module list, IO::Socket::INET6 isn't.
    # However, IO::Socket::INET6 is older and is said to be more widely
    # deployed for the moment, and may have less bugs, so we try the latter
    # first, then fall back on the core modules.  Worst case scenario, we
    # fall back to IO::Socket::INET, only supports IPv4.
    eval {
        require IO::Socket::INET6;
        my $s = IO::Socket::INET6->new(
            LocalAddr => "::1",
            LocalPort => 0,
            Listen=>1,
            );
        $s or die "\n";
        $s->close();
    };
    if ($@ eq "") {
        $IP_factory = sub { IO::Socket::INET6->new(Domain => AF_INET6, @_); };
        $have_IPv6 = 1;
        $useINET6 = 1;
    } else {
        eval {
            require IO::Socket::IP;
            my $s = IO::Socket::IP->new(
                LocalAddr => "::1",
                LocalPort => 0,
                Listen=>1,
                );
            $s or die "\n";
            $s->close();
        };
        if ($@ eq "") {
            $IP_factory = sub { IO::Socket::IP->new(@_); };
            $have_IPv6 = 1;
            $useINET6 = 0;
        } else {
            $IP_factory = sub { IO::Socket::INET->new(@_); };
            $have_IPv6 = 0;
            $useINET6 = 0;
        }
    }
}

my $is_tls13 = 0;
my $ciphersuite = undef;

sub new {
    my $class = shift;
    my ($filter,
        $execute,
        $cert,
        $debug) = @_;
    return init($class, $filter, $execute, $cert, $debug, 0);
}

sub new_dtls {
    my $class = shift;
    my ($filter,
        $execute,
        $cert,
        $debug) = @_;
    return init($class, $filter, $execute, $cert, $debug, 1);
}

sub init
{
    require IO::Socket::IP;
    my $class = shift;
    my ($filter,
        $execute,
        $cert,
        $debug,
        $isdtls) = @_;

    my $test_client_port;

    # Sometimes, our random selection of client ports gets unlucky
    # And we randomly select a port thats already in use.  This causes
    # this test to fail, so lets harden ourselves against that by doing
    # a test bind to the randomly selected port, and only continue once we
    # find a port thats available.
    my $test_client_addr = $have_IPv6 ? "[::1]" : "127.0.0.1";
    my $found_port = 0;
    for (my $i = 0; $i <= 10; $i++) {
        $test_client_port = 49152 + int(rand(65535 - 49152));
        my $test_sock;
        if ($useINET6 == 0) {
            $test_sock = IO::Socket::IP->new(LocalPort => $test_client_port,
                                             LocalAddr => $test_client_addr);
        } else {
            $test_sock = IO::Socket::INET6->new(LocalAddr => $test_client_addr,
                                                LocalPort => $test_client_port,
                                                Domain => AF_INET6);
        }
        if ($test_sock) {
            $found_port = 1;
            $test_sock->close();
            print "Found available client port ${test_client_port}\n";
            last;
        }
        print "Port ${test_client_port} in use - $@\n";
    }
  
    if ($found_port == 0) {
        die "Unable to find usable port for TLSProxy";
    }

    my $self = {
        #Public read/write
        proxy_addr => $test_client_addr,
        client_addr => $test_client_addr,
        filter => $filter,
        serverflags => "",
        clientflags => "",
        serverconnects => 1,
        reneg => 0,
        sessionfile => undef,

        #Public read
        isdtls => $isdtls,
        proxy_port => 0,
        client_port => $test_client_port,
        server_port => 0,
        serverpid => 0,
        clientpid => 0,
        execute => $execute,
        cert => $cert,
        debug => $debug,
        cipherc => "",
        ciphersuitesc => "",
        ciphers => "AES128-SHA",
        ciphersuitess => "TLS_AES_128_GCM_SHA256",
        flight => -1,
        direction => -1,
        partial => ["", ""],
        record_list => [],
        message_list => [],
    };

    return bless $self, $class;
}

sub DESTROY
{
    my $self = shift;

    $self->{proxy_sock}->close() if $self->{proxy_sock};
}

sub clearClient
{
    my $self = shift;

    $self->{cipherc} = "";
    $self->{ciphersuitec} = "";
    $self->{flight} = -1;
    $self->{direction} = -1;
    $self->{partial} = ["", ""];
    $self->{record_list} = [];
    $self->{message_list} = [];
    $self->{clientflags} = "";
    $self->{sessionfile} = undef;
    $self->{clientpid} = 0;
    $is_tls13 = 0;
    $ciphersuite = undef;

    TLSProxy::Message->clear();
    TLSProxy::Record->clear();
}

sub clear
{
    my $self = shift;

    $self->clearClient;
    $self->{ciphers} = "AES128-SHA";
    $self->{ciphersuitess} = "TLS_AES_128_GCM_SHA256";
    $self->{serverflags} = "";
    $self->{serverconnects} = 1;
    $self->{serverpid} = 0;
    $self->{reneg} = 0;
}

sub restart
{
    my $self = shift;

    $self->clear;
    $self->start;
}

sub clientrestart
{
    my $self = shift;

    $self->clear;
    $self->clientstart;
}

sub connect_to_server
{
    my $self = shift;
    my $servaddr = $self->{server_addr};

    $servaddr =~ s/[\[\]]//g; # Remove [ and ]

    my $sock = $IP_factory->(PeerAddr => $servaddr,
                             PeerPort => $self->{server_port},
                             Proto => $self->{isdtls} ? 'udp' : 'tcp');
    if (!defined($sock)) {
        my $err = $!;
        kill(3, $self->{real_serverpid});
        die "unable to connect: $err\n";
    }

    $self->{server_sock} = $sock;
}

sub start
{
    my ($self) = shift;
    my $pid;

    # Create the Proxy socket
    my $proxaddr = $self->{proxy_addr};
    $proxaddr =~ s/[\[\]]//g; # Remove [ and ]
    my $clientaddr = $self->{client_addr};
    $clientaddr =~ s/[\[\]]//g; # Remove [ and ]

    my @proxyargs;

    if ($self->{isdtls}) {
        @proxyargs = (
            LocalHost   => $proxaddr,
            LocalPort   => 0,
            PeerHost   => $clientaddr,
            PeerPort   => $self->{client_port},
            Proto       => "udp",
        );
    } else {
        @proxyargs = (
            LocalHost   => $proxaddr,
            LocalPort   => 0,
            Proto       => "tcp",
            Listen      => SOMAXCONN,
        );
    }

    if (my $sock = $IP_factory->(@proxyargs)) {
        $self->{proxy_sock} = $sock;
        $self->{proxy_port} = $sock->sockport();
        $self->{proxy_addr} = $sock->sockhost();
        $self->{proxy_addr} =~ s/(.*:.*)/[$1]/;
        print "Proxy started on port ",
            "$self->{proxy_addr}:$self->{proxy_port}\n";
        # use same address for s_server
        $self->{server_addr} = $self->{proxy_addr};
    } else {
        warn "Failed creating proxy socket (".$proxaddr.",0): $!\n";
    }

    if ($self->{proxy_sock} == 0) {
        return 0;
    }

    my $execcmd = $self->execute
        ." s_server -no_comp -engine ossltest -state"
        #In TLSv1.3 we issue two session tickets. The default session id
        #callback gets confused because the ossltest engine causes the same
        #session id to be created twice due to the changed random number
        #generation. Using "-ext_cache" replaces the default callback with a
        #different one that doesn't get confused.
        ." -ext_cache"
        ." -accept $self->{server_addr}:0"
        ." -cert ".$self->cert." -cert2 ".$self->cert
        ." -naccept ".$self->serverconnects;
    if ($self->{isdtls}) {
        $execcmd .= " -dtls -max_protocol DTLSv1.2"
                    # TLSProxy does not support message fragmentation. So
                    # set a high mtu and fingers crossed.
                    ." -mtu 1500";
    } else {
        $execcmd .= " -rev -max_protocol TLSv1.3";
    }
    if ($self->ciphers ne "") {
        $execcmd .= " -cipher ".$self->ciphers;
    }
    if ($self->ciphersuitess ne "") {
        $execcmd .= " -ciphersuites ".$self->ciphersuitess;
    }
    if ($self->serverflags ne "") {
        $execcmd .= " ".$self->serverflags;
    }
    if ($self->debug) {
        print STDERR "Server command: $execcmd\n";
    }

    open(my $savedin, "<&STDIN");

    # Temporarily replace STDIN so that sink process can inherit it...
    open(STDIN, "$^X -e 'sleep(10)' |") if $self->{isdtls};
    $pid = open(STDIN, "$execcmd 2>&1 |") or die "Failed to $execcmd: $!\n";
    $self->{real_serverpid} = $pid;

    # Process the output from s_server until we find the ACCEPT line, which
    # tells us what the accepting address and port are.
    while (<>) {
        print;
        s/\R$//;                # Better chomp
        next unless (/^ACCEPT\s.*:(\d+)$/);
        $self->{server_port} = $1;
        last;
    }

    if ($self->{server_port} == 0) {
        # This actually means that s_server exited, because otherwise
        # we would still searching for ACCEPT...
        waitpid($pid, 0);
        die "no ACCEPT detected in '$execcmd' output: $?\n";
    }

    # Just make sure everything else is simply printed [as separate lines].
    # The sub process simply inherits our STD* and will keep consuming
    # server's output and printing it as long as there is anything there,
    # out of our way.
    my $error;
    $pid = undef;
    if (eval { require Win32::Process; 1; }) {
        if (Win32::Process::Create(my $h, $^X, "perl -ne print", 0, 0, ".")) {
            $pid = $h->GetProcessID();
            $self->{proc_handle} = $h;  # hold handle till next round [or exit]
        } else {
            $error = Win32::FormatMessage(Win32::GetLastError());
        }
    } else {
        if (defined($pid = fork)) {
            $pid or exec("$^X -ne print") or exit($!);
        } else {
            $error = $!;
        }
    }

    # Change back to original stdin
    open(STDIN, "<&", $savedin);
    close($savedin);

    if (!defined($pid)) {
        kill(3, $self->{real_serverpid});
        die "Failed to capture s_server's output: $error\n";
    }

    $self->{serverpid} = $pid;

    print STDERR "Server responds on ",
                 "$self->{server_addr}:$self->{server_port}\n";

    # Connect right away...
    $self->connect_to_server();

    return $self->clientstart;
}

sub clientstart
{
    my ($self) = shift;

    my $success = 1;

    if ($self->execute) {
        my $pid;
        my $execcmd = $self->execute
             ." s_client -engine ossltest"
             ." -connect $self->{proxy_addr}:$self->{proxy_port}";
        if ($self->{isdtls}) {
            $execcmd .= " -dtls -max_protocol DTLSv1.2"
                        # TLSProxy does not support message fragmentation. So
                        # set a high mtu and fingers crossed.
                        ." -mtu 1500"
                        # UDP has no "accept" for sockets which means we need to
                        # know were to send data back to.
                        ." -bind $self->{client_addr}:$self->{client_port}";
        } else {
            $execcmd .= " -max_protocol TLSv1.3";
        }
        if ($self->cipherc ne "") {
            $execcmd .= " -cipher ".$self->cipherc;
        }
        if ($self->ciphersuitesc ne "") {
            $execcmd .= " -ciphersuites ".$self->ciphersuitesc;
        }
        if ($self->clientflags ne "") {
            $execcmd .= " ".$self->clientflags;
        }
        if ($self->clientflags !~ m/-(no)?servername/) {
            $execcmd .= " -servername localhost";
        }
        if (defined $self->sessionfile) {
            $execcmd .= " -ign_eof";
        }
        if ($self->debug) {
            print STDERR "Client command: $execcmd\n";
        }

        open(my $savedout, ">&STDOUT");
        # If we open pipe with new descriptor, attempt to close it,
        # explicitly or implicitly, would incur waitpid and effectively
        # dead-lock...
        if (!($pid = open(STDOUT, "| $execcmd"))) {
            my $err = $!;
            kill(3, $self->{real_serverpid});
            die "Failed to $execcmd: $err\n";
        }
        $self->{clientpid} = $pid;

        # queue [magic] input
        print $self->reneg ? "R" : "test";

        # this closes client's stdin without waiting for its pid
        open(STDOUT, ">&", $savedout);
        close($savedout);
    }

    # Wait for incoming connection from client
    my $fdset = IO::Select->new($self->{proxy_sock});
    if (!$fdset->can_read(60)) {
        kill(3, $self->{real_serverpid});
        die "s_client didn't try to connect\n";
    }

    my $client_sock;
    if($self->{isdtls}) {
        $client_sock = $self->{proxy_sock}
    } elsif (!($client_sock = $self->{proxy_sock}->accept())) {
        warn "Failed accepting incoming connection: $!\n";
        return 0;
    }

    print "Connection opened\n";

    my $server_sock = $self->{server_sock};
    my $indata;

    #Wait for either the server socket or the client socket to become readable
    $fdset = IO::Select->new($server_sock, $client_sock);
    my @ready;
    my $ctr = 0;
    local $SIG{PIPE} = "IGNORE";
    $self->{saw_session_ticket} = undef;
    while($fdset->count && $ctr < 10) {
        if (defined($self->{sessionfile})) {
            # s_client got -ign_eof and won't be exiting voluntarily, so we
            # look for data *and* session ticket...
            last if TLSProxy::Message->success()
                    && $self->{saw_session_ticket};
        }
        if (!(@ready = $fdset->can_read(1))) {
            last if TLSProxy::Message->success()
                && $self->{saw_session_ticket};

            $ctr++;
            next;
        }
        foreach my $hand (@ready) {
            if ($hand == $server_sock) {
                if ($server_sock->sysread($indata, 16384)) {
                    if ($indata = $self->process_packet(1, $indata)) {
                        $client_sock->syswrite($indata) or goto END;
                    }
                    $ctr = 0;
                } else {
                    $fdset->remove($server_sock);
                    $client_sock->shutdown(SHUT_WR);
                }
            } elsif ($hand == $client_sock) {
                if ($client_sock->sysread($indata, 16384)) {
                    if ($indata = $self->process_packet(0, $indata)) {
                        $server_sock->syswrite($indata) or goto END;
                    }
                    $ctr = 0;
                } else {
                    $fdset->remove($client_sock);
                    $server_sock->shutdown(SHUT_WR);
                }
            } else {
                kill(3, $self->{real_serverpid});
                die "Unexpected handle";
            }
        }
    }

    if ($ctr >= 10) {
        kill(3, $self->{real_serverpid});
        print "No progress made\n";
        $success = 0;
    }

    END:
    print "Connection closed\n";
    if($server_sock) {
        $server_sock->close();
        $self->{server_sock} = undef;
    }
    if($client_sock) {
        #Closing this also kills the child process
        $client_sock->close();
    }

    my $pid;
    if (--$self->{serverconnects} == 0) {
        $pid = $self->{serverpid};
        print "Waiting for 'perl -ne print' process to close: $pid...\n";
        $pid = waitpid($pid, 0);
        if ($pid > 0) {
            die "exit code $? from 'perl -ne print' process\n" if $? != 0;
        } elsif ($pid == 0) {
            kill(3, $self->{real_serverpid});
            die "lost control over $self->{serverpid}?";
        }
        $pid = $self->{real_serverpid};
        print "Waiting for s_server process to close: $pid...\n";
        # it's done already, just collect the exit code [and reap]...
        waitpid($pid, 0);
        die "exit code $? from s_server process\n" if $? != 0;
    } else {
        # It's a bit counter-intuitive spot to make next connection to
        # the s_server. Rationale is that established connection works
        # as synchronization point, in sense that this way we know that
        # s_server is actually done with current session...
        $self->connect_to_server();
    }
    $pid = $self->{clientpid};
    print "Waiting for s_client process to close: $pid...\n";
    waitpid($pid, 0);

    return $success;
}

sub process_packet
{
    my ($self, $server, $packet) = @_;
    my $len_real;
    my $decrypt_len;
    my $data;
    my $recnum;

    if ($server) {
        print "Received server packet\n";
    } else {
        print "Received client packet\n";
    }

    if ($self->{direction} != $server) {
        $self->{flight} = $self->{flight} + 1;
        $self->{direction} = $server;
    }

    print "Packet length = ".length($packet)."\n";
    print "Processing flight ".$self->flight."\n";

    #Return contains the list of record found in the packet followed by the
    #list of messages in those records and any partial message
    my @ret = TLSProxy::Record->get_records($server, $self->flight,
                                            $self->{partial}[$server].$packet,
                                            $self->{isdtls});

    $self->{partial}[$server] = $ret[2];
    push @{$self->{record_list}}, @{$ret[0]};
    push @{$self->{message_list}}, @{$ret[1]};

    print "\n";

    if (scalar(@{$ret[0]}) == 0 or length($ret[2]) != 0) {
        return "";
    }

    #Finished parsing. Call user provided filter here
    if (defined $self->filter) {
        $self->filter->($self);
    }

    #Take a note on NewSessionTicket
    foreach my $message (reverse @{$self->{message_list}}) {
        if ($message->{mt} == TLSProxy::Message::MT_NEW_SESSION_TICKET) {
            $self->{saw_session_ticket} = 1;
            last;
        }
    }

    #Reconstruct the packet
    $packet = "";
    foreach my $record (@{$self->record_list}) {
        $packet .= $record->reconstruct_record($server);
    }

    print "Forwarded packet length = ".length($packet)."\n\n";

    return $packet;
}

#Read accessors
sub execute
{
    my $self = shift;
    return $self->{execute};
}
sub cert
{
    my $self = shift;
    return $self->{cert};
}
sub debug
{
    my $self = shift;
    return $self->{debug};
}
sub flight
{
    my $self = shift;
    return $self->{flight};
}
sub record_list
{
    my $self = shift;
    return $self->{record_list};
}
sub success
{
    my $self = shift;
    return $self->{success};
}
sub end
{
    my $self = shift;
    return $self->{end};
}
sub supports_IPv6
{
    my $self = shift;
    return $have_IPv6;
}
sub proxy_addr
{
    my $self = shift;
    return $self->{proxy_addr};
}
sub proxy_port
{
    my $self = shift;
    return $self->{proxy_port};
}
sub server_addr
{
    my $self = shift;
    return $self->{server_addr};
}
sub server_port
{
    my $self = shift;
    return $self->{server_port};
}
sub serverpid
{
    my $self = shift;
    return $self->{serverpid};
}
sub clientpid
{
    my $self = shift;
    return $self->{clientpid};
}

#Read/write accessors
sub filter
{
    my $self = shift;
    if (@_) {
        $self->{filter} = shift;
    }
    return $self->{filter};
}
sub cipherc
{
    my $self = shift;
    if (@_) {
        $self->{cipherc} = shift;
    }
    return $self->{cipherc};
}
sub ciphersuitesc
{
    my $self = shift;
    if (@_) {
        $self->{ciphersuitesc} = shift;
    }
    return $self->{ciphersuitesc};
}
sub ciphers
{
    my $self = shift;
    if (@_) {
        $self->{ciphers} = shift;
    }
    return $self->{ciphers};
}
sub ciphersuitess
{
    my $self = shift;
    if (@_) {
        $self->{ciphersuitess} = shift;
    }
    return $self->{ciphersuitess};
}
sub serverflags
{
    my $self = shift;
    if (@_) {
        $self->{serverflags} = shift;
    }
    return $self->{serverflags};
}
sub clientflags
{
    my $self = shift;
    if (@_) {
        $self->{clientflags} = shift;
    }
    return $self->{clientflags};
}
sub serverconnects
{
    my $self = shift;
    if (@_) {
        $self->{serverconnects} = shift;
    }
    return $self->{serverconnects};
}
# This is a bit ugly because the caller is responsible for keeping the records
# in sync with the updated message list; simply updating the message list isn't
# sufficient to get the proxy to forward the new message.
# But it does the trick for the one test (test_sslsessiontick) that needs it.
sub message_list
{
    my $self = shift;
    if (@_) {
        $self->{message_list} = shift;
    }
    return $self->{message_list};
}

sub fill_known_data
{
    my $length = shift;
    my $ret = "";
    for (my $i = 0; $i < $length; $i++) {
        $ret .= chr($i);
    }
    return $ret;
}

sub is_tls13
{
    my $class = shift;
    if (@_) {
        $is_tls13 = shift;
    }
    return $is_tls13;
}

sub reneg
{
    my $self = shift;
    if (@_) {
        $self->{reneg} = shift;
    }
    return $self->{reneg};
}

#Setting a sessionfile means that the client will not close until the given
#file exists. This is useful in TLSv1.3 where otherwise s_client will close
#immediately at the end of the handshake, but before the session has been
#received from the server. A side effect of this is that s_client never sends
#a close_notify, so instead we consider success to be when it sends application
#data over the connection.
sub sessionfile
{
    my $self = shift;
    if (@_) {
        $self->{sessionfile} = shift;
        TLSProxy::Message->successondata(1);
    }
    return $self->{sessionfile};
}

sub ciphersuite
{
    my $class = shift;
    if (@_) {
        $ciphersuite = shift;
    }
    return $ciphersuite;
}

sub isdtls
{
    my $self = shift;
    return $self->{isdtls}; #read-only
}

1;
