add_executable(iris-spec-test spec_test.cpp)
target_link_libraries(iris-spec-test PRIVATE iris::iris)
set_target_properties(iris-spec-test PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris-spec-test)
add_test(NAME spec-api COMMAND iris-spec-test)

# The data test also checks the driver's JSON of data, so it builds that
# too, and it renders on a thread of its own.
find_package(Threads REQUIRED)
add_executable(iris-data-test data_test.cpp ../driver/json.cpp)
target_include_directories(iris-data-test PRIVATE "${PROJECT_SOURCE_DIR}/driver")
target_link_libraries(iris-data-test PRIVATE iris::iris Threads::Threads)
set_target_properties(iris-data-test PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris-data-test)
add_test(NAME data-api COMMAND iris-data-test)

# The budget is internal to the library, so the test includes its header.
add_executable(iris-budget-test budget_test.cpp)
target_include_directories(iris-budget-test PRIVATE "${PROJECT_SOURCE_DIR}/src")
set_target_properties(iris-budget-test PROPERTIES CXX_EXTENSIONS OFF)
target_compile_features(iris-budget-test PRIVATE cxx_std_23)
iris_set_warnings(iris-budget-test)
add_test(NAME budget COMMAND iris-budget-test)

# The allocation test replaces the global operator new, so it links the
# library alone.
add_executable(iris-memory-test memory_test.cpp)
target_link_libraries(iris-memory-test PRIVATE iris::iris Threads::Threads)
set_target_properties(iris-memory-test PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris-memory-test)
add_test(NAME memory COMMAND iris-memory-test)
set_tests_properties(memory PROPERTIES TIMEOUT 120)

# Each public header compiles alone, and no public header names Gecode.
get_target_property(iris_headers iris HEADER_SET)
set(iris_header_sources)
foreach(header IN LISTS iris_headers)
  cmake_path(GET header FILENAME name)
  set(source "${CMAKE_CURRENT_BINARY_DIR}/headers/${name}.cpp")
  file(CONFIGURE OUTPUT "${source}" CONTENT "#include <iris/${name}>\n")
  list(APPEND iris_header_sources "${source}")
endforeach()
add_library(iris-headers OBJECT EXCLUDE_FROM_ALL ${iris_header_sources})
target_link_libraries(iris-headers PRIVATE iris::iris)
set_target_properties(iris-headers PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris-headers)
add_test(NAME headers-compile-alone
  COMMAND "${CMAKE_COMMAND}" --build "${PROJECT_BINARY_DIR}" --target iris-headers
    --config $<CONFIG>)
add_test(NAME headers-name-no-gecode
  COMMAND "${CMAKE_COMMAND}" "-DHEADERS=${iris_headers}"
    -P "${CMAKE_CURRENT_SOURCE_DIR}/no-gecode.cmake")

