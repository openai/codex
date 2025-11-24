#
# api documentation
#
find_package(Doxygen)

# always check doc target
add_custom_target(doc ALL)

if(DOXYGEN_EXECUTABLE)
    option(DBUS_ENABLE_DOXYGEN_DOCS "build DOXYGEN documentation (requires Doxygen)" ON)
    add_auto_option(ENABLE_QT_HELP "build qt help documentation (requires qhelpgenerator(-qt5)); set INSTALL_QCH_DIR for custom qch installation path" AUTO)
endif()

if(DBUS_ENABLE_DOXYGEN_DOCS)
    set(top_srcdir ${CMAKE_SOURCE_DIR})
    set(top_builddir ${CMAKE_BINARY_DIR})
    if(WIN32)
        set(DBUS_GENERATE_MAN NO)
    else()
        set(DBUS_GENERATE_MAN YES)
    endif()
    if(NOT DEFINED INSTALL_QCH_DIR)
        set(INSTALL_QCH_DIR ${CMAKE_INSTALL_DATADIR}/doc/dbus)
    endif()
    find_program(QHELPGENERATOR_EXECUTABLE NAMES qhelpgenerator qhelpgenerator-qt5)
    check_auto_option(ENABLE_QT_HELP "Qt help" QHELPGENERATOR_EXECUTABLE "qhelpgenerator")
    if(ENABLE_QT_HELP AND QHELPGENERATOR_EXECUTABLE)
        message(STATUS "${QHELPGENERATOR_EXECUTABLE} found")
        set(DOXYGEN_GENERATE_QHP YES)
        set(DOXYGEN_QHG_LOCATION ${QHELPGENERATOR_EXECUTABLE})
        set(DOXYGEN_QCH_FILE ${CMAKE_CURRENT_BINARY_DIR}/api/qch/dbus-${VERSION}.qch)
        set(DBUS_ENABLE_QTHELP_DOCS ON PARENT_SCOPE)
        install(FILES ${DOXYGEN_QCH_FILE} DESTINATION ${INSTALL_QCH_DIR})
    else()
        set(DOXYGEN_GENERATE_QHP NO)
        set(DBUS_ENABLE_QTHELP_DOCS OFF PARENT_SCOPE)
    endif()
    configure_file(../Doxyfile.in ${CMAKE_BINARY_DIR}/Doxyfile )
    file(GLOB dbus_files "${CMAKE_SOURCE_DIR}/dbus/*.[ch]*")
    add_custom_command(
        OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/doxygen.stamp
        DEPENDS ${CMAKE_SOURCE_DIR}/Doxyfile.in  ${CMAKE_BINARY_DIR}/Doxyfile ${dbus_files}
        COMMAND ${CMAKE_COMMAND} -E make_directory ${CMAKE_CURRENT_BINARY_DIR}/api/html
        COMMAND ${CMAKE_COMMAND} -E make_directory ${CMAKE_CURRENT_BINARY_DIR}/api/man
        COMMAND ${CMAKE_COMMAND} -E make_directory ${CMAKE_CURRENT_BINARY_DIR}/api/xml
        COMMAND ${DOXYGEN_EXECUTABLE} ${CMAKE_BINARY_DIR}/Doxyfile
        COMMAND ${CMAKE_COMMAND} -E touch ${CMAKE_CURRENT_BINARY_DIR}/doxygen.stamp
        WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
        COMMENT "Generating API documentation with Doxygen"
    )
    add_custom_target(apidoc
        DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/doxygen.stamp
    )
    add_dependencies(doc apidoc)
    install(DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/api/html/ DESTINATION ${CMAKE_INSTALL_DATADIR}/doc/dbus/api)
endif()

find_program(XSLTPROC_EXECUTABLE xsltproc)
if(XSLTPROC_EXECUTABLE)
    message(STATUS "${XSLTPROC_EXECUTABLE} found")
    set(DBUS_HAVE_XSLTPROC 1)
endif()

