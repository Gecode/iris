# Run the conformance cases of one operation through a driver.
#
#   cmake -DDRIVER=... -DREPOSITORY=... -DCARGO=... -DOPERATION=parse
#         -P conformance.cmake
#
# Discovers the cases in conformance/cases/0.1 and formats as the runner
# does, selects those whose case.toml names OPERATION (read without one),
# and runs them with the runner from rust/. Fails unless every selected
# case passes.
cmake_minimum_required(VERSION 3.28)

foreach(variable IN ITEMS DRIVER REPOSITORY CARGO OPERATION)
  if(NOT DEFINED ${variable})
    message(FATAL_ERROR "conformance.cmake needs -D${variable}=...")
  endif()
endforeach()

# The runner shows canonical absolute paths, so the cases are found below
# the canonical repository path too.
file(REAL_PATH "${REPOSITORY}" REPOSITORY)
set(roots "${REPOSITORY}/conformance/cases/0.1" "${REPOSITORY}/formats")
# The runner gets at most this many case directories per command line, to
# stay below the command-length limit of Windows.
set(batch_size 100)

# A case directory has a spec.iris, spec.bin, or case.toml.
set(cases "")
foreach(root IN LISTS roots)
  file(GLOB_RECURSE markers LIST_DIRECTORIES false
    "${root}/spec.iris" "${root}/spec.bin" "${root}/case.toml")
  foreach(marker IN LISTS markers)
    get_filename_component(case "${marker}" DIRECTORY)
    list(APPEND cases "${case}")
  endforeach()
endforeach()
list(REMOVE_DUPLICATES cases)
list(LENGTH cases discovered)

# Reads the operation from the top-level `operation = "..."` line. This is
# not a TOML parser, so any other way of writing an operation key (quoted,
# escaped, dotted, or inside a table) stops the test rather than silently
# selecting the case as a read. Quoted top-level keys are rejected outright,
# since an escape can spell `operation` without the word appearing.
function(case_operation case result)
  set(operation read)
  if(EXISTS "${case}/case.toml")
    file(STRINGS "${case}/case.toml" lines
      REGEX "^[ \t]*\\[|^[^=#]*operation[^=]*=|^[^=#]*[\"']")
    set(in_table FALSE)
    foreach(line IN LISTS lines)
      if(line MATCHES "^[ \t]*\\[")
        set(in_table TRUE)
      elseif(in_table AND NOT line MATCHES "^[^=#]*operation[^=]*=")
        # A quoted key inside a table cannot set the top-level operation.
      elseif(NOT in_table AND line MATCHES
             "^[ \t]*operation[ \t]*=[ \t]*\"(parse|read|catalog|dzn)\"[ \t]*(#.*)?$")
        set(operation "${CMAKE_MATCH_1}")
      else()
        message(FATAL_ERROR "${case}/case.toml: unsupported operation line: ${line}")
      endif()
    endforeach()
  endif()
  set(${result} "${operation}" PARENT_SCOPE)
endfunction()

# The selected cases by repository-relative path, which names them in
# messages
set(selected "")
foreach(case IN LISTS cases)
  if(case MATCHES ";")
    message(FATAL_ERROR "a case path contains ';': ${case}")
  endif()
  case_operation("${case}" operation)
  if(operation STREQUAL OPERATION)
    file(RELATIVE_PATH path "${REPOSITORY}" "${case}")
    list(APPEND selected "${path}")
  endif()
endforeach()
list(SORT selected)
list(LENGTH selected count)
if(count EQUAL 0)
  message(FATAL_ERROR "no ${OPERATION} cases among ${discovered} discovered")
endif()

set(statuses total pass xfail fail xpass unrepresentable)
foreach(status IN LISTS statuses)
  set(${status} 0)
endforeach()
# The status of each case run, by repository-relative path
set(result_paths "")
set(result_statuses "")
set(problems "")

