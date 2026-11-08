# Run every case of every format in formats/ through the format builder
# driver, which answers with the spec of the format's builder in
# cpp/examples in place of the case's own.
#
#   cmake -DDRIVER=... -DREPOSITORY=... -DCARGO=... [-DCASE_TIMEOUT=seconds]
#         -P format-builders.cmake
#
# Each format directory needs a builder, examples/build_NAME.hpp with
# NAME the directory's name in snake case, and every one of its cases must
# pass, judged with the case's operation and parameters: the data must be
# the expected data and a failure the expected error. Fails unless the
# runner runs every case directory it holds.
cmake_minimum_required(VERSION 3.28)

foreach(variable IN ITEMS DRIVER REPOSITORY CARGO)
  if(NOT DEFINED ${variable})
    message(FATAL_ERROR "format-builders.cmake needs -D${variable}=...")
  endif()
endforeach()
if(NOT DEFINED CASE_TIMEOUT)
  set(CASE_TIMEOUT 30)
endif()

file(REAL_PATH "${REPOSITORY}" REPOSITORY)
file(GLOB entries LIST_DIRECTORIES true "${REPOSITORY}/formats/*")
set(formats "")
foreach(entry IN LISTS entries)
  if(IS_DIRECTORY "${entry}")
    cmake_path(GET entry FILENAME format)
    list(APPEND formats "${format}")
  endif()
endforeach()
list(SORT formats)
list(LENGTH formats format_count)
if(format_count EQUAL 0)
  message(FATAL_ERROR "no formats in ${REPOSITORY}/formats")
endif()

set(problems "")
set(all_cases 0)
foreach(format IN LISTS formats)
  string(REPLACE "-" "_" name "${format}")
  if(NOT EXISTS "${REPOSITORY}/cpp/examples/build_${name}.hpp")
    list(APPEND problems "${format} has no builder cpp/examples/build_${name}.hpp")
    continue()
  endif()
  # A case directory has a spec.iris, spec.bin, or case.toml.
  file(GLOB_RECURSE markers LIST_DIRECTORIES false
    "${REPOSITORY}/formats/${format}/spec.iris"
    "${REPOSITORY}/formats/${format}/spec.bin"
    "${REPOSITORY}/formats/${format}/case.toml")
  set(cases "")
  foreach(marker IN LISTS markers)
    get_filename_component(case "${marker}" DIRECTORY)
    list(APPEND cases "${case}")
  endforeach()
  list(REMOVE_DUPLICATES cases)
  list(LENGTH cases expected)
  execute_process(
    COMMAND "${CARGO}" run --quiet --locked -p gecode-iris-conformance --
      run --driver "\"${DRIVER}\" ${format}" --version 0.1
      --timeout ${CASE_TIMEOUT} "${REPOSITORY}/formats/${format}"
    WORKING_DIRECTORY "${REPOSITORY}/rust"
    RESULT_VARIABLE exit_status
    OUTPUT_VARIABLE output
    ERROR_VARIABLE errors)
  if(NOT output MATCHES "Summary: total=([0-9]+) pass=([0-9]+) ")
    message("${output}${errors}")
    list(APPEND problems "${format}: the runner printed no summary "
      "(status ${exit_status})")
    continue()
  endif()
  set(total "${CMAKE_MATCH_1}")
  set(pass "${CMAKE_MATCH_2}")
  message("${format}: ${pass} of ${total} cases pass through "
    "build_${name}.hpp (${expected} case directories)")
  math(EXPR all_cases "${all_cases} + ${total}")
  if(NOT exit_status EQUAL 0 OR NOT pass EQUAL total)
    message("${output}${errors}")
    list(APPEND problems "${format}: ${pass} of ${total} cases pass")
  endif()
  if(NOT total EQUAL expected)
    list(APPEND problems
      "${format}: the runner ran ${total} cases of ${expected}")
  endif()
  if(total LESS 2)
    list(APPEND problems "${format}: too few cases (${total})")
  endif()
endforeach()

message("Format builders: ${format_count} formats, ${all_cases} cases")
if(problems)
  list(JOIN problems "\n  " text)
  message(FATAL_ERROR "the format builders failed:\n  ${text}")
endif()
