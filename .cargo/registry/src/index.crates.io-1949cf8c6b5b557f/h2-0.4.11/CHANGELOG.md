# 0.4.11 (June 30, 2025)

* Fix client to not return an error when a clean shutdown otherwise doesn't get a TLS close_notify, which some servers don't bother sending.

# 0.4.10 (May 5, 2025)

* Fix `is_end_stream()` to return true only when ended cleanly, not when errored.

# 0.4.9 (April 14, 2025)

* Add `sever::Connection::has_streams()` method to check for active streams.

# 0.4.8 (February 18, 2025)

* Fix handling implicit stream resets at the more correct time.
* Fix window size decrements of send-closed streams.
* Fix reclaiming of reserved capacity when streams are closed.
* Fix to no longer call `poll_flush` after `poll_shutdown`.
* Fix busy loop in task when poll_shutdown returns pending.

# 0.4.7 (November 19, 2024)

* Fix treating HEADERS frames with a non-zero content-length but END_STREAM flag as malformed.
* Fix notifying the stream task when automatically reset on receipt of a stream error.

# 0.4.6 (August 19, 2024)

* Add `current_max_send_streams()` and `current_max_recv_streams()` to `client::SendRequest`.
* Fix sending a PROTOCOL_ERROR instead of REFUSED_STREAM when receiving oversized headers.
* Fix notifying a PushPromise task properly.
* Fix notifying a stream task when reset.

# 0.4.5 (May 17, 2024)

* Fix race condition that sometimes hung connections during shutdown.
* Fix pseudo header construction for CONNECT and OPTIONS requests.

# 0.4.4 (April 3, 2024)

* Limit number of CONTINUATION frames for misbehaving connections.

# 0.4.3 (March 15, 2024)

* Fix flow control limits to not apply until receiving SETTINGS ack.
* Fix not returning an error if IO ended without `close_notify`.
* Improve performance of decoding many headers.

# 0.4.2 (January 17th, 2024)

* Limit error resets for misbehaving connections.
* Fix selecting MAX_CONCURRENT_STREAMS value if no value is advertised initially.

# 0.4.1 (January 8, 2024)

* Fix assigning connection capacity which could starve streams in some instances.

# 0.4.0 (November 15, 2023)

* Update to `http` 1.0.
* Remove deprecated `Server::poll_close()`.

# 0.3.22 (November 15, 2023)

* Add `header_table_size(usize)` option to client and server builders.
* Improve throughput when vectored IO is not available.
* Update indexmap to 2.

# 0.3.21 (August 21, 2023)

* Fix opening of new streams over peer's max concurrent limit.
* Fix `RecvStream` to return data even if it has received a `CANCEL` stream error.
* Update MSRV to 1.63.

# 0.3.20 (June 26, 2023)

* Fix panic if a server received a request with a `:status` pseudo header in the 1xx range.
* Fix panic if a reset stream had pending push promises that were more than allowed.
* Fix potential flow control overflow by subtraction, instead returning a connection error.

# 0.3.19 (May 12, 2023)

* Fix counting reset streams when triggered by a GOAWAY.
* Send `too_many_resets` in opaque debug data of GOAWAY when too many resets received.

# 0.3.18 (April 17, 2023)

* Fix panic because of opposite check in `is_remote_local()`.

# 0.3.17 (April 13, 2023)

* Add `Error::is_library()` method to check if the originated inside `h2`.
* Add `max_pending_accept_reset_streams(usize)` option to client and server
  builders.
* Fix theoretical memory growth when receiving too many HEADERS and then
  RST_STREAM frames faster than an application can accept them off the queue.
  (CVE-2023-26964)

# 0.3.16 (February 27, 2023)

* Set `Protocol` extension on requests when received Extended CONNECT requests.
* Remove `B: Unpin + 'static` bound requiremented of bufs
* Fix releasing of frames when stream is finished, reducing memory usage.
* Fix panic when trying to send data and connection window is available, but stream window is not.
* Fix spurious wakeups when stream capacity is not available.

# 0.3.15 (October 21, 2022)

* Remove `B: Buf` bound on `SendStream`'s parameter
* add accessor for `StreamId` u32

# 0.3.14 (August 16, 2022)

* Add `Error::is_reset` function.
* Bump MSRV to Rust 1.56.
* Return `RST_STREAM(NO_ERROR)` when the server early responds.

# 0.3.13 (March 31, 2022)

* Update private internal `tokio-util` dependency.

# 0.3.12 (March 9, 2022)

