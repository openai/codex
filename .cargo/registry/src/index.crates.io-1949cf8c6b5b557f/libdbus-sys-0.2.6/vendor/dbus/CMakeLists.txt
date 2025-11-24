# where to look first for cmake modules, before ${CMAKE_ROOT}/Modules/ is checked
list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake")
list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake/modules")

# we do not need to have WIN32 defined
set(CMAKE_LEGACY_CYGWIN_WIN32 0)

project(dbus)

# we need to be up to date
cmake_minimum_required(VERSION 3.4 FATAL_ERROR)
if(COMMAND cmake_policy)
    cmake_policy(SET CMP0003 NEW)
endif()

if(CMAKE_MAJOR_VERSION GREATER 2)
    cmake_policy(SET CMP0026 NEW)
    if(CMAKE_MAJOR_VERSION GREATER 4 OR CMAKE_MINOR_VERSION GREATER 1)
        cmake_policy(SET CMP0053 NEW)
        cmake_policy(SET CMP0054 NEW)
    endif()
endif()

option(DBUS_BUILD_TESTS "enable unit test code" ON)

# detect version
include(MacrosAutotools)
autoinit(configure.ac)
autoversion(dbus)

# replacement for AC_C_BIGENDIAN
include (TestBigEndian)
test_big_endian(WORDS_BIGENDIAN)

if(EXISTS ${CMAKE_SOURCE_DIR}/config.h.in)
    autoheaderchecks(${CMAKE_SOURCE_DIR}/config.h.in ${CMAKE_SOURCE_DIR}/cmake/ConfigureChecks.cmake ${CMAKE_SOURCE_DIR}/cmake/config.h.cmake)
else()
    message(STATUS "Generate config.h.in with autogen.sh to enable cmake header difference check.")
endif()

# used by file version info
set(DBUS_PATCH_VERSION "0")

# set PACKAGE_... variables
autopackage(
    dbus
    ${DBUS_VERSION_STRING}
    "http://dbus.freedesktop.org"
    "https://gitlab.freedesktop.org/dbus/dbus/issues"
)

include(Macros)
string(TIMESTAMP DBUS_BUILD_TIMESTAMP "%Y%m%d%H%M" UTC)
set(BUILD_FILEVERSION ${DBUS_MAJOR_VERSION},${DBUS_MINOR_VERSION},${DBUS_MICRO_VERSION},${DBUS_PATCH_VERSION})
set(BUILD_TIMESTAMP ${DBUS_BUILD_TIMESTAMP})

########### basic vars ###############

if(UNIX AND CMAKE_SYSTEM_NAME STREQUAL "Linux")
    set(DBUS_LINUX 1)
endif()

include(GNUInstallDirs)

if(DBUSDIR)
    set(DBUS_INSTALL_DIR "${DBUSDIR}")
endif()
if($ENV{DBUSDIR})
    set(DBUS_INSTALL_DIR "$ENV{DBUSDIR}")
endif()

if(DBUS_INSTALL_DIR)
    set(CMAKE_INSTALL_PREFIX "${DBUS_INSTALL_DIR}" CACHE PATH "install prefix" FORCE)
else(DBUS_INSTALL_DIR)
    set(DBUS_INSTALL_DIR "${CMAKE_INSTALL_PREFIX}")
endif()

set(DBUS_PREFIX ${DBUS_INSTALL_DIR})

set(prefix                   ${DBUS_INSTALL_DIR})
set(exec_prefix              ${prefix})
set(DBUS_MACHINE_UUID_FILE   ${CMAKE_INSTALL_FULL_LOCALSTATEDIR}/lib/dbus/machine-id)
set(DBUS_BINDIR              ${CMAKE_INSTALL_FULL_BINDIR})
set(DBUS_DAEMONDIR           ${CMAKE_INSTALL_FULL_BINDIR})
set(DBUS_LOCALSTATEDIR       ${CMAKE_INSTALL_FULL_LOCALSTATEDIR})
set(DBUS_RUNSTATEDIR         ${CMAKE_INSTALL_FULL_LOCALSTATEDIR}/run)

# On Windows this is relative to where we put the bus setup, in
# ${datadir}/dbus-1. For simplicity, we only do this if
# ${sysconfdir} = ${prefix}/etc and ${datadir} = ${prefix}/share.
#
# On Unix, or on Windows with weird install layouts, it's the absolute path.
if(WIN32 AND ${CMAKE_INSTALL_FULL_SYSCONFDIR} STREQUAL ${prefix}/etc AND ${CMAKE_INSTALL_FULL_DATADIR} STREQUAL ${prefix}/share)
set(SYSCONFDIR_FROM_PKGDATADIR ../../etc)
set(DATADIR_FROM_PKGSYSCONFDIR ../../share)
else()
set(SYSCONFDIR_FROM_PKGDATADIR ${CMAKE_INSTALL_FULL_SYSCONFDIR})
set(DATADIR_FROM_PKGSYSCONFDIR ${CMAKE_INSTALL_FULL_DATADIR})
endif()

option(DBUS_RELOCATABLE "Attempt to make metadata relocatable" ON)
option(DBUS_ENABLE_PKGCONFIG "Enable pkgconfig support" ON)
option(ENABLE_VERBOSE_CONFIG "Be verbose on generating config files" OFF)

