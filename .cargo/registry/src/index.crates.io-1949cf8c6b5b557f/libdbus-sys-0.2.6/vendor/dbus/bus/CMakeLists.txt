add_definitions(-DDBUS_COMPILATION)

set(EFENCE "")

# config files for installation 
configure_file( "session.conf.in" "${CMAKE_CURRENT_BINARY_DIR}/session.conf" IMMEDIATE @ONLY)
configure_file( "legacy-config/session.conf.in" "${CMAKE_CURRENT_BINARY_DIR}/legacy-config/session.conf" IMMEDIATE @ONLY)
file(MAKE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/session.d)

if(NOT WIN32)
    configure_file( "system.conf.in" "${CMAKE_CURRENT_BINARY_DIR}/system.conf" IMMEDIATE @ONLY)
    configure_file( "legacy-config/system.conf.in" "${CMAKE_CURRENT_BINARY_DIR}/legacy-config/system.conf" IMMEDIATE @ONLY)
    file(MAKE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/system.d)
endif()

# copy services for local daemon start to local service dir data/dbus-1/services
set(SERVICE_FILES test/data/valid-service-files)
file(GLOB FILES "${SERVICE_FILES}/*.service.in" )
foreach(FILE ${FILES})
    get_filename_component(FILENAME ${FILE} NAME_WE)
    set(TARGET ${CMAKE_BINARY_DIR}/data/dbus-1/services/${FILENAME}.service)
    if(CONFIG_VERBOSE)
        message("FROM: ${FILE}\nTO: ${TARGET}\n")
    endif()
    configure_file(${FILE} ${TARGET} )
endforeach()


if(DBUS_BUS_ENABLE_INOTIFY)
    set(DIR_WATCH_SOURCE dir-watch-inotify.c)
elseif(DBUS_BUS_ENABLE_KQUEUE)
    set(DIR_WATCH_SOURCE dir-watch-kqueue.c)
else(DBUS_BUS_ENABLE_INOTIFY)
    set(DIR_WATCH_SOURCE dir-watch-default.c)
endif()

set(BUS_SOURCES
    activation.c
    activation.h
    apparmor.c
    apparmor.h
    audit.c
    audit.h
    bus.c
    bus.h
    config-loader-expat.c
    config-parser.c
    config-parser.h
    config-parser-common.c
    config-parser-common.h
#    config-parser-trivial.c
    connection.c
    connection.h
    containers.c
    containers.h
    desktop-file.c
    desktop-file.h
    dir-watch.h
    dispatch.c
    dispatch.h
    driver.c
    driver.h
    expirelist.c
    expirelist.h
    policy.c
    policy.h
    selinux.h
    selinux.c
    services.c
    services.h
    signals.c
    signals.h
    test.c
    test.h
    utils.c
    utils.h
    ${DIR_WATCH_SOURCE}
)
if(DBUS_ENABLE_STATS)
    list(APPEND BUS_SOURCES
        stats.c
        stats.h
    )
endif()

include_directories(
    ${CMAKE_BINARY_DIR}
    ${EXPAT_INCLUDE_DIR}
)

set(DBUS_DAEMON_SOURCES
    main.c
)

if(WIN32)
    add_executable_version_info(DBUS_DAEMON_SOURCES "dbus-daemon")
endif()

add_library(dbus-daemon-internal STATIC ${BUS_SOURCES})
target_link_libraries(dbus-daemon-internal ${DBUS_INTERNAL_LIBRARIES} ${EXPAT_LIBRARIES})

add_executable(dbus-daemon ${DBUS_DAEMON_SOURCES})
target_link_libraries(dbus-daemon dbus-daemon-internal)
set_target_properties(dbus-daemon PROPERTIES OUTPUT_NAME ${DBUS_DAEMON_NAME})
set_target_properties(dbus-daemon PROPERTIES COMPILE_FLAGS ${DBUS_INTERNAL_CLIENT_DEFINITIONS})

install(TARGETS dbus-daemon ${INSTALL_TARGETS_DEFAULT_ARGS})
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/session.conf DESTINATION ${CMAKE_INSTALL_DATADIR}/dbus-1)
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/legacy-config/session.conf DESTINATION ${CMAKE_INSTALL_FULL_SYSCONFDIR}/dbus-1)
install(DIRECTORY DESTINATION ${CMAKE_INSTALL_DATADIR}/dbus-1/session.d)
install(DIRECTORY DESTINATION ${CMAKE_INSTALL_DATADIR}/dbus-1/services)

if(NOT WIN32)
    install(FILES ${CMAKE_CURRENT_BINARY_DIR}/system.conf DESTINATION ${CMAKE_INSTALL_DATADIR}/dbus-1)
    install(FILES ${CMAKE_CURRENT_BINARY_DIR}/legacy-config/system.conf DESTINATION ${CMAKE_INSTALL_FULL_SYSCONFDIR}/dbus-1)
    install(DIRECTORY DESTINATION ${CMAKE_INSTALL_DATADIR}/dbus-1/system.d)
    install(DIRECTORY DESTINATION ${CMAKE_INSTALL_DATADIR}/dbus-1/system-services)
    install(DIRECTORY DESTINATION ${CMAKE_INSTALL_FULL_LOCALSTATEDIR}/run/dbus)
endif()

