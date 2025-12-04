cmake_minimum_required(VERSION 3.12.0)

file(READ ../gradle.properties GRADLE_PROPERTIES)
string(REGEX MATCH "project[.]version=([0-9.]+)" _ ${GRADLE_PROPERTIES})

project(ktreesitter VERSION ${CMAKE_MATCH_1} LANGUAGES C)

find_package(JNI REQUIRED)

set(CMAKE_C_STANDARD 11)

if(MSVC)
    add_compile_options(/W3 /wd4244)
else()
    set(CMAKE_C_VISIBILITY_PRESET hidden)
    add_compile_options(-Wall -Wextra
                        -Wno-unused-parameter
                        -Wno-cast-function-type
                        -Werror=incompatible-pointer-types
                        -Werror=implicit-function-declaration)
endif()

include_directories(${JNI_INCLUDE_DIRS}
                    ../tree-sitter/lib/src
                    ../tree-sitter/lib/include)

add_compile_definitions(TREE_SITTER_HIDE_SYMBOLS _DEFAULT_SOURCE _POSIX_C_SOURCE=200112L)

add_library(ktreesitter SHARED
            ./src/jni/language.c
            ./src/jni/lookahead_iterator.c
            ./src/jni/node.c
            ./src/jni/parser.c
            ./src/jni/query.c
            ./src/jni/query_cursor.c
            ./src/jni/tree.c
            ./src/jni/tree_cursor.c
            ./src/jni/module.c
            ../tree-sitter/lib/src/lib.c)

set_target_properties(ktreesitter PROPERTIES DEFINE_SYMBOL "")

install(TARGETS ktreesitter ARCHIVE EXCLUDE_FROM_ALL)