# For simplicity, we're not relocatable if CMAKE_INSTALL_LIBDIR
# is something more complicated (e.g. Debian multiarch);
# we don't want to have to compute how many ../ to add
if(CMAKE_INSTALL_LIBDIR STREQUAL "lib" OR CMAKE_INSTALL_LIBDIR STREQUAL "lib64")
    # We know we can use ../ to get to the prefix. Do nothing.
elseif(DBUS_RELOCATABLE)
    # Sorry, it's too hard to make this particular build relocatable
    message("Unusual CMAKE_INSTALL_LIBDIR: the generated package will not be relocatable.")
    set(DBUS_RELOCATABLE OFF)
endif()

# used in the C code
set(DBUS_LIBEXECDIR          ${CMAKE_INSTALL_FULL_LIBEXECDIR})
set(DBUS_DATADIR             ${CMAKE_INSTALL_FULL_DATADIR})

#enable building of shared library
set(BUILD_SHARED_LIBS ON)

set(INSTALL_TARGETS_DEFAULT_ARGS EXPORT DBus1Targets RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR} LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR} ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR})

if(CYGWIN)
    set(WIN32)
endif()

# search for required packages
if(WIN32)
    # include local header first to avoid using old installed header
    set(CMAKE_INCLUDE_PATH ${CMAKE_INCLUDE_PATH} ${CMAKE_SOURCE_DIR}/..)
    include(Win32Macros)
    addExplorerWrapper(${CMAKE_PROJECT_NAME})
endif()

if(NOT WIN32)
    set(CMAKE_THREAD_PREFER_PTHREAD ON)
    include(FindThreads)
endif()

option(DBUS_DISABLE_ASSERT "Disable assertion checking" OFF)

option(DBUS_ENABLE_STATS "enable bus daemon usage statistics" OFF)
option(ENABLE_TRADITIONAL_ACTIVATION "Enable traditional activation (without using systemd)" ON)

find_package(PkgConfig)

if(DBUS_LINUX)
    add_auto_option(ENABLE_SYSTEMD "build with systemd at_console support" AUTO)
    if (NOT PKG_CONFIG_FOUND)
        message(SEND_ERROR "pkg-config not found, this is required on Linux systems")
    endif()
    pkg_check_modules(SYSTEMD libsystemd>=209)
    if(NOT SYSTEMD_FOUND)
        pkg_check_modules(SYSTEMD libsystemd-login>=32 libsystemd-daemon>=32 libsystemd-journal>=32)
    endif()
    check_auto_option(ENABLE_SYSTEMD "systemd support" SYSTEMD_FOUND "systemd")
    if(ENABLE_SYSTEMD AND SYSTEMD_FOUND)
        set(DBUS_BUS_ENABLE_SYSTEMD ON)
        set(HAVE_SYSTEMD ${SYSTEMD_FOUND})
    endif()
    option(ENABLE_USER_SESSION "enable user-session semantics for session bus under systemd" OFF)
    set(DBUS_ENABLE_USER_SESSION ${ENABLE_USER_SESSION})

    add_path_option(WITH_SYSTEMD_SYSTEMUNITDIR "Directory for systemd service files" "")
    add_path_option(WITH_SYSTEMD_USERUNITDIR "Directory for systemd user service files" "")
    # get defaults
    pkg_check_modules(_SYSTEMD systemd)
    if(_SYSTEMD_FOUND)
        pkg_get_variable(_SYSTEMD_PREFIX systemd prefix)
        pkg_get_variable(_SYSTEMD_SYSTEMUNITDIR systemd systemdsystemunitdir)
        pkg_get_variable(_SYSTEMD_USERUNITDIR systemd systemduserunitdir)
        # remove install prefix, which may not match the current prefix
        string(REPLACE "${_SYSTEMD_PREFIX}/" "" DBUS_SYSTEMD_SYSTEMUNITDIR ${_SYSTEMD_SYSTEMUNITDIR})
        string(REPLACE "${_SYSTEMD_PREFIX}/" "" DBUS_SYSTEMD_USERUNITDIR ${_SYSTEMD_USERUNITDIR})
    else()
        set(DBUS_SYSTEMD_SYSTEMUNITDIR lib/systemd/system)
        set(DBUS_SYSTEMD_USERUNITDIR lib/systemd/user)
    endif()
    if(WITH_SYSTEMD_SYSTEMUNITDIR)
        set(DBUS_SYSTEMD_SYSTEMUNITDIR ${WITH_SYSTEMD_SYSTEMUNITDIR})
    endif()
    if(WITH_SYSTEMD_USERUNITDIR)
        set(DBUS_SYSTEMD_USERUNITDIR ${WITH_SYSTEMD_USERUNITDIR})
    endif()
endif()

if(NOT ENABLE_TRADITIONAL_ACTIVATION AND NOT (DBUS_LINUX AND DBUS_BUS_ENABLE_SYSTEMD))
    message(WARNING "\
Traditional activation and systemd activation are both disabled, so service \
activation (automatically starting services that receive messages) will not work. \
    ")
endif()

if(WIN32)
    set(FD_SETSIZE "8192" CACHE STRING "The maximum number of connections that can be handled at once")