if(DBUS_HAVE_XSLTPROC AND DBUS_ENABLE_DOXYGEN_DOCS)
    add_custom_command(
        OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/dbus.devhelp2
        COMMAND ${XSLTPROC_EXECUTABLE}
            -o ${CMAKE_CURRENT_BINARY_DIR}/dbus.devhelp2
            ${CMAKE_CURRENT_SOURCE_DIR}/doxygen_to_devhelp.xsl
            ${CMAKE_CURRENT_BINARY_DIR}/api/xml/index.xml
        DEPENDS
            ${CMAKE_CURRENT_SOURCE_DIR}/doxygen_to_devhelp.xsl
            ${CMAKE_CURRENT_BINARY_DIR}/doxygen.stamp
    )
    add_custom_target(devhelp2
        DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/dbus.devhelp2
    )
    add_dependencies(doc devhelp2)
    install(FILES ${CMAKE_CURRENT_BINARY_DIR}/dbus.devhelp2 DESTINATION ${CMAKE_INSTALL_DATADIR}/doc/dbus)
endif()

#
# docbook based documentation
# 
find_package(DocBookXSL)

if(DOCBOOKXSL_DIR AND XSLTPROC_EXECUTABLE)
    option(DBUS_ENABLE_XML_DOCS "build XML documentation" ON)
    set(DBUS_XML_DOCS_ENABLED 1)
    message(STATUS "xsltproc docbook generator found")
    add_custom_target(xmldoc)
    add_dependencies(doc xmldoc)
endif()