* Avoid time operations that can panic (#599)
* Bump MSRV to Rust 1.49 (#606)
* Fix header decoding error when a header name is contained at a continuation
  header boundary (#589)
* Remove I/O type names from handshake `tracing` spans (#608)

# 0.3.11 (January 26, 2022)

* Make `SendStream::poll_capacity` never return `Ok(Some(0))` (#596)
* Fix panic when receiving already reset push promise (#597)

# 0.3.10 (January 6, 2022)

* Add `Error::is_go_away()` and `Error::is_remote()` methods.
* Fix panic if receiving malformed PUSH_PROMISE with stream ID of 0.

# 0.3.9 (December 9, 2021)

* Fix hang related to new `max_send_buffer_size`.

# 0.3.8 (December 8, 2021)

* Add "extended CONNECT support". Adds `h2::ext::Protocol`, which is used for request and response extensions to connect new protocols over an HTTP/2 stream.
* Add `max_send_buffer_size` options to client and server builders, and a default of ~400MB. This acts like a high-water mark for the `poll_capacity()` method.
* Fix panic if receiving malformed HEADERS with stream ID of 0.

# 0.3.7 (October 22, 2021)

* Fix panic if server sends a malformed frame on a stream client was about to open.
* Fix server to treat `:status` in a request as a stream error instead of connection error.

# 0.3.6 (September 30, 2021)

* Fix regression of `h2::Error` that were created via `From<h2::Reason>` not returning their reason code in `Error::reason()`.

# 0.3.5 (September 29, 2021)

* Fix sending of very large headers. Previously when a single header was too big to fit in a single `HEADERS` frame, an error was returned. Now it is broken up and sent correctly.
* Fix buffered data field to be a bigger integer size.
* Refactor error format to include what initiated the error (remote, local, or user), if it was a stream or connection-level error, and any received debug data.

# 0.3.4 (August 20, 2021)

* Fix panic when encoding header size update over a certain size.
* Fix `SendRequest` to wake up connection when dropped.
* Fix potential hang if `RecvStream` is placed in the request or response `extensions`.
* Stop calling `Instant::now` if zero reset streams are configured.

# 0.3.3 (April 29, 2021)

* Fix client being able to make `CONNECT` requests without a `:path`.
* Expose `RecvStream::poll_data`.
* Fix some docs.

# 0.3.2 (March 24, 2021)

* Fix incorrect handling of received 1xx responses on the client when the request body is still streaming.

# 0.3.1 (February 26, 2021)

* Add `Connection::max_concurrent_recv_streams()` getter.
* Add `Connection::max_concurrent_send_streams()` getter.
* Fix client to ignore receipt of 1xx headers frames.
* Fix incorrect calculation of pseudo header lengths when determining if a received header is too big.
* Reduce monomorphized code size of internal code.

# 0.3.0 (December 23, 2020)

* Update to Tokio v1 and Bytes v1.
* Disable `tracing`'s `log` feature. (It can still be enabled by a user in their own `Cargo.toml`.)

# 0.2.7 (October 22, 2020)

* Fix stream ref count when sending a push promise
* Fix receiving empty DATA frames in response to a HEAD request
* Fix handling of client disabling SERVER_PUSH

# 0.2.6 (July 13, 2020)

* Integrate `tracing` directly where `log` was used. (For 0.2.x, `log`s are still emitted by default.)

# 0.2.5 (May 6, 2020)

* Fix rare debug assert failure in store shutdown.

# 0.2.4 (March 30, 2020)

* Fix when receiving `SETTINGS_HEADER_TABLE_SIZE` setting.

# 0.2.3 (March 25, 2020)

* Fix server being able to accept `CONNECT` requests without `:scheme` or `:path`.
* Fix receiving a GOAWAY frame from updating the recv max ID, it should only update max send ID.

# 0.2.2 (March 3, 2020)

* Reduce size of `FlowControl` and `RecvStream`.

# 0.2.1 (December 6, 2019)

* Relax `Unpin` bounds on the send `Buf` generic.

# 0.2.0 (December 3, 2019)

* Add `server::Connection::set_initial_window_size` and `client::Connection::set_initial_window_size` which can adjust the `INITIAL_WINDOW_SIZE` setting on an existing connection (#421).
* Update to `http` v0.2.
* Update to `tokio` v0.2.
* Change `unstable-stream` feature to `stream`.
* Change `ReserveCapacity` to `FlowControl` (#423).
* Remove `From<io::Error>` for `Error`.

# 0.2.0-alpha.3 (October 1, 2019)

* Update to futures `0.3.0-alpha.19`.
* Update to tokio `0.2.0-alpha.6`.

# 0.2.0-alpha.2 (September 20, 2019)

* Add server support for `PUSH_PROMISE`s (#327).
* Update to tokio `0.2.0-alpha.5`.
* Change `stream` feature to `unstable-stream`.

# 0.2.0-alpha.1 (August 30, 2019)

* Update from `futures` 0.1 to `std::future::Future`.
* Update `AsyncRead`/`AsyncWrite` to `tokio-io` 0.2 alpha.
* Change `Stream` implementations to be optional, default disabled. Specific async and poll functions are now inherent, and `Stream` can be re-enabled with the `stream` cargo feature.

# 0.1.25 (June 28, 2019)

* Fix to send a `RST_STREAM` instead of `GOAWAY` if receiving a frame on a previously closed stream.
* Fix receiving trailers without an end-stream flag to be a stream error instead of connection error.

# 0.1.24 (June 17, 2019)

* Fix server wrongly rejecting requests that don't have an `:authority` header (#372).

# 0.1.23 (June 4, 2019)

* Fix leaking of received DATA frames if the `RecvStream` is never polled (#368).

# 0.1.22 (June 3, 2019)

* Fix rare panic when remote sends `RST_STREAM` or `GOAWAY` for a stream pending window capacity (#364).

# 0.1.21 (May 30, 2019)

* Fix write loop when a header didn't fit in write buffer.

# 0.1.20 (May 16, 2019)

* Fix lifetime conflict for older compilers.

# 0.1.19 (May 15, 2019)

* Fix rare crash if `CONTINUATION` frame resumed in the middle of headers with the same name.
* Fix HPACK encoder using an old evicted index for repeated header names.

# 0.1.18 (April 9, 2019)

* Fix `server::Connection::abrupt_shutdown` to no longer return the same error the user sent (#352).

# 0.1.17 (March 12, 2019)

* Add user PING support (#346).
* Fix notifying a `RecvStream` task if locally sending a reset.
* Fix connections "hanging" when all handles are dropped but some streams had been reset.

# 0.1.16 (January 24, 2019)

* Log header values when malformed (#342).

# 0.1.15 (January 12, 2019)

* Fix race condition bug related to shutting down the client (#338).

# 0.1.14 (December 5, 2018)

* Fix closed streams to always return window capacity to the connection (#334).
* Fix locking when `Debug` printing an `OpaqueStreamRef` (#333).
* Fix inverted split for DATA frame padding (#330).
* Reduce `Debug` noise for `Frame` (#329).

# 0.1.13 (October 16, 2018)

* Add client support for Push Promises (#314).
* Expose `io::Error` from `h2::Error` (#311)
* Misc bug fixes (#304, #309, #319, #313, #320).

# 0.1.12 (August 8, 2018)

* Fix initial send window size (#301).
* Fix panic when calling `reserve_capacity` after connection has been closed (#302).
* Fix handling of incoming `SETTINGS_INITIAL_WINDOW_SIZE`. (#299)

# 0.1.11 (July 31, 2018)

* Add `stream_id` accessors to public API types (#292).
* Fix potential panic when dropping clients (#295).
* Fix busy loop when shutting down server (#296).

# 0.1.10 (June 15, 2018)

* Fix potential panic in `SendRequest::poll_ready()` (#281).
* Fix infinite loop on reset connection during prefix (#285).

# 0.1.9 (May 31, 2018)

* Add `poll_reset` to `SendResponse` and `SendStream` (#279).

# 0.1.8 (May 23, 2018)

* Fix client bug when max streams is reached. (#277)

# 0.1.7 (May 14, 2018)

* Misc bug fixes (#266, #273, #261, #275).

# 0.1.6 (April 24, 2018)

* Misc bug fixes related to stream management (#258, #260, #262).

# 0.1.5 (April 6, 2018)

* Fix the `last_stream_id` sent during graceful GOAWAY (#254).

# 0.1.4 (April 5, 2018)

* Add `initial_connection_window_size` to client and server `Builder`s (#249).
* Add `graceful_shutdown` and `abrupt_shutdown` to `server::Connection`,
  deprecating `close_connection` (#250).

# 0.1.3 (March 28, 2018)

* Allow configuring max streams before the peer's settings frame is
  received (#242).
* Fix HPACK decoding bug with regards to large literals (#244).
* Fix state transition bug triggered by receiving a RST_STREAM frame (#247).

# 0.1.2 (March 13, 2018)

* Fix another bug relating to resetting connections and reaching
  max concurrency (#238).

# 0.1.1 (March 8, 2018)

* When streams are dropped, close the connection (#222).
* Notify send tasks on connection error (#231).
* Fix bug relating to resetting connections and reaching max concurrency (#235).
* Normalize HTTP request path to satisfy HTTP/2.0 specification (#228).
* Update internal dependencies.

# 0.1.0 (Jan 12, 2018)

* Initial release