endif()

find_package(EXPAT)
find_package(X11)
find_package(GLIB2)
if(GLIB2_FOUND)
    option(DBUS_WITH_GLIB "build with glib" ON)
endif()

# analogous to AC_USE_SYSTEM_EXTENSIONS in configure.ac
set(_GNU_SOURCE 1)

# do config checks
include(ConfigureChecks)

# make some more macros available
include(MacroLibrary)

if(VCS)
    set(DBUS_VERBOSE_C_S 1 CACHE STRING "verbose mode" FORCE)
endif()

if(MSVC)
    # controll folders in msvc projects
    include(ProjectSourceGroup)
    if(NOT GROUP_CODE)
        #set(GROUP_CODE split) #cmake default
        set(GROUP_CODE flat)
    endif()
    add_definitions(-D_CRT_SECURE_NO_DEPRECATE -D_CRT_NONSTDC_NO_DEPRECATE)
    string(APPEND CMAKE_C_FLAGS_DEBUG " /FIconfig.h")
    string(APPEND CMAKE_C_FLAGS_RELEASE " /FIconfig.h")
    option(DBUS_MSVC_ANALYZE "Enable code analyzing for MSVC compiler: /analyze" OFF)
endif()

#
# setup warnings
#
if(NOT MSVC)
    # We're treating -fno-common like a warning: it makes the linker more
    # strict, because on some systems the linker is *always* this strict
    string(APPEND CMAKE_C_FLAGS " -fno-common")
    string(APPEND CMAKE_CXX_FLAGS " -fno-common")
endif()

option(ENABLE_WERROR "Unconditionally make all compiler warnings fatal" OFF)

if(MSVC)
    # Use the highest warning level
    if(WALL)
        set(WALL 1 CACHE STRING "all warnings"  FORCE)
        set(CMAKE_CXX_WARNING_LEVEL 4 CACHE STRING "warning level" FORCE)
        if(CMAKE_CXX_FLAGS MATCHES "/W[0-4]")
            string(REGEX REPLACE "/W[0-4]" "/W4" CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS}")
        else()
            string(APPEND CMAKE_CXX_FLAGS " /W4")
        endif()

        if(CMAKE_C_FLAGS MATCHES "/W[0-4]")
            string(REGEX REPLACE "/W[0-4]" "/W4" CMAKE_C_FLAGS "${CMAKE_C_FLAGS}")
        else()
            string(APPEND CMAKE_C_FLAGS " /W4")
        endif()
    else()
        set(CMAKE_CXX_WARNING_LEVEL 3 CACHE STRING "warning level" FORCE)
    endif()

    # see https://msdn.microsoft.com/en-us/library/z78503e6.aspx
    # 4018 'expression' : signed/unsigned mismatch
    set(WARNINGS 4018)
    # 4090 'operation' : different 'modifier' qualifiers
    # 4101 'identifier' : unreferenced local variable
    # 4127 conditional expression is constant
    # 4244 'argument' : conversion from 'type1' to 'type2', possible loss of data
    set(WARNINGS_DISABLED 4090 4101 4127 4244)
    # 4002 too many actual parameters for macro 'identifier'
    # 4003 not enough actual parameters for macro 'identifier'
    # 4013 'function' undefined; assuming extern returning int
    # 4028 formal parameter 'number' different from declaration
    # 4031 second formal parameter list longer than the first list
    # 4047 operator' : 'identifier1' differs in levels of indirection from 'identifier2'
    # 4114 same type qualifier used more than once
    # 4133 'type' : incompatible types - from 'type1' to 'type2'
    set(WARNINGS_ERRORS 4002 4003 4013 4028 4031 4047 4114 4133)
    if(DBUS_MSVC_ANALYZE AND MSVC_VERSION GREATER 1600)
        string(APPEND CMAKE_C_FLAGS " /analyze")
    endif()
else()
    set(WARNINGS
        all
        array-bounds
        cast-align
        char-subscripts
        declaration-after-statement
        double-promotion
        duplicated-branches
        duplicated-cond
        extra
        float-equal
        format-nonliteral
        format-security
        format=2
        implicit-function-declaration
        init-self
        inline
        jump-misses-init
        logical-op
        missing-declarations
        missing-format-attribute
        missing-include-dirs
        missing-noreturn
        missing-prototypes
        nested-externs
        no-error=missing-field-initializers
        no-error=unused-label
        no-error=unused-parameter
        no-missing-field-initializers
        no-unused-label
        no-unused-parameter
        null-dereference
        old-style-definition
        packed
        pointer-arith
        pointer-sign
        redundant-decls
        restrict
        return-type
        shadow
        sign-compare
        strict-aliasing
        strict-prototypes
        switch-default
        switch-enum
        undef
        unused-but-set-variable
        write-strings
    )
    set(WARNINGS_C
        pointer-sign
    )
    set(WARNINGS_DISABLED
        error=overloaded-virtual
        error=missing-field-initializers
        error=unused-parameter
        unused-parameter
    )
    set(WARNINGS_ERRORS
    )
    if(ENABLE_WERROR)
        list(APPEND WARNINGS error)
    endif()
endif()

