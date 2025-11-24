
macro(MACRO_GETENV_WIN_PATH var name)
    set(${var} $ENV{${name}})
    string(REGEX REPLACE "\\\\" "/" ${var} "${${var}}")
endmacro()
