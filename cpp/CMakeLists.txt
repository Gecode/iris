cmake_minimum_required(VERSION 3.28)
project(iris VERSION 0.1.0 LANGUAGES CXX)

# The library uses no C++ modules, so skip the dependency scan for them.
set(CMAKE_CXX_SCAN_FOR_MODULES OFF)

option(IRIS_BUILD_TESTS "Build the conformance driver and the tests" ${PROJECT_IS_TOP_LEVEL})
if(EXISTS "${PROJECT_SOURCE_DIR}/../conformance/cases")
  set(iris_has_conformance ON)
else()
  set(iris_has_conformance OFF)
endif()
option(IRIS_CONFORMANCE_TESTS "Run the conformance suite through the driver (needs cargo)"
  ${iris_has_conformance})

# Warnings for targets of this project only. CMAKE_COMPILE_WARNING_AS_ERROR
# makes them errors; the dev preset sets it.
function(iris_set_warnings target)
  if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    target_compile_options(${target} PRIVATE
      -Wall -Wextra -Wpedantic -Wconversion -Wsign-conversion -Wshadow
      -Wold-style-cast -Wnon-virtual-dtor -Woverloaded-virtual
      -Wimplicit-fallthrough)
  elseif(MSVC)
    target_compile_options(${target} PRIVATE /W4 /permissive- /utf-8 /Zc:__cplusplus)
  endif()
endfunction()

add_library(iris)
add_library(iris::iris ALIAS iris)
target_sources(iris
  PRIVATE
    src/data.cpp
    src/dzn.cpp
    src/error.cpp
    src/eval.cpp
    src/lexer.cpp
    src/numbers.cpp
    src/parser.cpp
    src/reader.cpp
    src/resolve.cpp
    src/spec.cpp
    src/text.cpp
  PUBLIC
    FILE_SET HEADERS
    BASE_DIRS include
    FILES
      include/iris/data.hpp
      include/iris/error.hpp
      include/iris/iris.hpp
      include/iris/params.hpp
      include/iris/spec.hpp)
target_compile_features(iris PUBLIC cxx_std_23)
set_target_properties(iris PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris)

if(IRIS_BUILD_TESTS)
  add_executable(iris-driver driver/main.cpp driver/json.cpp)
  target_link_libraries(iris-driver PRIVATE iris::iris)
  set_target_properties(iris-driver PROPERTIES CXX_EXTENSIONS OFF)
  iris_set_warnings(iris-driver)
  enable_testing()
  add_subdirectory(tests)
endif()