if(DBUS_SERVICE)
    set(dbus_service_SOURCES
        bus-service-win.c
        # TODO: add additional files
        #    service-main.c
        #    ${BUS_SOURCES}
    )

    add_executable_version_info(dbus_service_SOURCES "dbus-service")
    add_executable(dbus-service ${dbus_service_SOURCES} )
    target_link_libraries(dbus-service ${DBUS_INTERNAL_LIBRARIES} ${EXPAT_LIBRARIES})
    set_target_properties(dbus-service PROPERTIES COMPILE_FLAGS ${DBUS_INTERNAL_CLIENT_DEFINITIONS})
    install(TARGETS dbus-service ${INSTALL_TARGETS_DEFAULT_ARGS})
endif()

set(LAUNCH_HELPER_SOURCES
    config-loader-expat.c
    config-parser-common.c
    config-parser-trivial.c
    desktop-file.c
    utils.c
)

if(NOT WIN32)
    # test-bus-system depends on this library
    add_library(launch-helper-internal STATIC ${LAUNCH_HELPER_SOURCES})
    target_link_libraries(launch-helper-internal ${DBUS_INTERNAL_LIBRARIES} ${EXPAT_LIBRARIES})

    if(ENABLE_TRADITIONAL_ACTIVATION)
        add_executable(dbus-daemon-launch-helper activation-helper.c activation-helper-bin.c )
        target_link_libraries(dbus-daemon-launch-helper launch-helper-internal)
        install(TARGETS dbus-daemon-launch-helper RUNTIME DESTINATION ${CMAKE_INSTALL_LIBEXECDIR})
        # It is intended not to check here that uid is 0 - see https://gitlab.freedesktop.org/dbus/dbus/-/merge_requests/154#note_478876
        install(CODE "message(\"-- Note: Not installing \$ENV{DESTDIR}\${CMAKE_INSTALL_PREFIX}/${CMAKE_INSTALL_LIBEXECDIR}/dbus-daemon-launch-helper binary setuid!\")")
        install(CODE "message(\"-- Note: You'll need to manually set permissions to 'root:${DBUS_USER}' and permissions '4750'\")")
    endif()
endif()

if(MSVC)
    project_source_group(${GROUP_CODE} bus_test_SOURCES dummy)
endif()

macro(install_example_in_file a)
    configure_file(${a}.in ${CMAKE_CURRENT_BINARY_DIR}/${a})
    install(FILES ${CMAKE_CURRENT_BINARY_DIR}/${a} DESTINATION ${CMAKE_INSTALL_DATADIR}/doc/dbus/examples)
endmacro()

macro(install_example_file a)
    install(FILES ${CMAKE_CURRENT_SOURCE_DIR}/${a} DESTINATION ${CMAKE_INSTALL_DATADIR}/doc/dbus/examples)
endmacro()

install_example_in_file(example-session-disable-stats.conf)
if(NOT WIN32)
    install_example_in_file(example-system-enable-stats.conf)
    install_example_file(example-system-hardening-without-traditional-activation.conf)
endif()

if(DBUS_BUS_ENABLE_SYSTEMD)
    configure_file(dbus.socket.in ${CMAKE_CURRENT_BINARY_DIR}/dbus.socket)
    configure_file(dbus.service.in ${CMAKE_CURRENT_BINARY_DIR}/dbus.service)
    add_systemd_service(${CMAKE_CURRENT_BINARY_DIR}/dbus.socket PATH ${DBUS_SYSTEMD_SYSTEMUNITDIR} LINKS sockets.target.wants)
    add_systemd_service(${CMAKE_CURRENT_BINARY_DIR}/dbus.service PATH ${DBUS_SYSTEMD_SYSTEMUNITDIR} LINKS multi-user.target.wants)
endif()

if(DBUS_ENABLE_USER_SESSION)
    configure_file(systemd-user/dbus.socket.in ${CMAKE_CURRENT_BINARY_DIR}/systemd-user/dbus.socket)
    configure_file(systemd-user/dbus.service.in ${CMAKE_CURRENT_BINARY_DIR}/systemd-user/dbus.service)
    add_systemd_service(${CMAKE_CURRENT_BINARY_DIR}/systemd-user/dbus.socket PATH ${DBUS_SYSTEMD_USERUNITDIR} LINKS sockets.target.wants)
    add_systemd_service(${CMAKE_CURRENT_BINARY_DIR}/systemd-user/dbus.service PATH ${DBUS_SYSTEMD_USERUNITDIR})
endif()

## mop up the gcov files
#clean-local:
#	/bin/rm *.bb *.bbg *.da *.gcov || true

#install-data-hook:
#	$(mkinstalldirs) $(DESTDIR)/$(localstatedir)/run/dbus
#	$(mkinstalldirs) $(DESTDIR)/$(configdir)/system.d
#	$(mkinstalldirs) $(DESTDIR)/$(datadir)/dbus-1/services

##install_file(${configdir}/system.d FILE

#### Init scripts fun
#SCRIPT_IN_FILES=messagebus.in
#		rc.messagebus.in

## Red Hat start
#if DBUS_INIT_SCRIPTS_RED_HAT

#initddir=$(sysconfdir)/rc.d/init.d

#initd_SCRIPTS= 	
#	messagebus

#endif
# ## Red Hat end

## Slackware start
#if DBUS_INIT_SCRIPTS_SLACKWARE

#initddir=$(sysconfdir)/rc.d/

#initd_SCRIPTS= 	
#	rc.messagebus

#endif
## Slackware end

#MAN_IN_FILES=dbus-daemon.1.in
#man_MANS = dbus-daemon.1

#### Extra dist 

#EXTRA_DIST=$(CONFIG_IN_FILES) $(SCRIPT_IN_FILES) $(man_MANS) $(MAN_IN_FILES)
