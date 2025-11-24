This file describes how to compile dbus using the cmake build system

Requirements
------------
- cmake version >= 2.6.0 see http://www.cmake.org
- installed libexpat see http://sourceforge.net/projects/expat/ 
    unsupported RelWithDebInfo builds could be fetched 
    from http://sourceforge.net/projects/kde-windows/files/expat/
- for generating documentation based on docbook files
    - xsltproc (provided by the distribution or from http://xmlsoft.org/XSLT/xsltproc2.html) and
    - docbook dtd and stylesheets (provided by the distribution or from https://sourceforge.net/projects/docbook/)

Building
--------

Win32 MinGW-w64|32
1. install mingw-w64 from http://sourceforge.net/projects/mingw-w64/
2. install cmake, libexpat and optional xsltproc and docbook dtd and stylesheets
3. get dbus sources
4. unpack dbus sources into a sub directory (referred as <dbus-src-root> later)
5. mkdir dbus-build
6. cd dbus-build
7. run 
    cmake -G "MinGW Makefiles" [<options, see below>] <dbus-src-root>
    mingw32-make
    mingw32-make install

Mingw32 cross compile on Linux
2. install cmake, libexpat and optional xsltproc and docbook dtd and stylesheets
    e.g on openSUSE Leap 15.0:
      sudo zypper ar -f https://download.opensuse.org/repositories/windows:/mingw:/win32/openSUSE_Leap_15.0/
      sudo zypper install mingw32-cross-gcc-c++ mingw32-libexpat-devel libxslt-tools mingw32-docbook_4 mingw32-docbool-xsl-stylesheets
2. get dbus sources
3. unpack dbus sources into a sub directory (referred as <dbus-src-root> later)
4. mkdir dbus-build
5. cd dbus-build
6. run
    cmake -DCMAKE_TOOLCHAIN_FILE=<dbus-src-root>/cmake/i686-w64-mingw32.cmake \
          -DCMAKE_FIND_ROOT_PATH=/usr/i686-w64-mingw32/sysroot/i686-w64-mingw32 \
          [<options, see below>] <dbus-src-root>
    make
    make install

    Win32 Microsoft nmake
1. install MSVC 2010 Express Version from http://www.microsoft.com/visualstudio/en-us/products/2010-editions/visual-cpp-express
2. install cmake, libexpat and optional xsltproc and docbook dtd and stylesheets
3. get dbus sources
4. unpack dbus sources into a sub directory (referred as <dbus-src-root> later)
5. mkdir dbus-build
6. cd dbus-build
7. run 
    cmake -G "NMake Makefiles" [<options, see below>] <dbus-src-root>
    nmake
    nmake install

Win32 Visual Studio 2010 Express IDE
1. install MSVC 2010 Express Version from http://www.microsoft.com/visualstudio/en-us/products/2010-editions/visual-cpp-express
2. install cmake, libexpat and optional xsltproc and docbook dtd and stylesheets
3. get dbus sources
4. unpack dbus sources into a sub directory (referred as <dbus-src-root> later)
5. mkdir dbus-build
6. cd dbus-build
7. run
      cmake -G "Visual Studio 10" [<options, see below>] <dbus-src-root>
8a. open IDE with
      vcexpress dbus.sln
8b. for immediate build run
      vcexpress dbus.sln /build

Win32 Visual Studio 2010 Professional IDE
1. install MSVC 2010 Professional Version
2. install cmake, libexpat and optional xsltproc and docbook dtd and stylesheets
3. get dbus sources
4. unpack dbus sources into a sub directory (referred as <dbus-src-root> later)
5. mkdir dbus-build
6. cd dbus-build
7. run 
      cmake -G "Visual Studio 10" [<options, see below>] <dbus-src-root>
8a. open IDE with
      devenv dbus.sln
8b. for immediate build run
      devenv dbus.sln /build

Linux
2. install cmake, libexpat and optional xsltproc and docbook dtd and stylesheets
2. get dbus sources
3. unpack dbus sources into a sub directory (referred as <dbus-src-root> later)
4. mkdir dbus-build
5. cd dbus-build
6. run 
    cmake [<options, see below>] <dbus-src-root>
    make
    make install

For other compilers see cmake --help in the Generators section

Configuration flags
-------------------

When using the cmake build system the dbus-specific configuration flags that can be given 
to the cmake program are these (use -D<key>=<value> on command line). The listed values 
are the defaults (in a typical build - some are platform-specific).

// in case cmake is not able to determine the xsltproc executable (optional)
XSLTPROC_EXECUTABLE=<path to xsltproc executable)

// in case cmake is not able to determine the installation of the docbook stylesheets (optional)
DOCBOOKXSL_DIR=<path to docbook stylesheets)

