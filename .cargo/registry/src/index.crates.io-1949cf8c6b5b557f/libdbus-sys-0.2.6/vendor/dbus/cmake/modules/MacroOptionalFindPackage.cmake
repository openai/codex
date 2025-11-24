# - MACRO_OPTIONAL_FIND_PACKAGE() combines FIND_PACKAGE() with an OPTION()
# MACRO_OPTIONAL_FIND_PACKAGE( <name> [QUIT] )
# This macro is a combination of OPTION() and FIND_PACKAGE(), it
# works like FIND_PACKAGE(), but additionally it automatically creates
# an option name WITH_<name>, which can be disabled via the cmake GUI.
# or via -DWITH_<name>=OFF
# The standard <name>_FOUND variables can be used in the same way
# as when using the normal FIND_PACKAGE()

MACRO (MACRO_OPTIONAL_FIND_PACKAGE _name )
   OPTION(WITH_${_name} "Search for ${_name} package" ON)
   if (WITH_${_name})
      FIND_PACKAGE(${_name} ${ARGN})
   else (WITH_${_name})
      set(${_name}_FOUND)
      set(${_name}_INCLUDE_DIR)
      set(${_name}_INCLUDES)
      set(${_name}_LIBRARY)
      set(${_name}_LIBRARIES)
   endif (WITH_${_name})
ENDMACRO (MACRO_OPTIONAL_FIND_PACKAGE)