generate_compiler_warning_flags(
    RESULTVAR
        WARNINGS_CFLAGS
    WARNINGS
        ${WARNINGS}
        ${WARNINGS_C}
    DISABLED
        ${WARNINGS_DISABLED}
    ERRORS
        ${WARNINGS_ERRORS}
)
generate_compiler_warning_flags(
    RESULTVAR
        WARNINGS_CXXFLAGS
    WARNINGS
        ${WARNINGS}
    DISABLED
        ${WARNINGS_DISABLED}
    ERRORS
        ${WARNINGS_ERRORS}
)

string(APPEND CMAKE_C_FLAGS " ${WARNINGS_CFLAGS}")
string(APPEND CMAKE_CXX_FLAGS " ${WARNINGS_CFLAGS}")

# let wine be able to show file and lines in backtrace
if(DBUS_USE_WINE)
    string(APPEND CMAKE_C_FLAGS " -gstabs")
    string(APPEND CMAKE_CXX_FLAGS " -gstabs")
endif()

if(UNIX AND NOT DBUS_DISABLE_ASSERT)
    # required for backtrace
    if (APPLE)
        string(APPEND CMAKE_SHARED_LINKER_FLAGS " -Wl,-export_dynamic")
        string(APPEND CMAKE_EXE_LINKER_FLAGS " -Wl,-export_dynamic")
    else()
        string(APPEND CMAKE_SHARED_LINKER_FLAGS " -Wl,--export-dynamic")
        string(APPEND CMAKE_EXE_LINKER_FLAGS " -Wl,--export-dynamic")
    endif()
    set(DBUS_BUILT_R_DYNAMIC 1)
endif()

if(DBUS_WITH_GLIB)
    autodefine(GLIB_VERSION_MIN_REQUIRED)
    autodefine(GLIB_VERSION_MAX_ALLOWED)
endif()

string(APPEND CMAKE_C_FLAGS_DEBUG " -D_DEBUG")

#########################################################################
# Windows CE (>= 5.0.0)
#
# WinCE support now relies on the presence of platform files, found in cmake/modules/platform
# Cmake 2.8.0 doesn't include WinCE platform files by default, but working ones can be found
# on CMake's bugtracker :
# http://public.kitware.com/Bug/view.php?id=7919
#
# for cmake 2.8.0 get the following patch only :
# http://public.kitware.com/Bug/file_download.php?file_id=2944&type=bug
#
# after applying the patch, you can enable the WinCE build by specifying :
# cmake [...] -DCMAKE_SYSTEM_NAME=WinCE -DCMAKE_SYSTEM_VERSION=X.XX
# (where X.XX is your actual WinCE version, e.g. 5.02 for Windows Mobile 6)
#
# Note that you should have a proper cross-compilation environment set up prior to running
# cmake, ie. the PATH, INCLUDE and LIB env vars pointing to your CE SDK/toolchain.
#
if(WINCE)

message("Building for WinCE (${CMAKE_SYSTEM_VERSION})")

endif()
#########################################################################


enable_testing()

#########################################################################
# Disallow in-source build
#macro_ensure_out_of_source_build("dbus requires an out of source build. Please create a separate build directory and run 'cmake path_to_dbus [options]' there.")

# ... and warn in case of an earlier in-source build
#set(generatedFileInSourceDir EXISTS ${CMAKE_SOURCE_DIR}/config.h)
#if(${generatedFileInSourceDir})
#   message(STATUS "config.h exists in your source directory.")
#endif(${generatedFileInSourceDir})
#########################################################################

########### build output path definitions ###############
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
if(WIN32 OR CYGWIN)
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
else()
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
endif()
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)

# for including config.h and for includes like <dir/foo.h>
include_directories(. ${CMAKE_BINARY_DIR} ${CMAKE_INCLUDE_PATH})

# linker search directories
link_directories(${DBUS_LIB_DIR} ${CMAKE_LIBRARY_OUTPUT_DIRECTORY} )
include_directories( ${CMAKE_LIBRARY_PATH}  )

set(DBUS_INCLUDES)

enable_testing()

########### command line options ###############
if(DBUS_BUILD_TESTS)
    set(DBUS_ENABLE_EMBEDDED_TESTS ON)
    set(DBUS_ENABLE_MODULAR_TESTS ON)
endif()

option(DBUS_USE_OUTPUT_DEBUG_STRING "enable win32 debug port for message output" OFF)
if(WIN32)
    # win32 dbus service support - this support is not complete
    option(DBUS_SERVICE "enable dbus service installer" OFF)
endif()

option(DBUS_ENABLE_ANSI "enable -ansi -pedantic gcc flags" OFF)
if(DBUS_ENABLE_ANSI)
    if(NOT MSVC)
        add_definitions(-ansi -D_POSIX_C_SOURCE=199309L -D_BSD_SOURCE -pedantic)
    else(NOT MSVC)
        add_definitions(-Za -D_POSIX_C_SOURCE=199309L -D_BSD_SOURCE -W4)
    endif()
endif()

option(DBUS_ENABLE_VERBOSE_MODE "support verbose debug mode" ON)

option(DBUS_DISABLE_CHECKS "Disable public API sanity checking" OFF)

