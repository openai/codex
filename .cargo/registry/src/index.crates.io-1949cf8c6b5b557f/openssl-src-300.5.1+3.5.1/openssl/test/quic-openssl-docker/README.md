quic-openssl-docker
===================

Dockerfile for quic working group interop testing

Overview
--------

This Dockerfile builds a container for use with the
[QUIC working group interop testing facility](https://interop.seemann.io/)
It can also be used locally to test QUIC interoperability via the
[QUIC interop runner](https://github.com/quic-interop/quic-interop-runner)
Please see instructions there for running local interop testing

Building the container
----------------------

From this directory:
`docker build -t quay.io/openssl-ci/openssl-quic-interop:latest .`

Note the tag name is important, as the interop runner knows the container
by this name.  If you build locally with changes, the interop runner project
will pick up the container from your local registry rather than downloading it
