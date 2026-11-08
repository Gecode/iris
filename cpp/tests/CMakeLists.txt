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
set_tests_properties(headers-compile-alone PROPERTIES RESOURCE_LOCK iris-build-tree)
add_test(NAME headers-name-no-gecode
  COMMAND "${CMAKE_COMMAND}" "-DHEADERS=${iris_headers}"
    -P "${CMAKE_CURRENT_SOURCE_DIR}/no-gecode.cmake")

# The spec builder: its checks and their call sites, text and typed reads,
# and the types it gives, which the second test checks at compile time.
add_executable(iris-build-test build_test.cpp)
target_link_libraries(iris-build-test PRIVATE iris::iris Threads::Threads)
set_target_properties(iris-build-test PROPERTIES CXX_EXTENSIONS OFF)
target_compile_definitions(iris-build-test PRIVATE
  "IRIS_REPOSITORY=\"${PROJECT_SOURCE_DIR}/..\"")
iris_set_warnings(iris-build-test)
add_test(NAME build-api COMMAND iris-build-test)
# The call site of every statement and of the errors of a built spec. The
# table of call sites is internal, so the test includes the library's
# headers.
add_executable(iris-build-call-sites-test build_call_sites_test.cpp)
target_include_directories(iris-build-call-sites-test PRIVATE
  "${PROJECT_SOURCE_DIR}/src")
target_link_libraries(iris-build-call-sites-test PRIVATE iris::iris)
set_target_properties(iris-build-call-sites-test PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris-build-call-sites-test)
add_test(NAME build-call-sites COMMAND iris-build-call-sites-test)
add_executable(iris-build-types-test build_types_test.cpp)
target_link_libraries(iris-build-types-test PRIVATE iris::iris)
set_target_properties(iris-build-types-test PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris-build-types-test)
add_test(NAME build-types COMMAND iris-build-types-test)

# Programs the builder rejects at compile time. Each file compiles as it is,
# and fails with IRIS_EXPECT_FAILURE defined, with a diagnostic that matches
# its pattern: either test builds one object target in this build tree.
# In a UTF-8 locale GCC quotes names with multibyte quotes, so a pattern
# matches a quote with `.*`, never with a single `.`.
set(iris_compile_failures
  "set_wrong_type|no matching.*set|C2672"
  "operator_wrong_type|invalid operands|no match for .*operator|C267[0-9]"
  "get_wrong_type|conversion|cannot convert|C2440"
  "set_wrong_depth|no matching.*set|C2672"
  "param_wrong_type|no matching.*set|C2672|C2664"
  "get_wrong_depth|conversion|cannot convert|C2440"
  "pattern_too_few|pattern_placeholders_do_not_match_its_arguments"
  "pattern_too_many|pattern_placeholders_do_not_match_its_arguments"
  "pattern_literal_placeholder|pattern_placeholders_do_not_match_its_arguments"
  "pattern_unpaired_brace|pattern_has_an_unpaired_brace"
  "pattern_rest_of_line|pattern_spells_rest_of_line_as_text"
  "pow_using_directive|is ambiguous|C2872")
foreach(entry IN LISTS iris_compile_failures)
  string(FIND "${entry}" "|" bar)
  string(SUBSTRING "${entry}" 0 ${bar} case)
  math(EXPR start "${bar} + 1")
  string(SUBSTRING "${entry}" ${start} -1 diagnostic)
  set(source "${CMAKE_CURRENT_SOURCE_DIR}/compile_fail/${case}.cpp")
  add_library(iris-compiles-${case} OBJECT EXCLUDE_FROM_ALL "${source}")
  add_library(iris-compile-fails-${case} OBJECT EXCLUDE_FROM_ALL "${source}")
  target_compile_definitions(iris-compile-fails-${case} PRIVATE IRIS_EXPECT_FAILURE)
  # clang-tidy reads the file as it compiles, without the failure.
  set_target_properties(iris-compile-fails-${case} PROPERTIES
    EXPORT_COMPILE_COMMANDS OFF)
  foreach(target IN ITEMS iris-compiles-${case} iris-compile-fails-${case})
    target_link_libraries(${target} PRIVATE iris::iris)
    set_target_properties(${target} PROPERTIES CXX_EXTENSIONS OFF)
    iris_set_warnings(${target})
  endforeach()
  add_test(NAME compiles-${case}
    COMMAND "${CMAKE_COMMAND}" --build "${PROJECT_BINARY_DIR}"
      --target iris-compiles-${case} --config $<CONFIG>)
  add_test(NAME compile-fails-${case}
    COMMAND "${CMAKE_COMMAND}" --build "${PROJECT_BINARY_DIR}"
      --target iris-compile-fails-${case} --config $<CONFIG>)
  set_tests_properties(compile-fails-${case} PROPERTIES
    PASS_REGULAR_EXPRESSION "${diagnostic}")
  set_tests_properties(compiles-${case} compile-fails-${case} PROPERTIES
    RESOURCE_LOCK iris-build-tree)
endforeach()

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

