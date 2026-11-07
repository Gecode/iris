# Run the C++ builder round trip of rust/ with this build's compiler.
#
#   cmake -DREPOSITORY=... -DCARGO=... -DCXX=... -DCMAKE=... [-DGENERATOR=...]
#         [-DCXX_FLAGS=...] [-DLINKER_FLAGS=...]
#         [-DCLANG_FORMAT=...] [-DCLANG_TIDY=...] [-DSDKROOT=...]
#         -P builder-round-trip.cmake
#
# CXX_FLAGS and LINKER_FLAGS, when defined, replace the compile and
# executable link flags of the programs' build, those of its configuration
# included.
#
# The round trip checks the generated programs with clang-format 22.1.8, and
# with clang-tidy 22.1.8 when CLANG_TIDY names it. Without that
# clang-format, the test prints a line starting "Skipped:", which the test
# treats as skipped.
cmake_minimum_required(VERSION 3.28)

foreach(variable IN ITEMS REPOSITORY CARGO CXX CMAKE)
  if(NOT DEFINED ${variable})
    message(FATAL_ERROR "builder-round-trip.cmake needs -D${variable}=...")
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

has_version("${CLANG_FORMAT}" format_ok)
if(NOT format_ok)
  message("Skipped: the round trip needs clang-format ${version}; configure "
    "with -DIRIS_CLANG_FORMAT=PATH to name it (found '${CLANG_FORMAT}')")
  return()
endif()
set(environment
  "CXX=${CXX}"
  "IRIS_CMAKE=${CMAKE}"
  "IRIS_CLANG_FORMAT=${CLANG_FORMAT}")
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
  message("clang-tidy ${version} not found, so the round trip skips it "
    "(found '${CLANG_TIDY}')")
endif()

execute_process(
  COMMAND "${CMAKE_COMMAND}" -E env ${environment}
    "${CARGO}" test --locked -p gecode-iris --test builder_round_trip_cpp
      -- --ignored --nocapture
  WORKING_DIRECTORY "${REPOSITORY}/rust"
  RESULT_VARIABLE status)
if(NOT status EQUAL 0)
  message(FATAL_ERROR "the C++ builder round trip failed with status ${status}")
endif()