if(NOT MSVC)
    option(DBUS_GCOV_ENABLED "compile with coverage profiling instrumentation (gcc only)" OFF)
    if(DBUS_GCOV_ENABLED)
        add_definitions(-fprofile-arcs -ftest-coverage)
        # FIXME!!!!
        ## remove optimization
        # CFLAGS=`echo "$CFLAGS" | sed -e 's/-O[0-9]*//g'`
    endif()
endif()

if("${CMAKE_SYSTEM_NAME}" STREQUAL "Linux")
    option(DBUS_BUS_ENABLE_INOTIFY "build with inotify support (linux only)" ON)
    if(DBUS_BUS_ENABLE_INOTIFY)
        if(NOT HAVE_SYS_INOTIFY_H)
            message(FATAL_ERROR "sys/inotify.h not found!")
        endif()
    endif()
elseif("${CMAKE_SYSTEM_NAME}" MATCHES ".*BSD")
    option(DBUS_BUS_ENABLE_KQUEUE "build with kqueue support (FreeBSD only)" ON)
    if(DBUS_BUS_ENABLE_KQUEUE)
        if(NOT HAVE_SYS_EVENT_H)
            message(FATAL_ERROR "sys/event.h not found!")
        endif()
    endif()
endif()

string(TOUPPER ${CMAKE_SYSTEM_NAME} sysname)
if("${sysname}" MATCHES ".*SOLARIS.*")
    option(HAVE_CONSOLE_OWNER_FILE "enable console owner file(solaris only)" ON)
    if(HAVE_CONSOLE_OWNER_FILE)
        set(DBUS_CONSOLE_OWNER_FILE "/dev/console" CACHE STRING "Directory to check for console ownerhip")
    endif()
endif()

if(NOT EXPAT_FOUND)
    message(FATAL_ERROR "expat not found!")
endif()

# all missing or hardcoded for now

# 'hidden' ones
set(atomic_int OFF)
set(atomic_int486 OFF)
if(CMAKE_COMPILER_IS_GNUCC AND NOT DBUS_ENABLE_ANSI)
    find_program(UNAME_EXECUTABLE
        NAMES uname
        PATHS /bin /usr/bin /usr/local/bin c:/Programme/MSys/bin d:/Programme/MSys/bin
    )

    if(UNAME_EXECUTABLE)
        execute_process(
            COMMAND ${UNAME_EXECUTABLE} "-m"
            OUTPUT_VARIABLE UNAME_OUTPUT
        )

        if("UNAME_OUTPUT" MATCHES "^.*i[0123]86.*$")
            set(atomic_int ON)
        else("UNAME_OUTPUT" MATCHES "^.*i[0123]86.*$")
            if("UNAME_OUTPUT" MATCHES "^.*i?86.*$")
                set(atomic_int ON)
                set(atomic_int_486 ON)
            endif()
        endif()
    endif()
endif()

if(X11_FOUND)
    option(DBUS_BUILD_X11 "Build with X11 autolaunch support " ON)
endif()

# test binary names
if(WIN32)
    # follow Automake's naming convention so we can share .in files
    set(EXEEXT ${CMAKE_EXECUTABLE_SUFFIX})
endif()

if(MSVC_IDE)
    if(CMAKE_BUILD_TYPE MATCHES Debug)
        set(IDE_BIN /Debug )
        message(STATUS)
        message(STATUS "Visual Studio: test programs will only work with 'Debug' configuration!")
        message(STATUS "To run tests with 'Release' configuration use -DCMAKE_BUILD_TYPE=Release")
        message(STATUS "Add '..\\..\\test\\data' to the command line option of the test programs")
        message(STATUS)
    else(CMAKE_BUILD_TYPE MATCHES Debug)
        set(IDE_BIN /Release)
        message(STATUS)
        message(STATUS "Visual Studio: test programs will only work with 'Release' configuration!")
        message(STATUS "To run tests with 'Debug' configuration use -DCMAKE_BUILD_TYPE=Debug")
        message(STATUS "Add '..\\..\\test\\data' to the command line option of the test programs")
        message(STATUS)
    endif()
    set(TEST_PATH_FORCE FORCE)
    file(REMOVE ${CMAKE_BINARY_DIR}/data/dbus-1/services)
endif()

#### Find socket directories
set(DBUS_SESSION_SOCKET_DIR "" CACHE STRING "Default directory for session socket")
if(UNIX)
    if (CMAKE_CROSSCOMPILING)
        if (NOT DBUS_SESSION_SOCKET_DIR)
            message(FATAL_ERROR "cannot autodetect session socket directory "
                    "when crosscompiling, pass -DDBUS_SESSION_SOCKET_DIR=...")
        endif()
    elseif(NOT $ENV{TMPDIR} STREQUAL "")
        set(DBUS_SESSION_SOCKET_DIR $ENV{TMPDIR})
    elseif(NOT $ENV{TEMP} STREQUAL "")
        set(DBUS_SESSION_SOCKET_DIR $ENV{TEMP})
    elseif(NOT $ENV{TMP} STREQUAL "")
        set(DBUS_SESSION_SOCKET_DIR $ENV{TMP})
    else()
        set(DBUS_SESSION_SOCKET_DIR /tmp)
    endif()
endif()

 # Not used on Windows, where there is no system bus
set(DBUS_SYSTEM_PID_FILE ${DBUS_RUNSTATEDIR}/dbus/pid)

