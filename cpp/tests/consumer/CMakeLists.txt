# A project that uses the library as its users do: through find_package(iris)
# against an installed copy, or, with IRIS_SOURCE_DIR set, through
# FetchContent of that source tree.
cmake_minimum_required(VERSION 3.28)
project(iris-consumer LANGUAGES CXX)

# The consumer uses no C++ modules either, so it needs no dependency
# scanner, such as clang-scan-deps for Clang.
set(CMAKE_CXX_SCAN_FOR_MODULES OFF)

set(IRIS_SOURCE_DIR "" CACHE PATH "Fetch the library from this source tree instead of finding it")

if(IRIS_SOURCE_DIR)
  include(FetchContent)
  FetchContent_Declare(iris SOURCE_DIR "${IRIS_SOURCE_DIR}")
  FetchContent_MakeAvailable(iris)
else()
  find_package(iris 0.1 REQUIRED CONFIG)
endif()

add_executable(consumer main.cpp)
target_link_libraries(consumer PRIVATE iris::iris)
