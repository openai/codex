# - Try to find the GLIB2 libraries
# Once done this will define
#
#  GLIB2_FOUND - system has glib2
#  GLIB2_INCLUDE_DIR - the glib2 include directory
#  GLIB2_LIBRARIES - glib2 library

# Copyright (c) 2008 Laurent Montel, <montel@kde.org>
# Copyright (c) 2013 Ralf Habacker, <ralf.habacker@freenet.de>
#
# Redistribution and use is allowed according to the terms of the BSD license.
# For details see the accompanying COPYING-CMAKE-SCRIPTS file.


if(GLIB2_INCLUDE_DIR AND GLIB2_LIBRARIES)
    # Already in cache, be silent
    set(GLIB2_FIND_QUIETLY TRUE)
endif(GLIB2_INCLUDE_DIR AND GLIB2_LIBRARIES)

if (NOT WIN32)
    find_package(PkgConfig)
    pkg_check_modules(PC_LibGLIB2 QUIET glib-2.0)
endif()

find_path(GLIB2_MAIN_INCLUDE_DIR
          NAMES glib.h
          HINTS ${PC_LibGLIB2_INCLUDEDIR}
          PATH_SUFFIXES glib-2.0)

find_library(GLIB2_LIBRARY
             NAMES glib-2.0
             HINTS ${PC_LibGLIB2_LIBDIR}
)

find_library(GOBJECT2_LIBRARY
             NAMES gobject-2.0
             HINTS ${PC_LibGLIB2_LIBDIR}
)

find_library(GIO2_LIBRARY
             NAMES gio-2.0
             HINTS ${PC_LibGLIB2_LIBDIR}
)

set(GLIB2_LIBRARIES ${GLIB2_LIBRARY} ${GOBJECT2_LIBRARY} ${GIO2_LIBRARY})

# search the glibconfig.h include dir under the same root where the library is found
get_filename_component(glib2LibDir "${GLIB2_LIBRARY}" PATH)

find_path(GLIB2_INTERNAL_INCLUDE_DIR glibconfig.h
          PATH_SUFFIXES glib-2.0/include
          HINTS ${PC_LibGLIB2_INCLUDEDIR} "${glib2LibDir}" ${CMAKE_SYSTEM_LIBRARY_PATH})

set(GLIB2_INCLUDE_DIR "${GLIB2_MAIN_INCLUDE_DIR}")

# not sure if this include dir is optional or required
# for now it is optional
if(GLIB2_INTERNAL_INCLUDE_DIR)
  set(GLIB2_INCLUDE_DIR ${GLIB2_INCLUDE_DIR} "${GLIB2_INTERNAL_INCLUDE_DIR}")
endif(GLIB2_INTERNAL_INCLUDE_DIR)

include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(GLIB2  DEFAULT_MSG  GLIB2_LIBRARIES GLIB2_MAIN_INCLUDE_DIR)

mark_as_advanced(GLIB2_INCLUDE_DIR GLIB2_LIBRARIES)