# Run the cases `paths` in batches.
function(run_batches paths)
  list(LENGTH paths n)
  foreach(start RANGE 0 ${n} ${batch_size})
    if(start EQUAL n)
      break()
    endif()
    list(SUBLIST paths ${start} ${batch_size} batch)
    list(TRANSFORM batch PREPEND "${REPOSITORY}/")
    execute_process(
      COMMAND "${CARGO}" run --quiet --locked -p iris-conformance --
        run --driver "\"${DRIVER}\"" --version 0.1 ${batch}
      WORKING_DIRECTORY "${REPOSITORY}/rust"
      RESULT_VARIABLE exit_status
      OUTPUT_VARIABLE output
      ERROR_VARIABLE errors)
    message("${output}${errors}")
    # Status 1 reports failed cases, which are listed below
    if(NOT exit_status EQUAL 0 AND NOT exit_status EQUAL 1)
      message(FATAL_ERROR "the runner exited with status ${exit_status}")
    endif()
    if(exit_status EQUAL 1)
      list(APPEND problems "the runner reported failed cases")
    endif()
    if(output MATCHES "Summary: total=([0-9]+) pass=([0-9]+) xfail=([0-9]+) fail=([0-9]+) xpass=([0-9]+) unrepresentable=([0-9]+)")
      set(group 1)
      foreach(status IN LISTS statuses)
        math(EXPR ${status} "${${status}} + ${CMAKE_MATCH_${group}}")
        set(${status} "${${status}}" PARENT_SCOPE)
        math(EXPR group "${group} + 1")
      endforeach()
    else()
      message(FATAL_ERROR "the runner printed no summary")
    endif()
    # A path with ';' splits its line, so the parts fail to match below.
    string(REGEX MATCHALL "\n(PASS|FAIL|XFAIL|XPASS|UNREPRESENTABLE) [^\n]*"
      lines "\n${output}")
    foreach(line IN LISTS lines)
      if(NOT line MATCHES "^\n(PASS|FAIL|XFAIL|XPASS|UNREPRESENTABLE) (.+)$")
        message(FATAL_ERROR "unexpected status line: ${line}")
      endif()
      set(status "${CMAKE_MATCH_1}")
      string(REGEX REPLACE "^//\\?/" "" path "${CMAKE_MATCH_2}")
      string(LENGTH "${REPOSITORY}/" prefix_length)
      string(SUBSTRING "${path}" 0 ${prefix_length} prefix)
      if(NOT prefix STREQUAL "${REPOSITORY}/")
        message(FATAL_ERROR "status line for a path outside the repository: "
          "${path}")
      endif()
      string(SUBSTRING "${path}" ${prefix_length} -1 path)
      if(NOT path IN_LIST paths)
        message(FATAL_ERROR "status line for a case that was not run: ${path}")
      endif()
      if(path IN_LIST result_paths)
        message(FATAL_ERROR "two status lines for ${path}")
      endif()
      list(APPEND result_paths "${path}")
      list(APPEND result_statuses "${status}")
    endforeach()
  endforeach()
  set(result_paths "${result_paths}" PARENT_SCOPE)
  set(result_statuses "${result_statuses}" PARENT_SCOPE)
  set(problems "${problems}" PARENT_SCOPE)
endfunction()

run_batches("${selected}")

set(passed 0)
foreach(path IN LISTS selected)
  list(FIND result_paths "${path}" at)
  if(at EQUAL -1)
    list(APPEND problems "no status for ${path}")
    continue()
  endif()
  list(GET result_statuses ${at} status)
  if(status STREQUAL "PASS")
    math(EXPR passed "${passed} + 1")
  else()
    list(APPEND problems "${path}: ${status}")
  endif()
endforeach()

message("Selected ${count} ${OPERATION} cases from ${discovered} discovered in "
  "conformance/cases/0.1 and formats")
message("Passed ${passed}")
message("Summary of all batches: total=${total} pass=${pass} xfail=${xfail} "
  "fail=${fail} xpass=${xpass} unrepresentable=${unrepresentable}")
if(NOT total EQUAL count)
  list(APPEND problems "the runner ran ${total} cases, not ${count}")
endif()
if(problems)
  list(JOIN problems "\n  " text)
  message(FATAL_ERROR "${OPERATION} conformance failed:\n  ${text}")
endif()
