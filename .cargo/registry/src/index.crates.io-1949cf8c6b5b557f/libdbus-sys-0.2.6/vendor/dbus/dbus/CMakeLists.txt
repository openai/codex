configure_file(dbus-arch-deps.h.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-arch-deps.h )

add_definitions(-DDBUS_COMPILATION)

set(dbusinclude_HEADERS
    dbus.h
    dbus-address.h
    dbus-bus.h
    dbus-connection.h
    dbus-errors.h
    dbus-macros.h
    dbus-memory.h
    dbus-message.h
    dbus-misc.h
    dbus-pending-call.h
    dbus-protocol.h
    dbus-server.h
    dbus-shared.h
    dbus-signature.h
    dbus-syntax.h
    dbus-threads.h
    dbus-types.h
)
set(dbusinclude_ARCH_HEADERS
    ${CMAKE_CURRENT_BINARY_DIR}/dbus-arch-deps.h
)

### source code that goes in the installed client library
### and is specific to library functionality
set(DBUS_LIB_SOURCES
    dbus-address.c
    dbus-auth.c
    dbus-bus.c
    dbus-connection.c
    dbus-credentials.c
    dbus-errors.c
    dbus-keyring.c
    dbus-marshal-header.c
    dbus-marshal-byteswap.c
    dbus-marshal-recursive.c
    dbus-marshal-validate.c
    dbus-message.c
    dbus-misc.c
    dbus-nonce.c
    dbus-object-tree.c
    dbus-pending-call.c
    dbus-resources.c
    dbus-server.c
    dbus-server-socket.c
    dbus-server-debug-pipe.c
    dbus-sha.c
    dbus-signature.c
    dbus-syntax.c
    dbus-timeout.c
    dbus-threads.c
    dbus-transport.c
    dbus-transport-socket.c
    dbus-watch.c
)


if(UNIX)
    set(DBUS_LIB_SOURCES ${DBUS_LIB_SOURCES}
        dbus-uuidgen.c
        dbus-transport-unix.c
        dbus-server-unix.c
    )
else(UNIX)
    set(DBUS_LIB_SOURCES ${DBUS_LIB_SOURCES}
        dbus-transport-win.c
        dbus-server-win.c
    )
endif()

set(DBUS_LIB_HEADERS
    dbus-auth.h
    dbus-connection-internal.h
    dbus-credentials.h
    dbus-keyring.h
    dbus-marshal-header.h
    dbus-marshal-byteswap.h
    dbus-marshal-recursive.h
    dbus-marshal-validate.h
    dbus-message-internal.h
    dbus-message-private.h
    dbus-misc.h
    dbus-object-tree.h
    dbus-protocol.h
    dbus-resources.h
    dbus-server-debug-pipe.h
    dbus-server-protected.h
    dbus-server-unix.h
    dbus-sha.h
    dbus-timeout.h
    dbus-threads.h
    dbus-threads-internal.h
    dbus-transport.h
    dbus-transport-protected.h
    dbus-uuidgen.h
    dbus-watch.h
    ${CMAKE_BINARY_DIR}/config.h
)
if(UNIX)
    set(DBUS_LIB_HEADERS ${DBUS_LIB_HEADERS}
        dbus-transport-unix.h
    )
else(UNIX)
    set(DBUS_LIB_HEADERS ${DBUS_LIB_HEADERS}
        dbus-transport-win.h
    )
endif()


### source code that goes in the installed client library
### AND is generic utility functionality used by the 
### daemon or test programs (all symbols in here should 
### be underscore-prefixed)
set(DBUS_SHARED_SOURCES
    dbus-dataslot.c
    dbus-file.c
    dbus-hash.c
    dbus-internals.c
    dbus-list.c
    dbus-marshal-basic.c
    dbus-memory.c
    dbus-mempool.c
    dbus-string.c
    dbus-sysdeps.c
    dbus-pipe.c
)

set(DBUS_SHARED_HEADERS
    dbus-dataslot.h
    dbus-file.h
    dbus-hash.h
    dbus-internals.h
    dbus-list.h
    dbus-marshal-basic.h
    dbus-mempool.h
    dbus-string.h
    dbus-string-private.h
    dbus-pipe.h
    dbus-sysdeps.h
)