# A driver that answers with the spec of the format's builder in examples/
# in place of the case's own, for the format-builders test below. It builds
# without that test too, so that clang-tidy finds it in the compile database.
add_executable(iris-format-builder-driver format_builder_driver.cpp
  ../driver/driver.cpp ../driver/json.cpp)
target_include_directories(iris-format-builder-driver PRIVATE
  "${PROJECT_SOURCE_DIR}/driver" "${PROJECT_SOURCE_DIR}/examples")
target_link_libraries(iris-format-builder-driver PRIVATE iris::iris)
set_target_properties(iris-format-builder-driver PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris-format-builder-driver)

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

  # Every case of every format through the format's builder in examples/.
  add_test(NAME format-builders
    COMMAND "${CMAKE_COMMAND}"
      "-DDRIVER=$<TARGET_FILE:iris-format-builder-driver>"
      "-DREPOSITORY=${PROJECT_SOURCE_DIR}/.."
      "-DCARGO=${IRIS_CARGO}"
      "-DCASE_TIMEOUT=${IRIS_CONFORMANCE_CASE_TIMEOUT}"
      -P "${CMAKE_CURRENT_SOURCE_DIR}/format-builders.cmake")
  math(EXPR timeout "60 * ${IRIS_CONFORMANCE_CASE_TIMEOUT}")
  set_tests_properties(format-builders PROPERTIES TIMEOUT ${timeout})

  # Tests of rust/ that generate C++ and build it with this build's
  # compiler: the builder round trip, every accepted corpus spec through a
  # generated builder program, and the generated views of every format. Both
  # check the generated code with clang-format 22.1.8 and clang-tidy 22.1.8
  # when found; the round trip needs that clang-format and is skipped
  # without it.
  find_program(IRIS_CLANG_FORMAT clang-format)
  find_program(IRIS_CLANG_TIDY clang-tidy)
  # The programs build with this build's compile and link flags, those of
  # its configuration included, so that a sanitizer preset builds them
  # sanitized.
  set(program_cxx_flags "${CMAKE_CXX_FLAGS}")
  set(program_linker_flags "${CMAKE_EXE_LINKER_FLAGS}")
  get_property(multi_config GLOBAL PROPERTY GENERATOR_IS_MULTI_CONFIG)
  if(multi_config)
    foreach(config IN LISTS CMAKE_CONFIGURATION_TYPES)
      string(TOUPPER "${config}" upper)
      string(APPEND program_cxx_flags
        "$<$<CONFIG:${config}>: ${CMAKE_CXX_FLAGS_${upper}}>")
      string(APPEND program_linker_flags
        "$<$<CONFIG:${config}>: ${CMAKE_EXE_LINKER_FLAGS_${upper}}>")
    endforeach()
  elseif(CMAKE_BUILD_TYPE)
    string(TOUPPER "${CMAKE_BUILD_TYPE}" upper)
    string(APPEND program_cxx_flags " ${CMAKE_CXX_FLAGS_${upper}}")
    string(APPEND program_linker_flags
      " ${CMAKE_EXE_LINKER_FLAGS_${upper}}")
  endif()
  string(STRIP "${program_cxx_flags}" program_cxx_flags)
  string(STRIP "${program_linker_flags}" program_linker_flags)
  foreach(test IN ITEMS builder-round-trip generated-views)
    if(test STREQUAL "builder-round-trip")
      set(rust_test builder_round_trip_cpp)
      set(needs_clang_format ON)
    else()
      set(rust_test cpp_views)
      set(needs_clang_format OFF)
    endif()
    add_test(NAME ${test}
      COMMAND "${CMAKE_COMMAND}"
        -DTEST=${rust_test}
        -DNEEDS_CLANG_FORMAT=${needs_clang_format}
        "-DREPOSITORY=${PROJECT_SOURCE_DIR}/.."
        "-DCARGO=${IRIS_CARGO}"
        "-DCXX=${CMAKE_CXX_COMPILER}"
        "-DCXX_FLAGS=${program_cxx_flags}"
        "-DLINKER_FLAGS=${program_linker_flags}"
        "-DCMAKE=${CMAKE_COMMAND}"
        "-DGENERATOR=${CMAKE_GENERATOR}"
        "-DCLANG_FORMAT=${IRIS_CLANG_FORMAT}"
        "-DCLANG_TIDY=${IRIS_CLANG_TIDY}"
        "-DSDKROOT=${CMAKE_OSX_SYSROOT}"
        -P "${CMAKE_CURRENT_SOURCE_DIR}/cpp-programs.cmake")
  endforeach()
  # The round trip builds about 2,400 programs in rust/target, which one run
  # at a time may use; the views build one program in a directory of their
  # own.
  set_tests_properties(builder-round-trip PROPERTIES
    TIMEOUT 3600
    SKIP_REGULAR_EXPRESSION "Skipped: builder_round_trip_cpp needs clang-format"
    RESOURCE_LOCK iris-cpp-round-trip)
  set_tests_properties(generated-views PROPERTIES
    TIMEOUT 1800
    RESOURCE_LOCK iris-cpp-views)
endif()