if(DBUS_ENABLE_XML_DOCS)
#
# generate docbook file from template
#
# @param _infile docbook source template file
# @param _outfile generated docbook source file
#
macro(generate_docbook_file _infile _outfile)
    get_filename_component(outname ${_outfile} NAME)
    set(srcfile ${CMAKE_CURRENT_SOURCE_DIR}/${_infile})
    set(tmpfile ${CMAKE_CURRENT_BINARY_DIR}/${outname}.tmp.cmake)
    file(WRITE ${tmpfile} "
set(EXPANDED_SYSCONFDIR ${CMAKE_INSTALL_FULL_SYSCONFDIR})
set(EXPANDED_DATADIR ${CMAKE_INSTALL_FULL_DATADIR})
set(DBUS_VERSION ${DBUS_VERSION})
configure_file(${srcfile} ${_outfile})
    ")
    add_custom_command(OUTPUT ${_outfile}
        COMMAND ${CMAKE_COMMAND} -E remove ${_outfile}
        COMMAND ${CMAKE_COMMAND} -P ${tmpfile}
        DEPENDS ${srcfile} ${CMAKE_BINARY_DIR}/CMakeCache.txt
        COMMENT "Generating ${outname}"
    )
    add_custom_target(xmldoc-${outname} DEPENDS ${_outfile})
endmacro()

#
# generate docbook file from source
#
# @param _source           docbook xml source file
# @param FORMATS <formats> list with output formats to generate ('html' and/or 'man')
#
macro(DOCBOOK _source)
    set(options)
    set(oneValueArgs)
    set(multiValueArgs FORMATS)
    cmake_parse_arguments(ARGS "${options}" "${oneValueArgs}" "${multiValueArgs}" ${ARGN})

    get_filename_component(_infile ${_source} ABSOLUTE)
    get_filename_component(_name ${_infile} NAME)
    foreach(_format ${ARGS_FORMATS})
        if(${_format} STREQUAL "man")
            string(REPLACE ".xml" "" _outname ${_name})
            set(STYLESHEET "${DOCBOOKXSL_DIR}/manpages/docbook.xsl")
            set(INSTALL_DIR ${CMAKE_INSTALL_DATADIR}/man/man1)
        else()
            string(REPLACE ".xml" ".html" _outname ${_name})
            set(STYLESHEET "${DOCBOOKXSL_DIR}/html/docbook.xsl")
            set(INSTALL_DIR ${CMAKE_INSTALL_DATADIR}/doc/dbus)
        endif()
        set(_outfile ${CMAKE_CURRENT_BINARY_DIR}/${_outname})
        add_custom_command(
            OUTPUT ${_outfile}
            COMMAND ${XSLTPROC_EXECUTABLE} --output ${_outfile} --nonet --xinclude  --param passivetex.extensions '1' --param generate.consistent.ids '1' ${STYLESHEET} ${_infile}
            DEPENDS ${XSLTPROC_EXECUTABLE} ${_infile}
            WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
        )
        add_custom_target(xmldoc-${_outname} DEPENDS ${_outfile})
        add_dependencies(xmldoc xmldoc-${_outname})
        install(FILES ${_outfile} DESTINATION ${INSTALL_DIR})
    endforeach()
endmacro()

### copy tests to builddir so that generated tests and static tests 
### are all in one place.
### todo how to add more filetypes 
macro(COPYDIR _src _type)
    foreach(FILE_TYPE ${_type})
        foreach(DIR ${_src})
            file(GLOB FILES "${DIR}/${FILE_TYPE}" )
            file(MAKE_DIRECTORY ${CMAKE_BINARY_DIR}/${DIR})
            foreach(FILE ${FILES})
                get_filename_component(FILENAME ${FILE} NAME)
                set(TARGET ${CMAKE_BINARY_DIR}/${DIR}/${FILENAME})
                configure_file(${FILE} ${TARGET} COPYONLY)
                if(CONFIG_VERBOSE)
                    message("FROM: ${FILE}\nTO: ${TARGET}\n")
                endif()
            endforeach()
        endforeach()
    endforeach()
endmacro()

COPYDIR(doc *.png)
COPYDIR(doc *.svg)

generate_docbook_file(dbus-cleanup-sockets.1.xml.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-cleanup-sockets.1.xml)
generate_docbook_file(dbus-daemon.1.xml.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-daemon.1.xml)
generate_docbook_file(dbus-launch.1.xml.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-launch.1.xml)
generate_docbook_file(dbus-monitor.1.xml.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-monitor.1.xml)
generate_docbook_file(dbus-run-session.1.xml.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-run-session.1.xml)
generate_docbook_file(dbus-send.1.xml.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-send.1.xml)
generate_docbook_file(dbus-test-tool.1.xml.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-test-tool.1.xml)
generate_docbook_file(dbus-update-activation-environment.1.xml.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-update-activation-environment.1.xml)
generate_docbook_file(dbus-uuidgen.1.xml.in ${CMAKE_CURRENT_BINARY_DIR}/dbus-uuidgen.1.xml)

set(formats html)
if(UNIX)
    list(APPEND formats man)
endif()
docbook(${CMAKE_CURRENT_BINARY_DIR}/dbus-cleanup-sockets.1.xml FORMATS ${formats})
docbook(${CMAKE_CURRENT_BINARY_DIR}/dbus-daemon.1.xml FORMATS ${formats})
docbook(${CMAKE_CURRENT_BINARY_DIR}/dbus-launch.1.xml FORMATS ${formats})
docbook(${CMAKE_CURRENT_BINARY_DIR}/dbus-monitor.1.xml FORMATS ${formats})
docbook(${CMAKE_CURRENT_BINARY_DIR}/dbus-run-session.1.xml FORMATS ${formats})
docbook(${CMAKE_CURRENT_BINARY_DIR}/dbus-send.1.xml FORMATS ${formats})
docbook(${CMAKE_CURRENT_BINARY_DIR}/dbus-test-tool.1.xml FORMATS ${formats})
docbook(${CMAKE_CURRENT_BINARY_DIR}/dbus-uuidgen.1.xml FORMATS ${formats})
docbook(${CMAKE_CURRENT_BINARY_DIR}/dbus-update-activation-environment.1.xml FORMATS ${formats})

#
# handle html index file
#
configure_file(index.html.in ${CMAKE_CURRENT_BINARY_DIR}/index.html)
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/index.html DESTINATION ${CMAKE_INSTALL_DATADIR}/doc/dbus)

#
# misc files
#
set(DTD_DATA
    diagram.png
    diagram.svg
)

install(FILES ${DTD_DATA} DESTINATION ${CMAKE_INSTALL_DATADIR}/doc/dbus)

set(DOC_DATA
    system-activation.txt
)

install(FILES ${DOC_DATA} DESTINATION ${CMAKE_INSTALL_DATADIR}/doc/dbus)

set(DBUS_DTD_DIR "${CMAKE_INSTALL_DATADIR}/xml/dbus-1" CACHE STRING  "Directory for installing DTD files")
set(DBUS_XML_CATALOG_DIR "${DBUS_DTD_DIR}" CACHE STRING  "Directory for installing XML catalog file")

set(EXTRA_DIST
    busconfig.dtd
    introspect.dtd
    introspect.xsl
)

install(FILES ${EXTRA_DIST} DESTINATION ${DBUS_DTD_DIR})

configure_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/catalog.xml.in"
  "${CMAKE_CURRENT_BINARY_DIR}/catalog.xml"
  @ONLY)

install(FILES "${CMAKE_CURRENT_BINARY_DIR}/catalog.xml" DESTINATION ${DBUS_XML_CATALOG_DIR})

endif()