// Choose the type of build, options are: None(CMAKE_CXX_FLAGS or
// CMAKE_C_FLAGS used) Debug Release RelWithDebInfo MinSizeRel.
CMAKE_BUILD_TYPE:STRING=Debug

// Include path for 3rdparty packages
CMAKE_INCLUDE_PATH:PATH=

// Library path for 3rdparty packages
CMAKE_LIBRARY_PATH:PATH=

// Install path prefix, prepended onto install directories.
CMAKE_INSTALL_PREFIX:PATH=C:/Program Files/dbus


// enable unit test code
DBUS_BUILD_TESTS:BOOL=ON

// The name of the dbus daemon executable
DBUS_DAEMON_NAME:STRING=dbus-daemon

// Disable assertion checking
DBUS_DISABLE_ASSERT:BOOL=OFF

// Disable public API sanity checking
DBUS_DISABLE_CHECKS:BOOL=OFF

// enable -ansi -pedantic gcc flags
DBUS_ENABLE_ANSI:BOOL=OFF

// build DOXYGEN documentation (requires Doxygen)
DBUS_ENABLE_DOXYGEN_DOCS:BOOL=OFF

// Unconditionally make all compiler warnings fatal
ENABLE_WERROR:BOOL=OFF

// build qt help documentation (requires qhelpgenerator(-qt5));
// set INSTALL_QCH_DIR for custom qch installation path
ENABLE_QT_HELP:STRING=AUTO

// enable bus daemon usage statistics
DBUS_ENABLE_STATS:BOOL=OFF

// build with systemd at_console support
ENABLE_SYSTEMD:STRING=AUTO

// Directory for systemd service files
WITH_SYSTEMD_SYSTEMUNITDIR:STRING=

// Be verbose on generating config files
ENABLE_VERBOSE_CONFIG:BOOL=OFF

// enable user-session semantics for session bus under systemd
ENABLE_USER_SESSION:BOOL=ON

// Directory for systemd user service files
WITH_SYSTEMD_USERUNITDIR:STRING=

// support verbose debug mode
DBUS_ENABLE_VERBOSE_MODE:BOOL=ON

// build XML  documentation (requires xmlto or meinproc4)
DBUS_ENABLE_XML_DOCS:BOOL=ON

// install required system libraries
DBUS_INSTALL_SYSTEM_LIBS:BOOL=OFF

// session bus default listening address
DBUS_SESSION_BUS_LISTEN_ADDRESS:STRING=autolaunch:

// session bus fallback address for clients
DBUS_SESSION_BUS_CONNECT_ADDRESS:STRING=autolaunch:

//Default directory for session socket
DBUS_SESSION_SOCKET_DIR:STRING=/tmp

// system bus default address (only useful on Unix)
DBUS_SYSTEM_BUS_DEFAULT_ADDRESS:STRING=unix:path=/var/run/dbus/system_bus_socket

win32 only:
// enable win32 debug port for message output
DBUS_USE_OUTPUT_DEBUG_STRING:BOOL=OFF

// use wine to run cross compiled test applications (only for cross compiling for Windows)
DBUS_USE_WINE:BOOL=OFF

gcc only:
// compile with coverage profiling instrumentation
DBUS_GCOV_ENABLED:BOOL=OFF

solaris only:
// enable console owner file 
HAVE_CONSOLE_OWNER_FILE:BOOL=ON

// Directory to check for console ownership
DBUS_CONSOLE_OWNER_FILE:STRING=/dev/console

// Linux only:
// enable inotify as dir watch backend
DBUS_BUS_ENABLE_INOTIFY:BOOL=ON

*BSD only:
// enable kqueue as dir watch backend
DBUS_BUS_ENABLE_KQUEUE:BOOL=ON

x11 only:
// Build with X11 auto launch support
DBUS_BUILD_X11:BOOL=ON

MSVC only (Visual Studio >= 2012):
// Enable code analyzing for MSVC compiler: /analyze
DBUS_MSVC_ANALYZE:BOOL=OFF

Note: Above mentioned options starting with DBUS_ or CMAKE_ could
be extracted after configuring from the output of running
"<maketool> help-options" in the build directory.


How to compile in dbus into clients with cmake
----------------------------------------------

To compile dbus library into a client application with cmake
the following cmake commands are required:

1. call find_package to find dbus package

find_package(DBus1)

2. after specifing targets link dbus into target

add_executable(test test.c)
target_link_libraries(test ${DBus1_LIBRARIES})

Adding ${DBus1_LIBRARIES} to targets also adds required dbus
include dirs and compiler definitions by default. There is
no need to add them with include_directories and add_definitions.

To compile against dbus installed in a non standard location
specify -DDBus1_DIR=<dbus-install-root>/lib[64]/cmake/DBus1
on cmake command line.
