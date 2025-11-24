# - Try to find LibIconv
# Once done this will define
#
#  LIBICONV_FOUND - system has LibIconv
#  LIBICONV_INCLUDE_DIR - the LibIconv include directory
#  LIBICONV_LIBRARIES - the libraries needed to use LibIconv
#  LIBICONV_DEFINITIONS - Compiler switches required for using LibIconv

if (LIBICONV_INCLUDE_DIR AND LIBICONV_LIBRARIES)

    # in cache already
    SET(LIBICONV_FOUND TRUE)

else (LIBICONV_INCLUDE_DIR AND LIBICONV_LIBRARIES)

    IF (NOT WIN32)
        MESSAGE(FATAL_ERROR "Please set this to the correct values!")
        # use pkg-config to get the directories and then use these values
        # in the FIND_PATH() and FIND_LIBRARY() calls
        INCLUDE(UsePkgConfig)
        PKGCONFIG(libiconv-1.9 _LibIconvIncDir _LibIconvLinkDir _LibIconvLinkFlags _LiIconvCflags)
        SET(LIBICONV_DEFINITIONS ${_LibIconvCflags})
    ENDIF (NOT WIN32)

    FIND_PATH(LIBICONV_INCLUDE_DIR iconv.h
      PATHS
     ${_LibIconvIncDir}
      PATH_SUFFIXES libiconv
    )

    FIND_LIBRARY(LIBICONV_LIBRARIES NAMES iconv libiconv
      PATHS
      ${_LibIconvLinkDir}
    )

    if (LIBICONV_INCLUDE_DIR AND LIBICONV_LIBRARIES)
       set(LIBICONV_FOUND TRUE)
    endif (LIBICONV_INCLUDE_DIR AND LIBICONV_LIBRARIES)

    if (LIBICONV_FOUND)
      if (NOT LibIconv_FIND_QUIETLY)
        message(STATUS "Found LibIconv: ${LIBICONV_LIBRARIES}")
      endif (NOT LibIconv_FIND_QUIETLY)
    else (LIBICONV_FOUND)
      if (LibIconv_FIND_REQUIRED)
        message(SEND_ERROR "Could NOT find LibIconv")
      endif (LibIconv_FIND_REQUIRED)
    endif (LIBICONV_FOUND)

    MARK_AS_ADVANCED(LIBICONV_INCLUDE_DIR LIBICONV_LIBRARIES)

endif (LIBICONV_INCLUDE_DIR AND LIBICONV_LIBRARIES)
