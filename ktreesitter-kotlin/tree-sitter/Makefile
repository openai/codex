ifeq ($(OS),Windows_NT)
$(error Windows is not supported)
endif

VERSION := 0.25.8
DESCRIPTION := An incremental parsing system for programming tools
HOMEPAGE_URL := https://tree-sitter.github.io/tree-sitter/

# install directory layout
PREFIX ?= /usr/local
INCLUDEDIR ?= $(PREFIX)/include
LIBDIR ?= $(PREFIX)/lib
PCLIBDIR ?= $(LIBDIR)/pkgconfig

# collect sources
ifneq ($(AMALGAMATED),1)
	SRC := $(wildcard lib/src/*.c)
	# do not double-include amalgamation
	SRC := $(filter-out lib/src/lib.c,$(SRC))
else
	# use amalgamated build
	SRC := lib/src/lib.c
endif
OBJ := $(SRC:.c=.o)

# define default flags, and override to append mandatory flags
ARFLAGS := rcs
CFLAGS ?= -O3 -Wall -Wextra -Wshadow -Wpedantic -Werror=incompatible-pointer-types
override CFLAGS += -std=c11 -fPIC -fvisibility=hidden
override CFLAGS += -D_POSIX_C_SOURCE=200112L -D_DEFAULT_SOURCE
override CFLAGS += -Ilib/src -Ilib/src/wasm -Ilib/include

# ABI versioning
SONAME_MAJOR := $(word 1,$(subst ., ,$(VERSION)))
SONAME_MINOR := $(word 2,$(subst ., ,$(VERSION)))

# OS-specific bits
ifneq ($(findstring darwin,$(shell $(CC) -dumpmachine)),)
	SOEXT = dylib
	SOEXTVER_MAJOR = $(SONAME_MAJOR).$(SOEXT)
	SOEXTVER = $(SONAME_MAJOR).$(SONAME_MINOR).$(SOEXT)
	LINKSHARED += -dynamiclib -Wl,-install_name,$(LIBDIR)/libtree-sitter.$(SOEXTVER)
else
	SOEXT = so
	SOEXTVER_MAJOR = $(SOEXT).$(SONAME_MAJOR)
	SOEXTVER = $(SOEXT).$(SONAME_MAJOR).$(SONAME_MINOR)
	LINKSHARED += -shared -Wl,-soname,libtree-sitter.$(SOEXTVER)
endif
ifneq ($(filter $(shell uname),FreeBSD NetBSD DragonFly),)
	PCLIBDIR := $(PREFIX)/libdata/pkgconfig
endif

all: libtree-sitter.a libtree-sitter.$(SOEXT) tree-sitter.pc

libtree-sitter.a: $(OBJ)
	$(AR) $(ARFLAGS) $@ $^

libtree-sitter.$(SOEXT): $(OBJ)
	$(CC) $(LDFLAGS) $(LINKSHARED) $^ $(LDLIBS) -o $@
ifneq ($(STRIP),)
	$(STRIP) $@
endif

tree-sitter.pc: lib/tree-sitter.pc.in
	sed -e 's|@PROJECT_VERSION@|$(VERSION)|' \
		-e 's|@CMAKE_INSTALL_LIBDIR@|$(LIBDIR:$(PREFIX)/%=%)|' \
		-e 's|@CMAKE_INSTALL_INCLUDEDIR@|$(INCLUDEDIR:$(PREFIX)/%=%)|' \
		-e 's|@PROJECT_DESCRIPTION@|$(DESCRIPTION)|' \
		-e 's|@PROJECT_HOMEPAGE_URL@|$(HOMEPAGE_URL)|' \
		-e 's|@CMAKE_INSTALL_PREFIX@|$(PREFIX)|' $< > $@

clean:
	$(RM) $(OBJ) tree-sitter.pc libtree-sitter.a libtree-sitter.$(SOEXT)

install: all
	install -d '$(DESTDIR)$(INCLUDEDIR)'/tree_sitter '$(DESTDIR)$(PCLIBDIR)' '$(DESTDIR)$(LIBDIR)'
	install -m644 lib/include/tree_sitter/api.h '$(DESTDIR)$(INCLUDEDIR)'/tree_sitter/api.h
	install -m644 tree-sitter.pc '$(DESTDIR)$(PCLIBDIR)'/tree-sitter.pc
	install -m644 libtree-sitter.a '$(DESTDIR)$(LIBDIR)'/libtree-sitter.a
	install -m755 libtree-sitter.$(SOEXT) '$(DESTDIR)$(LIBDIR)'/libtree-sitter.$(SOEXTVER)
	ln -sf libtree-sitter.$(SOEXTVER) '$(DESTDIR)$(LIBDIR)'/libtree-sitter.$(SOEXTVER_MAJOR)
	ln -sf libtree-sitter.$(SOEXTVER_MAJOR) '$(DESTDIR)$(LIBDIR)'/libtree-sitter.$(SOEXT)

uninstall:
	$(RM) '$(DESTDIR)$(LIBDIR)'/libtree-sitter.a \
		'$(DESTDIR)$(LIBDIR)'/libtree-sitter.$(SOEXTVER) \
		'$(DESTDIR)$(LIBDIR)'/libtree-sitter.$(SOEXTVER_MAJOR) \
		'$(DESTDIR)$(LIBDIR)'/libtree-sitter.$(SOEXT) \
		'$(DESTDIR)$(INCLUDEDIR)'/tree_sitter/api.h \
		'$(DESTDIR)$(PCLIBDIR)'/tree-sitter.pc

.PHONY: all install uninstall clean


##### Dev targets #####

test:
	cargo xtask fetch-fixtures
	cargo xtask generate-fixtures
	cargo xtask test

test-wasm:
	cargo xtask generate-fixtures --wasm
	cargo xtask test-wasm

lint:
	cargo update --workspace --locked --quiet
	cargo check --workspace --all-targets
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings

lint-web:
	npm --prefix lib/binding_web ci
	npm --prefix lib/binding_web run lint

format:
	cargo fmt --all

changelog:
	@git-cliff --config .github/cliff.toml --prepend CHANGELOG.md --latest --github-token $(shell gh auth token)

.PHONY: test test-wasm lint format changelog