# The consumer project builds and runs against an installed copy, through
# find_package(iris), and through FetchContent of this source tree. Both
# build with this build's compiler and flags.
if(IRIS_INSTALL)
  set(consumer_dir "${CMAKE_CURRENT_BINARY_DIR}/consumer")
  set(consumer_options
    "-DCMAKE_CXX_COMPILER=${CMAKE_CXX_COMPILER}"
    "-DCMAKE_BUILD_TYPE=$<CONFIG>"
    "-DCMAKE_CXX_FLAGS=${CMAKE_CXX_FLAGS}"
    "-DCMAKE_EXE_LINKER_FLAGS=${CMAKE_EXE_LINKER_FLAGS}"
    -DCMAKE_COMPILE_WARNING_AS_ERROR=ON)
  if(CMAKE_OSX_SYSROOT)
    list(APPEND consumer_options "-DCMAKE_OSX_SYSROOT=${CMAKE_OSX_SYSROOT}")
  endif()
  add_test(NAME consumer-install
    COMMAND "${CMAKE_COMMAND}" --install "${PROJECT_BINARY_DIR}"
      --prefix "${consumer_dir}/prefix" --config $<CONFIG>)
  set_tests_properties(consumer-install PROPERTIES FIXTURES_SETUP iris-installed)
  add_test(NAME consumer-find-package
    COMMAND "${CMAKE_CTEST_COMMAND}"
      --build-and-test "${CMAKE_CURRENT_SOURCE_DIR}/consumer" "${consumer_dir}/find-package"
      --build-generator "${CMAKE_GENERATOR}"
      --build-makeprogram "${CMAKE_MAKE_PROGRAM}"
      --build-options ${consumer_options} "-DCMAKE_PREFIX_PATH=${consumer_dir}/prefix"
      --test-command consumer)
  set_tests_properties(consumer-find-package PROPERTIES FIXTURES_REQUIRED iris-installed)
  add_test(NAME consumer-fetch-content
    COMMAND "${CMAKE_CTEST_COMMAND}"
      --build-and-test "${CMAKE_CURRENT_SOURCE_DIR}/consumer" "${consumer_dir}/fetch-content"
      --build-generator "${CMAKE_GENERATOR}"
      --build-makeprogram "${CMAKE_MAKE_PROGRAM}"
      --build-options ${consumer_options} "-DIRIS_SOURCE_DIR=${PROJECT_SOURCE_DIR}"
      --test-command consumer)
endif()

# The examples of the site's C++ guide, which read files under formats/. Each
# must print its recorded output.
if(iris_has_conformance)
  file(GLOB example_sources CONFIGURE_DEPENDS "${PROJECT_SOURCE_DIR}/examples/*.cpp")
  foreach(source IN LISTS example_sources)
    cmake_path(GET source STEM example)
    add_executable(iris-example-${example} "${source}")
    target_link_libraries(iris-example-${example} PRIVATE iris::iris)
    set_target_properties(iris-example-${example} PROPERTIES CXX_EXTENSIONS OFF)
    iris_set_warnings(iris-example-${example})
    add_test(NAME example-${example}
      COMMAND "${CMAKE_COMMAND}"
        "-DEXAMPLE=$<TARGET_FILE:iris-example-${example}>"
        "-DRECORDING=${PROJECT_SOURCE_DIR}/examples/${example}.out"
        "-DREPOSITORY=${PROJECT_SOURCE_DIR}/.."
        -P "${CMAKE_CURRENT_SOURCE_DIR}/example.cmake")
  endforeach()
endif()

if(IRIS_CONFORMANCE_TESTS)
  find_program(IRIS_CARGO cargo)
  if(NOT IRIS_CARGO)
    message(FATAL_ERROR
      "IRIS_CONFORMANCE_TESTS needs cargo to run the conformance runner; "
      "install Rust or configure with -DIRIS_CONFORMANCE_TESTS=OFF")
  endif()
  # The runner's limit for one case. Sanitized drivers run slower, so
  # their presets raise it.
  set(IRIS_CONFORMANCE_CASE_TIMEOUT 30 CACHE STRING
    "Seconds the conformance runner allows the driver for one case")
  foreach(operation IN ITEMS parse read catalog dzn)
    add_test(NAME ${operation}-conformance
      COMMAND "${CMAKE_COMMAND}"
        "-DDRIVER=$<TARGET_FILE:iris-driver>"
        "-DREPOSITORY=${PROJECT_SOURCE_DIR}/.."
        "-DCARGO=${IRIS_CARGO}"
        -DOPERATION=${operation}
        "-DCASE_TIMEOUT=${IRIS_CONFORMANCE_CASE_TIMEOUT}"
        -P "${CMAKE_CURRENT_SOURCE_DIR}/conformance.cmake")
    # Whichever test runs first builds the runner with cargo. The whole
    # test may take as long as 60 cases at the limit, 30 minutes by default.
    math(EXPR timeout "60 * ${IRIS_CONFORMANCE_CASE_TIMEOUT}")
    set_tests_properties(${operation}-conformance PROPERTIES TIMEOUT ${timeout})
  endforeach()
endif()
