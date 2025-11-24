# - Check if the given struct or class has the specified member variable
# CHECK_STRUCT_MEMBER (STRUCT MEMBER HEADER VARIABLE)
#
#  STRUCT - the name of the struct or class you are interested in
#  MEMBER - the member which existence you want to check
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

MACRO (CHECK_STRUCT_MEMBER _STRUCT _MEMBER _HEADER _RESULT)
   SET(_INCLUDE_FILES)
   FOREACH (it ${_HEADER})
      SET(_INCLUDE_FILES "${_INCLUDE_FILES}#include <${it}>\n")
   ENDFOREACH (it)

   SET(_CHECK_STRUCT_MEMBER_SOURCE_CODE "
${_INCLUDE_FILES}
int main()
{
   ${_STRUCT}* tmp;
   tmp->${_MEMBER};
  return 0;
}
")
   CHECK_CXX_SOURCE_COMPILES("${_CHECK_STRUCT_MEMBER_SOURCE_CODE}" ${_RESULT})

ENDMACRO (CHECK_STRUCT_MEMBER)