set(DBUS_CONSOLE_AUTH_DIR "" CACHE STRING "Directory to check for pam_console/pam_foreground flag files, or empty to ignore")

# This won't work on Windows. It's not meant to - the system bus is
# meaningless on Windows anyway.
#
# This has to be suitable for hard-coding in client libraries as well as
# in the dbus-daemon's configuration, so it has to be valid to listen on
# and also to connect to. If this ever changes, it'll need to be split into
# two variables, one for the listening address and one for the connecting
# address.
set(DBUS_SYSTEM_BUS_DEFAULT_ADDRESS "unix:path=${DBUS_RUNSTATEDIR}/dbus/system_bus_socket" CACHE STRING "system bus default address")

if(WIN32)
    set(DBUS_SESSION_BUS_LISTEN_ADDRESS "autolaunch:" CACHE STRING "session bus default listening address")
    set(DBUS_SESSION_BUS_CONNECT_ADDRESS "autolaunch:" CACHE STRING "session bus fallback address for clients")

    set(DBUS_SYSTEM_CONFIG_FILE "share/dbus-1/system.conf")
    set(DBUS_SESSION_CONFIG_FILE "share/dbus-1/session.conf")
  # bus-test expects a non empty string
    set(DBUS_USER "Administrator")
    set(DBUS_TEST_USER "guest")
    set(DBUS_SESSION_CONF_MAYBE_AUTH_EXTERNAL "<!--<auth>EXTERNAL</auth>-->")
else(WIN32)
    set(DBUS_SESSION_BUS_LISTEN_ADDRESS "unix:tmpdir=${DBUS_SESSION_SOCKET_DIR}" CACHE STRING "session bus default listening address")
    set(DBUS_SESSION_BUS_CONNECT_ADDRESS "autolaunch:" CACHE STRING "session bus fallback address for clients")
    set(DBUS_SYSTEM_CONFIG_FILE  ${DBUS_DATADIR}/dbus-1/system.conf)
    set(DBUS_SESSION_CONFIG_FILE ${DBUS_DATADIR}/dbus-1/session.conf)
    set(DBUS_USER "messagebus")
    set(DBUS_TEST_USER "nobody")
  # For best security, assume that all non-Windows platforms can do
  # credentials-passing.
    set(DBUS_SESSION_CONF_MAYBE_AUTH_EXTERNAL "<auth>EXTERNAL</auth>")
endif()

set(DBUS_DAEMON_NAME "dbus-daemon" CACHE STRING "The name of the dbus daemon executable")

########### create config.h ###############

#include(ConfigureChecks.cmake)

# compiler definitions
add_definitions(-DHAVE_CONFIG_H)
add_definitions(${DBUS_BUS_CFLAGS})


if(DBUS_BUILD_TESTS)
    # set variables used for the .in files (substituted by configure_file) in test/data:
    set(DBUS_TEST_EXEC ${Z_DRIVE_IF_WINE}${CMAKE_RUNTIME_OUTPUT_DIRECTORY}${IDE_BIN})
    # Working directory for build-time tests, so that they'll pick up
    # the correct libdbus-1-3.dll on Windows.
    # This happens to be the same as DBUS_TEST_EXEC, but its meaning is
    # different, and it has no direct Autotools equivalent (Autotools
    # tests automatically get their own ${builddir} as working directory).
    set(DBUS_TEST_WORKING_DIR ${CMAKE_RUNTIME_OUTPUT_DIRECTORY}${IDE_BIN})
    set(DBUS_TEST_DATA ${Z_DRIVE_IF_WINE}${CMAKE_BINARY_DIR}/test/data)
    set(DBUS_TEST_DAEMON ${Z_DRIVE_IF_WINE}${CMAKE_RUNTIME_OUTPUT_DIRECTORY}${IDE_BIN}/dbus-daemon${EXEEXT})
    set(DBUS_TEST_DBUS_LAUNCH ${Z_DRIVE_IF_WINE}${CMAKE_RUNTIME_OUTPUT_DIRECTORY}${IDE_BIN}/dbus-launch${EXEEXT})
    set(DBUS_TEST_HOMEDIR ${Z_DRIVE_IF_WINE}${CMAKE_BINARY_DIR}/dbus)
    set(DBUS_TEST_RUN_SESSION ${Z_DRIVE_IF_WINE}${CMAKE_RUNTIME_OUTPUT_DIRECTORY}${IDE_BIN}/dbus-run-session${EXEEXT})
    set(TEST_LAUNCH_HELPER_BINARY ${Z_DRIVE_IF_WINE}${DBUS_TEST_EXEC}/dbus-daemon-launch-helper-for-tests${EXEEXT})
    if(UNIX)
        set(TEST_SOCKET_DIR ${DBUS_SESSION_SOCKET_DIR})
        set(TEST_LISTEN "unix:tmpdir=${TEST_SOCKET_DIR}")
    endif()
    if(WIN32)
        set(TEST_LISTEN "tcp:host=localhost")
    endif()
endif()

set(DBUS_LIBRARIES dbus-1)
set(DBUS_INTERNAL_LIBRARIES dbus-internal)

