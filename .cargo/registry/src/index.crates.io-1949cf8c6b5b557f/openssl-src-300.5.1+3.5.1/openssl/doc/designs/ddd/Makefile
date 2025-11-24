#
# To run the demos when linked with a shared library (default) ensure that
# libcrypto and libssl are on the library path. For example to run the
# ddd-01-conn-blocking-tls demo:
#
#    LD_LIBRARY_PATH=../../.. ./ddd-01-conn-blocking-tls
#
# Building ddd-06-mem-uv-tls and ddd-06-mem-uv-quic requires the
# library libuv and header file.  On Ubuntu, they are provided by the
# package "libuv1-dev".

TESTS_BASE = ddd-01-conn-blocking \
             ddd-02-conn-nonblocking \
             ddd-02-conn-nonblocking-threads \
             ddd-03-fd-blocking \
             ddd-04-fd-nonblocking \
             ddd-05-mem-nonblocking \
             ddd-06-mem-uv

TESTS = $(foreach x,$(TESTS_BASE),$(x)-tls $(x)-quic)

CFLAGS  = -I../../../include -g -Wall -Wsign-compare
LDFLAGS = -L../../..
LDLIBS  = -lcrypto -lssl

CC_CMD = $(CC) $(CFLAGS) $(LDFLAGS) -o "$@" "$<" $(LDLIBS)

all: $(TESTS)

clean:
	rm -f $(TESTS) *.o

ddd-%-tls: ddd-%.c
	$(CC_CMD)

ddd-%-quic: ddd-%.c
	$(CC_CMD) -DUSE_QUIC

ddd-%-uv-tls: ddd-%-uv.c
	$(CC_CMD) -luv

ddd-%-uv-quic: ddd-%-uv.c
	$(CC_CMD) -luv -DUSE_QUIC
