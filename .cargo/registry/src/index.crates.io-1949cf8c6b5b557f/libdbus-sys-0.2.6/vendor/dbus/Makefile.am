SUBDIRS=dbus bus tools test doc

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = dbus-1.pc

cmakeconfigdir = $(libdir)/cmake/DBus1
cmakeconfig_DATA = cmake/DBus1Config.cmake       \
                   cmake/DBus1ConfigVersion.cmake

EXTRA_DIST =			\
	autogen.sh		\
	CONTRIBUTING.md		\
	dbus-1.pc.in		\
	cleanup-man-pages.sh    \
        NEWS.pre-1-0            \
        NEWS.pre-1-2		\
	README.cmake		\
	README.valgrind		\
	README.win		\
	README.wince		\
	README.cygwin		\
	README.launchd		\
	CMakeLists.txt \
	bus/CMakeLists.txt \
	dbus/CMakeLists.txt \
	doc/CMakeLists.txt \
	test/CMakeLists.txt \
	test/name-test/CMakeLists.txt \
	tools/CMakeLists.txt \
	cmake \
	$(NULL)

all-local: Doxyfile

update-authors:
	( cd $(srcdir) && git shortlog -s -e ) | cut -c 8- | sort > $(srcdir)/AUTHORS

DISTCHECK_CONFIGURE_FLAGS = \
	--enable-xml-docs \
	--with-systemdsystemunitdir=$$dc_install_base/$(systemdsystemunitdir)

ACLOCAL_AMFLAGS = -I m4 ${ACLOCAL_FLAGS}

# Add rules for code-coverage testing, as defined by AX_CODE_COVERAGE
include $(top_srcdir)/aminclude_static.am
