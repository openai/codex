# folders in the msvc projects
# mode==flat  : headers and ourses in no folders
# mode==split : standard behavior of cmake, split headers and sources
# mode== <other values" : code is in this folder
macro(project_source_group mode sources headers)
    #message(STATUS ${mode})
    #message(STATUS ${sources} ${headers})
    if(${mode} MATCHES "flat")
        source_group("Source Files" Files)
        source_group("Header Files" Files)
        source_group("cmake" FILES CMakeLists.txt)
    else(${mode} MATCHES "flat")
        if(NOT ${mode} MATCHES "split")
            source_group("${mode}" FILES ${${sources}} ${${headers}})
        endif()
    endif()
endmacro()