if(DBUS_ENABLE_EMBEDDED_TESTS)
    set(DBUS_SHARED_SOURCES ${DBUS_SHARED_SOURCES} dbus-test-tap.c)
    set(DBUS_SHARED_HEADERS ${DBUS_SHARED_HEADERS} dbus-test-tap.h)
    # ... else they are in the test library instead
endif()

### source code that is generic utility functionality used
### by the bus daemon or test apps, but is NOT included
### in the D-BUS client library (all symbols in here 
### should be underscore-prefixed but don't really need 
### to be unless they move to DBUS_SHARED_SOURCES later)
set(DBUS_UTIL_SOURCES
    dbus-asv-util.c
    dbus-mainloop.c
    dbus-message-util.c
    dbus-shell.c
    dbus-pollable-set.c
    dbus-pollable-set-poll.c
    dbus-string-util.c
    dbus-sysdeps-util.c
)

set(DBUS_UTIL_HEADERS
    dbus-asv-util.h
    dbus-mainloop.h
    dbus-shell.h
    dbus-pollable-set.h
    dbus-spawn.h
    dbus-test.h
)

### platform specific settings
if(WIN32)
    set(DBUS_SHARED_SOURCES ${DBUS_SHARED_SOURCES}
        dbus-file-win.c
        dbus-init-win.cpp
        dbus-sysdeps-win.c
        dbus-pipe-win.c
        dbus-sysdeps-thread-win.c
    )
    set(DBUS_SHARED_HEADERS ${DBUS_SHARED_HEADERS}
        dbus-sockets-win.h
        dbus-sysdeps-win.h
    )
    set(DBUS_UTIL_SOURCES ${DBUS_UTIL_SOURCES}
        dbus-spawn-win.c
        dbus-sysdeps-util-win.c
    )
    if(WINCE)
    set(DBUS_SHARED_SOURCES ${DBUS_SHARED_SOURCES}
        dbus-sysdeps-wince-glue.c
    )
    set(DBUS_SHARED_HEADERS ${DBUS_SHARED_HEADERS}
        dbus-sysdeps-wince-glue.h
    )
    endif()
else(WIN32)
    set(DBUS_SHARED_SOURCES ${DBUS_SHARED_SOURCES}
        dbus-file-unix.c
        dbus-pipe-unix.c
        dbus-sysdeps-unix.c
        dbus-sysdeps-pthread.c
        dbus-userdb.c
    )
    set(DBUS_SHARED_HEADERS ${DBUS_SHARED_HEADERS}
        dbus-server-unix.h
        dbus-transport-unix.h
        dbus-sysdeps-unix.h
        dbus-userdb.h
    )
    set(DBUS_UTIL_SOURCES ${DBUS_UTIL_SOURCES}
        dbus-userdb-util.c
        dbus-sysdeps-util-unix.c
    )
    if(ENABLE_TRADITIONAL_ACTIVATION)
        set(DBUS_UTIL_SOURCES ${DBUS_UTIL_SOURCES}
            dbus-spawn-unix.c
        )
    endif()
endif()

if(DBUS_HAVE_LINUX_EPOLL)
    set(DBUS_UTIL_SOURCES
        ${DBUS_UTIL_SOURCES}
        dbus-pollable-set-epoll.c
    )
endif()

set(libdbus_SOURCES
    ${DBUS_LIB_SOURCES}
    ${DBUS_SHARED_SOURCES}
)

set(libdbus_HEADERS
    ${DBUS_LIB_HEADERS}
    ${DBUS_SHARED_HEADERS}
)

if(DEFINED DBUS_LIBRARY_REVISION)
    math(EXPR DBUS_LIBRARY_MAJOR "${DBUS_LIBRARY_CURRENT} - ${DBUS_LIBRARY_AGE}")
endif()

if(WIN32)
    add_library_version_info(libdbus_SOURCES "${CMAKE_SHARED_LIBRARY_PREFIX}dbus-1-${DBUS_LIBRARY_MAJOR}")
endif()

if(MSVC_IDE)
    project_source_group(${GROUP_CODE} DBUS_LIB_SOURCES DBUS_LIB_HEADERS)
    project_source_group(${GROUP_CODE} DBUS_SHARED_SOURCES DBUS_SHARED_HEADERS)
    project_source_group(${GROUP_CODE} DBUS_UTIL_SOURCES DBUS_UTIL_SOURCES)
endif()

# for clock_getres() on e.g. GNU/Linux (but not Android)
find_library(LIBRT rt)

