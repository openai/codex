# - Check if the prototype for a function exists.
# CHECK_PROTOTYPE_EXISTS (FUNCTION HEADER VARIABLE)
#
#  FUNCTION - the name of the function you are looking for
#  HEADER - the header(s) where the prototype should be declared
#  VARIABLE - variable to store the result
#
# The following variables may be set before calling this macro to
# modify the way the check is run:
#
#  CMAKE_REQUIRED_FLAGS = string of compile command line flags
#  CMAKE_REQUIRED_DEFINITIONS = list of macros to define (-DFOO=bar)
#  CMAKE_REQUIRED_INCLUDES = list of include directories

INCLUDE(CheckCXXSourceCompiles)

MACRO (CHECK_PROTOTYPE_EXISTS _SYMBOL _HEADER _RESULT)
   SET(_INCLUDE_FILES)
   FOREACH (it ${_HEADER})
      SET(_INCLUDE_FILES "${_INCLUDE_FILES}#include <${it}>\n")
   ENDFOREACH (it)

   SET(_CHECK_PROTO_EXISTS_SOURCE_CODE "
${_INCLUDE_FILES}
int main()
{
#ifndef ${_SYMBOL}
   int i = sizeof(&${_SYMBOL});
#endif
  return 0;
}
")
   CHECK_CXX_SOURCE_COMPILES("${_CHECK_PROTO_EXISTS_SOURCE_CODE}" ${_RESULT})
ENDMACRO (CHECK_PROTOTYPE_EXISTS _SYMBOL _HEADER _RESULT)

