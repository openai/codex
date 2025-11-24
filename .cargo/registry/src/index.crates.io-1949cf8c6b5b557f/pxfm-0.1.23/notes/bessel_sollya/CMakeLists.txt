cmake_minimum_required(VERSION 3.31)
project(bessel_sollya)

set(CMAKE_CXX_STANDARD 20)

add_library(bessel_sollya SHARED library.c)

find_package(PkgConfig REQUIRED)
pkg_check_modules(MPFR REQUIRED IMPORTED_TARGET mpfr)
pkg_check_modules(MPFI REQUIRED IMPORTED_TARGET mpfi)
include_directories(${MPFR_INCLUDE_DIRS})
link_directories(${MPFR_LIBRARY_DIRS})
add_definitions(${MPFR_CFLAGS_OTHER})

include_directories(/opt/homebrew/include)
link_directories(/opt/homebrew/lib)
target_link_libraries(bessel_sollya PRIVATE PkgConfig::MPFR PkgConfig::MPFI)