# A project that uses the library as its users do: through find_package(iris)
# against an installed copy, or through FetchContent of the source tree that
# IRIS_SOURCE_DIR names or of the release archive that IRIS_URL names, checked
# against IRIS_URL_SHA256.
cmake_minimum_required(VERSION 3.28)
project(iris-consumer LANGUAGES CXX)

# The consumer uses no C++ modules either, so it needs no dependency
# scanner, such as clang-scan-deps for Clang.
set(CMAKE_CXX_SCAN_FOR_MODULES OFF)

set(IRIS_SOURCE_DIR "" CACHE PATH "Fetch the library from this source tree instead of finding it")
set(IRIS_URL "" CACHE STRING "Fetch the library from this release archive instead of finding it")
set(IRIS_URL_SHA256 "" CACHE STRING "The SHA-256 checksum of the archive that IRIS_URL names")

if(IRIS_URL)
  include(FetchContent)
  FetchContent_Declare(iris URL "${IRIS_URL}" URL_HASH "SHA256=${IRIS_URL_SHA256}")
  FetchContent_MakeAvailable(iris)
elseif(IRIS_SOURCE_DIR)
  include(FetchContent)
  FetchContent_Declare(iris SOURCE_DIR "${IRIS_SOURCE_DIR}")
  FetchContent_MakeAvailable(iris)
else()
  find_package(iris 0.1 REQUIRED CONFIG)
endif()

add_executable(consumer main.cpp)
target_link_libraries(consumer PRIVATE iris::iris)