# for socket() on QNX
find_library(LIBSOCKET socket)

### Client library
add_library(dbus-1 SHARED
    ${libdbus_SOURCES}
    ${libdbus_HEADERS}
)

if(WIN32)
    if(DEFINED DBUS_LIBRARY_REVISION)
        set_target_properties(dbus-1 PROPERTIES SUFFIX "-${DBUS_LIBRARY_MAJOR}${CMAKE_SHARED_LIBRARY_SUFFIX}")
        add_custom_command(TARGET dbus-1 POST_BUILD
            COMMAND ${CMAKE_COMMAND} -E copy "$<TARGET_FILE:dbus-1>" "$<TARGET_FILE_DIR:dbus-1>/${CMAKE_SHARED_LIBRARY_PREFIX}dbus-1${CMAKE_SHARED_LIBRARY_SUFFIX}"
            COMMENT "Create non versioned dbus-1 library for legacy applications"
        )
        install(FILES ${LEGACY_FILE_NAME} DESTINATION ${CMAKE_INSTALL_BINDIR})
    endif()
    if(WINCE)
        target_link_libraries(dbus-1 ws2)
    else(WINCE)
        target_link_libraries(dbus-1 ws2_32 advapi32 netapi32 iphlpapi dbghelp)
    endif()
else(WIN32)
    if(DEFINED DBUS_LIBRARY_REVISION)
        set_target_properties(dbus-1 PROPERTIES VERSION ${DBUS_LIBRARY_MAJOR}.${DBUS_LIBRARY_AGE}.${DBUS_LIBRARY_REVISION} SOVERSION ${DBUS_LIBRARY_MAJOR})
    endif()
    target_link_libraries(dbus-1 ${CMAKE_THREAD_LIBS_INIT} ${SYSTEMD_LIBRARIES})
    if(LIBRT)
        target_link_libraries(dbus-1 ${LIBRT})
    endif()
    if(LIBSOCKET)
        target_link_libraries(dbus-1 ${LIBSOCKET})
    endif()
    if (HAVE_BACKTRACE)
        target_link_libraries(dbus-1 ${Backtrace_LIBRARY})
        target_include_directories(dbus-1 PRIVATE ${Backtrace_INCLUDE_DIR})
    endif()
endif()

target_include_directories(dbus-1 INTERFACE $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}/dbus-1.0>;$<INSTALL_INTERFACE:${CMAKE_INSTALL_LIBDIR}/dbus-1.0/include>)

# Assume that Linux has -Wl,--version-script and other platforms do not
if("${CMAKE_SYSTEM_NAME}" STREQUAL "Linux")
    set(SOVERSION ${DBUS_LIBRARY_MAJOR})
    configure_file(Version.in ${CMAKE_CURRENT_BINARY_DIR}/Version)
    set_target_properties(dbus-1 PROPERTIES LINK_FLAGS -Wl,--version-script=${CMAKE_CURRENT_BINARY_DIR}/Version)
endif()

install(TARGETS dbus-1 ${INSTALL_TARGETS_DEFAULT_ARGS})
install(FILES ${dbusinclude_HEADERS} DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}/dbus-1.0/dbus)
install(FILES ${dbusinclude_ARCH_HEADERS} DESTINATION ${CMAKE_INSTALL_LIBDIR}/dbus-1.0/include/dbus)

### Internal library, used for the daemon, tools and tests, compiled statically.

add_library(dbus-internal ${DBUS_INTERNAL_ADD_LIBRARY_OPTIONS}
    ${DBUS_UTIL_SOURCES}
    ${DBUS_UTIL_HEADERS}
)
target_link_libraries(dbus-internal dbus-1)
if(WIN32)
    if(WINCE)
        target_link_libraries(dbus-internal ws2)
    else(WINCE)
        target_link_libraries(dbus-internal ws2_32 advapi32 netapi32 iphlpapi)
    endif()
else(WIN32)
    target_link_libraries(dbus-internal ${CMAKE_THREAD_LIBS_INIT})
    if(LIBRT)
        target_link_libraries(dbus-internal ${LIBRT})
    endif()
    if(LIBSOCKET)
        target_link_libraries(dbus-internal ${LIBSOCKET})
    endif()
endif()

if(UNIX)
# set version info
endif()


## mop up the gcov files
#clean-local:
#/bin/rm *.bb *.bbg *.da *.gcov .libs/*.da .libs/*.bbg || true
