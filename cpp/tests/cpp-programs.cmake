# Run a test of the gecode-iris crate in rust/ that generates C++ programs
# and builds them with this build's compiler: the builder round trip
# (TEST=builder_round_trip_cpp) or the generated views (TEST=cpp_views).
#
#   cmake -DTEST=... -DREPOSITORY=... -DCARGO=... -DCXX=... -DCMAKE=...
#         [-DGENERATOR=...] [-DCXX_FLAGS=...] [-DLINKER_FLAGS=...]
#         [-DCLANG_FORMAT=...] [-DCLANG_TIDY=...] [-DSDKROOT=...]
#         [-DNEEDS_CLANG_FORMAT=ON] -P cpp-programs.cmake
#
# CXX_FLAGS and LINKER_FLAGS, when defined, replace the compile and
# executable link flags of the programs' build, those of its configuration
# included.
#
# The test checks the generated code with clang-format 22.1.8 when
# CLANG_FORMAT names it, and with clang-tidy 22.1.8 when CLANG_TIDY names
# it. With NEEDS_CLANG_FORMAT and without that clang-format, the script
# prints a line starting "Skipped:", which the test treats as skipped.
cmake_minimum_required(VERSION 3.28)

foreach(variable IN ITEMS TEST REPOSITORY CARGO CXX CMAKE)
  if(NOT DEFINED ${variable})
    message(FATAL_ERROR "cpp-programs.cmake needs -D${variable}=...")
  endif()
endforeach()

set(version 22.1.8)

# Whether `tool` prints `version` for --version
function(has_version tool result)
  set(${result} FALSE PARENT_SCOPE)
  if(tool)
    execute_process(COMMAND "${tool}" --version
      OUTPUT_VARIABLE output ERROR_QUIET RESULT_VARIABLE status)
    if(status EQUAL 0 AND output MATCHES "version ${version}([^0-9]|$)")
      set(${result} TRUE PARENT_SCOPE)
    endif()
  endif()
endfunction()

set(environment "CXX=${CXX}" "IRIS_CMAKE=${CMAKE}")
has_version("${CLANG_FORMAT}" format_ok)
if(format_ok)
  list(APPEND environment "IRIS_CLANG_FORMAT=${CLANG_FORMAT}")
elseif(NEEDS_CLANG_FORMAT)
  message("Skipped: ${TEST} needs clang-format ${version}; configure "
    "with -DIRIS_CLANG_FORMAT=PATH to name it (found '${CLANG_FORMAT}')")
  return()
else()
  message("clang-format ${version} not found, so ${TEST} skips it "
    "(found '${CLANG_FORMAT}')")
endif()
if(GENERATOR)
  list(APPEND environment "CMAKE_GENERATOR=${GENERATOR}")
endif()
if(DEFINED CXX_FLAGS)
  list(APPEND environment "IRIS_CXX_FLAGS=${CXX_FLAGS}")
endif()
if(DEFINED LINKER_FLAGS)
  list(APPEND environment "IRIS_EXE_LINKER_FLAGS=${LINKER_FLAGS}")
endif()
has_version("${CLANG_TIDY}" tidy_ok)
if(tidy_ok)
  list(APPEND environment "IRIS_CLANG_TIDY=${CLANG_TIDY}")
  # clang-tidy on macOS finds the SDK through SDKROOT, which CMake leaves
  # to the compiler unless CMAKE_OSX_SYSROOT names it.
  if(NOT SDKROOT AND NOT DEFINED ENV{SDKROOT} AND CMAKE_HOST_APPLE)
    execute_process(COMMAND xcrun --show-sdk-path
      OUTPUT_VARIABLE SDKROOT OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
  endif()
  if(SDKROOT)
    list(APPEND environment "SDKROOT=${SDKROOT}")
  endif()
else()
  message("clang-tidy ${version} not found, so ${TEST} skips it "
    "(found '${CLANG_TIDY}')")
endif()

execute_process(
  COMMAND "${CMAKE_COMMAND}" -E env ${environment}
    "${CARGO}" test --locked -p gecode-iris --test ${TEST}
      -- --ignored --nocapture
  WORKING_DIRECTORY "${REPOSITORY}/rust"
  RESULT_VARIABLE status)
if(NOT status EQUAL 0)
  message(FATAL_ERROR "${TEST} failed with status ${status}")
endif()
