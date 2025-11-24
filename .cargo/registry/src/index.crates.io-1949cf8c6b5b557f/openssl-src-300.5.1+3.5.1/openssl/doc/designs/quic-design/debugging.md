QUIC: Debugging and Tracing
===========================

When debugging the QUIC stack it is extremely useful to have protocol traces
available. There are two approaches you can use to obtain this data:

- qlog
- Packet capture

Neither of these approaches is strictly superior to the other and both have pros
and cons:

- In general, qlog is aimed at storing only information relevant to the
  QUIC protocol itself without storing bulk data. This includes both transmitted
  and received packets but also information about the internal state of a QUIC
  implementation which is not directly observable from the network.

- By comparison, packet capture stores all packets in their entirety.
  Packet captures are thus larger, but they also provide more complete
  information in general and do not have information removed. On the other hand,
  because they work from a network viewpoint, they cannot provide direct
  information on the internal state of a QUIC implementation. For example,
  packet capture cannot directly tell you when an implementation deems a packet
  lost.

Both of these approaches have good GUI visualisation tools available for viewing
the logged data.

To summarise:

- qlog:
  - Pro: Smaller files
  - Con: May leave out data assumed to be irrelevant
  - Pro: Information on internal states and decisions made by a QUIC
    implementation
  - Pro: No need to obtain a keylog
- PCAP:
  - Pro: Complete capture
  - Con: No direct information on internal states of a QUIC implementation
  - Con: Need to obtain a keylog

Using qlog
----------

To enable qlog you must:

- build using the `enable-unstable-qlog` build-time configuration option;

- set the environment variable `QLOGDIR` to a directory where qlog log files
  are to be written;

- set the environment variable `OSSL_QFILTER` to a filter specifying the events
  you want to be written (set `OSSL_QFILTER='*'` for all events).

Any process using the libssl QUIC implementation will then automatically write
qlog files in the JSON-SEQ format to the specified directory. The files have the
naming convention recommended by the specification: `{ODCID}_{ROLE}.sqlog`,
where `{ODCID}` is the initial (original) DCID of a connection and `{ROLE}` is
`client` or `server`.

The log files can be loaded into [qvis](https://qvis.quictools.info/). The [qvis
website](https://qvis.quictools.info/) also has some sample qlog files which you
can load at the click of a button, which enables you to see what kind of
information qvis can offer you.

Note that since the qlog specification is not finalised and still evolving,
the format of the output may change, as may the method of configuring this
logging support.

Currently this implementation tracks qvis's qlog support, as that is the
main target use case at this time.

Note that since qlog emphasises logging only data which is relevant to a QUIC
protocol implementation, for the purposes of reducing the volume of logging
data, application data is generally not logged. (However, this is not a
guarantee and must not be relied upon from a privacy perspective.)

[See here for more details on the design of the qlog facility.](qlog.md)

Using PCAP
----------

To use PCAP you can use any standard packet capture tool, such as Wireshark or
tcpdump (e.g. `tcpdump -U -i "$IFACE" -w "$FILE" 'udp port 1234'`).

**Using Wireshark.** Once you have obtained a packet capture as a standard
`pcap` or `pcapng` file, you can load it into Wireshark, which has excellent
QUIC protocol decoding support.

**Activating the decoder.** If you are using QUIC on a port not known to be
commonly used for QUIC, you may need to tell Wireshark to try and decode a flow
as QUIC. To do this, right click on the Protocol column and select “Decode
As...”. Click on “(none)” under the Current column and select QUIC.

**Keylogs.** Since QUIC is an encrypted protocol, Wireshark cannot provide much
information without access to the encryption keys used for the connection
(though it is able to decrypt Initial packets).

In order to provide this information you need to provide Wireshark with a keylog
file. This is a log file containing encryption keys for the connection which is
written directly by a QUIC implementation for debugging purposes. The purpose of
such a file is to enable a TLS or QUIC session to be decrypted for development
purposes in a lab environment. It should go without saying that the export of a
keylog file should never be used in a production environment.

For the OpenSSL QUIC implementation, OpenSSL must be instructed to save a keylog
file using the SSL_CTX_set_keylog_callback(3) API call. If the application you
are using does not provide a way to enable this functionality, this requires
recompiling the application you are using as OpenSSL does not provide a way
to enable this functionality directly.

If you are using OpenSSL QUIC to talk to another QUIC implementation, you also
may be able to obtain a keylog from that other implementation. (It does not
matter from which side of the connection you obtain the keylog.)

Once you have a keylog file you can configure Wireshark to use it.
There are two ways to do this:

- **Manual configuration.** Select Edit →
  Preferences and navigate to Protocols → TLS. Enter the path to the keylog file
  under “(Pre)-Master-Secret log filename". You can have key information being
  appended to this log continuously if desired. Press OK and Wireshark should
  now be able to decrypt any TLS or QUIC session described by the log file.

- **Embedding.** Alternatively, you can embed a keylog file into a `.pcapng`
  file directly, so that Wireshark can decrypt the packets automatically when
  the packet capture file is opened. This avoids the need to have a centralised
  key log file and ensures that the key log for a specific packet capture is
  kept together with the captured packets. It is also highly useful if you want
  to distribute a packet capture file publicly, for example for educational
  purposes.

  To embed a keylog, you can use the `editcap` command provided by Wireshark
  after taking a packet capture (note that `tls` should be specified below
  regardless of whether TLS or QUIC is being used):

  ```bash
  $ editcap --inject-secrets tls,$PATH_TO_KEYLOG_FILE \
    "$INPUT_FILENAME" "$OUTPUT_FILENAME"
  ```

  This tool accepts `.pcap` or `.pcapng` input and will generate a `.pcapng`
  output file.