set(DBUS_INTERNAL_ADD_LIBRARY_OPTIONS STATIC)
set(DBUS_INTERNAL_CLIENT_DEFINITIONS "-DDBUS_COMPILATION")

configure_file(cmake/config.h.cmake ${CMAKE_CURRENT_BINARY_DIR}/config.h )

if(WIN32)
configure_file(cmake/dbus-env.bat.cmake ${CMAKE_BINARY_DIR}/bin/dbus-env.bat )
    install(FILES ${CMAKE_BINARY_DIR}/bin/dbus-env.bat DESTINATION ${CMAKE_INSTALL_BINDIR})
endif()

#
# create cmake find_package related files
#
set(INSTALL_CMAKE_DIR ${CMAKE_INSTALL_LIBDIR}/cmake/DBus1)
if(WIN32)
    configure_file(cmake/DBus1Config.cmake.in "${CMAKE_BINARY_DIR}/DBus1Config.cmake" @ONLY)
else()
    configure_file(cmake/DBus1Config.pkgconfig.in "${CMAKE_BINARY_DIR}/DBus1Config.cmake" @ONLY)
endif()
configure_file(cmake/DBus1ConfigVersion.cmake.in "${CMAKE_BINARY_DIR}/DBus1ConfigVersion.cmake" @ONLY)
install(FILES
    "${CMAKE_BINARY_DIR}/DBus1Config.cmake"
    "${CMAKE_BINARY_DIR}/DBus1ConfigVersion.cmake"
    DESTINATION "${INSTALL_CMAKE_DIR}"
)

if(WIN32)
    install(EXPORT DBus1Targets DESTINATION "${INSTALL_CMAKE_DIR}")
endif()

########### subdirs ###############

add_subdirectory( dbus )
add_subdirectory( bus )
if(DBUS_BUILD_TESTS)
    add_subdirectory( test )
    add_custom_target(check
        COMMAND ctest -R ^test-.*
    )
endif()
add_subdirectory( tools )
add_subdirectory( doc )


option(DBUS_INSTALL_SYSTEM_LIBS "install required system libraries" OFF)
message(" ")
message("set -DDBUS_INSTALL_SYSTEM_LIBS=1 to install runtime libraries too")
message("set DBUSDIR (environment or cmake option) to overwrite the default install directory ")
message(" ")
message(" ")
get_filename_component(C_COMPILER ${CMAKE_C_COMPILER} NAME)
get_filename_component(CXX_COMPILER ${CMAKE_CXX_COMPILER} NAME)

message("                  D-BUS ${DBUS_VERSION}                               ")
message("                  =============                                       ")
message("                                                                      ")
message("        install prefix:           ${prefix}                           ")
message("        install exec_prefix:      ${exec_prefix}                      ")
message("        install libdir:           ${CMAKE_INSTALL_FULL_LIBDIR}        ")
message("        install libexecdir:       ${CMAKE_INSTALL_FULL_LIBEXECDIR}    ")
message("        install bindir:           ${CMAKE_INSTALL_FULL_BINDIR}        ")
message("        install sysconfdir:       ${CMAKE_INSTALL_FULL_SYSCONFDIR}    ")
message("        install datadir:          ${CMAKE_INSTALL_FULL_DATADIR}       ")
message("        source code location:     ${CMAKE_SOURCE_DIR}                 ")
message("        build dir:                ${CMAKE_BINARY_DIR}                 ")
message("        c compiler:               ${C_COMPILER}                       ")
message("        cflags:                   ${CMAKE_C_FLAGS}                    ")
message("        cflags debug:             ${CMAKE_C_FLAGS_DEBUG}              ")
message("        cflags release:           ${CMAKE_C_FLAGS_RELEASE}            ")
message("        cxx compiler:             ${CXX_COMPILER}                     ")
message("        cxxflags:                 ${CMAKE_CXX_FLAGS}                  ")
message("        cxxflags debug:           ${CMAKE_CXX_FLAGS_DEBUG}            ")
message("        cxxflags release:         ${CMAKE_CXX_FLAGS_RELEASE}          ")
message("        64-bit int:               ${DBUS_INT64_TYPE}                  ")
message("        32-bit int:               ${DBUS_INT32_TYPE}                  ")
message("        16-bit int:               ${DBUS_INT16_TYPE}                  ")
message("        Doxygen:                  ${DOXYGEN}                          ")
message("        Docbook Generator:        ${DOCBOOK_GENERATOR_NAME}           ")


