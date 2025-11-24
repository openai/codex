#
# win32 macros 
# 
# Copyright (c) 2006-2007, Ralf Habacker
#
# Redistribution and use is allowed according to the terms of the BSD license.
#

if(WIN32)
    #
    # addExplorerWrapper creates batch files for fast access 
    # to the build environment from the win32 explorer. 
    # 
    # For mingw and nmake projects it's opens a command shell,
    # for Visual Studio IDE's (at least tested with VS 8 2005) it
    # opens the related .sln file with paths setting specified at 
    # configure time. 
    #
    macro(addExplorerWrapper _projectname)
        # write explorer wrappers
        get_filename_component(CMAKE_BIN_PATH ${CMAKE_COMMAND} PATH)
        set(ADD_PATH "${CMAKE_BIN_PATH}")

        if(QT_QMAKE_EXECUTABLE)
            get_filename_component(QT_BIN_PATH ${QT_QMAKE_EXECUTABLE} PATH)
            set(ADD_PATH "${ADD_PATH};${QT_BIN_PATH}")
        endif()

        # add here more pathes 

        if(MINGW)
            get_filename_component(MINGW_BIN_PATH ${CMAKE_CXX_COMPILER} PATH)
            set(ADD_PATH "${ADD_PATH};${MINGW_BIN_PATH}")
            write_file(${CMAKE_BINARY_DIR}/${_projectname}-shell.bat "set PATH=${ADD_PATH};%PATH%\ncmd.exe")
        else(MINGW)
            if(CMAKE_BUILD_TOOL STREQUAL  "nmake")
                get_filename_component(VC_BIN_PATH ${CMAKE_CXX_COMPILER} PATH)
                write_file(${CMAKE_BINARY_DIR}/${_projectname}-shell.bat "set PATH=${ADD_PATH};%PATH%\ncall \"${VC_BIN_PATH}\\vcvars32.bat\"\ncmd.exe")
            else(CMAKE_BUILD_TOOL STREQUAL  "nmake")
                write_file(${CMAKE_BINARY_DIR}/${_projectname}-sln.bat "set PATH=${ADD_PATH};%PATH%\nstart ${_projectname}.sln")
            endif()
        endif()
    endmacro()
endif()