message("        gcc coverage profiling:   ${DBUS_GCOV_ENABLED}                ")
if(MSVC)
message("        MSVC code analyze mode:   ${DBUS_MSVC_ANALYZE}                ")
endif()
message("        Building unit tests:      ${DBUS_BUILD_TESTS}                 ")
message("        Building with GLib:       ${DBUS_WITH_GLIB}                   ")
message("        Building verbose mode:    ${DBUS_ENABLE_VERBOSE_MODE}         ")
message("        Building w/o assertions:  ${DBUS_DISABLE_ASSERT}              ")
message("        Building w/o checks:      ${DBUS_DISABLE_CHECKS}              ")
message("        Building bus stats API:   ${DBUS_ENABLE_STATS}                ")
message("        installing system libs:   ${DBUS_INSTALL_SYSTEM_LIBS}         ")
message("        Building inotify support: ${DBUS_BUS_ENABLE_INOTIFY}          ")
message("        Building kqueue support:  ${DBUS_BUS_ENABLE_KQUEUE}           ")
message("        Building systemd support: ${DBUS_BUS_ENABLE_SYSTEMD}          ")
message("        systemd system install dir:${DBUS_SYSTEMD_SYSTEMUNITDIR}      ")
message("        systemd user install dir: ${DBUS_SYSTEMD_USERUNITDIR}         ")
message("        Traditional activation:   ${ENABLE_TRADITIONAL_ACTIVATION}    ")
message("        Building Doxygen docs:    ${DBUS_ENABLE_DOXYGEN_DOCS}         ")
message("        Building Qt help docs:    ${DBUS_ENABLE_QTHELP_DOCS}          ")
message("        Building XML docs:        ${DBUS_ENABLE_XML_DOCS}             ")
message("        Daemon executable name:   ${DBUS_DAEMON_NAME}")
if(WIN32)
message("        Session bus listens on:   ${DBUS_SESSION_BUS_LISTEN_ADDRESS}  ")
message("        Session clients connect to: ${DBUS_SESSION_BUS_CONNECT_ADDRESS} ")
else()
message("        System bus address:       ${DBUS_SYSTEM_BUS_DEFAULT_ADDRESS}  ")
message("        System bus socket:        ${DBUS_SYSTEM_SOCKET}               ")
message("        System bus PID file:      ${DBUS_SYSTEM_PID_FILE}             ")
message("        System bus user:          ${DBUS_USER}                        ")
message("        Session bus socket dir:   ${DBUS_SESSION_SOCKET_DIR}          ")

if(DBUS_CONSOLE_AUTH_DIR)
message("        pam_console auth dir:     ${DBUS_CONSOLE_AUTH_DIR}            ")
else()
message("        pam_console auth dir:     (none)                              ")
endif()

message("        'make check' socket dir:  ${TEST_SOCKET_DIR}                  ")
endif()
message("        Test listen address:      ${TEST_LISTEN}                      ")
if(MSVC)
message("        build timestamp:          ${DBUS_BUILD_TIMESTAMP}             ")
endif()

message(" ")
if(DBUS_BUILD_TESTS)
    message("NOTE: building with unit tests increases the size of the installed library and renders it insecure.")
endif()

if(DBUS_BUILD_TESTS AND DBUS_DISABLE_ASSERT)
    message("NOTE: building with unit tests but without assertions means tests may not properly report failures (this configuration is only useful when doing something like profiling the tests)")
endif()

if(DBUS_GCOV_ENABLED)
    message("NOTE: building with coverage profiling is definitely for developers only.")
endif()

if(DBUS_ENABLE_VERBOSE_MODE)
    message("NOTE: building with verbose mode increases library size, may slightly increase security risk, and decreases performance.")
endif()

if(NOT DBUS_DISABLE_ASSERT)
    message("NOTE: building with assertions increases library size and decreases performance.")
endif()

if(DBUS_DISABLE_CHECKS)
    message("NOTE: building without checks for arguments passed to public API makes it harder to debug apps using D-BUS, but will slightly decrease D-BUS library size and _very_ slightly improve performance.")
endif()

foreach(_note ${FOOTNOTES})
    message(${_note})
endforeach()

message(" ")

include(CPackInstallConfig)

add_custom_target(help-options
    cmake -LH
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)

#
# create pkgconfig file
#
if(DBUS_ENABLE_PKGCONFIG)
    set(PLATFORM_LIBS pthread ${LIBRT})
    if(PKG_CONFIG_FOUND)
        # convert lists of link libraries into -lstdc++ -lm etc..
        foreach(LIB ${CMAKE_C_IMPLICIT_LINK_LIBRARIES} ${PLATFORM_LIBS})
            set(LIBDBUS_LIBS "${LIBDBUS_LIBS} -l${LIB}")
        endforeach()
        set(original_prefix "${CMAKE_INSTALL_PREFIX}")
        if(DBUS_RELOCATABLE)
            set(pkgconfig_prefix "\${pcfiledir}/../..")
        else()
            set(pkgconfig_prefix "\${original_prefix}")
        endif()
        set(exec_prefix "\${prefix}")
        set(bindir "\${prefix}/${CMAKE_INSTALL_BINDIR}")
        set(libdir "\${prefix}/${CMAKE_INSTALL_LIBDIR}")
        set(includedir "\${prefix}/${CMAKE_INSTALL_INCLUDEDIR}")
        set(sysconfdir "\${prefix}/${CMAKE_INSTALL_SYSCONFDIR}")
        set(datadir "\${prefix}/${CMAKE_INSTALL_DATADIR}")
        set(datarootdir "\${prefix}/${CMAKE_INSTALL_DATADIR}")
        set(dbus_daemondir "\${prefix}/${CMAKE_INSTALL_BINDIR}")
        configure_file(dbus-1.pc.in ${CMAKE_BINARY_DIR}/dbus-1.pc @ONLY)
        install(FILES ${CMAKE_BINARY_DIR}/dbus-1.pc DESTINATION ${CMAKE_INSTALL_LIBDIR}/pkgconfig)
    endif()
endif()
