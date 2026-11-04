# Run EXAMPLE from the repository root, as the guide does, and require exit
# status 0, nothing on standard error, and the output in RECORDING.
execute_process(COMMAND "${EXAMPLE}"
  WORKING_DIRECTORY "${REPOSITORY}"
  OUTPUT_VARIABLE output
  ERROR_VARIABLE errors
  RESULT_VARIABLE status)
if(NOT status EQUAL 0 OR NOT errors STREQUAL "")
  message(FATAL_ERROR "${EXAMPLE} exited with ${status}:\n${errors}")
endif()

# Windows ends the lines of standard output with CRLF.
string(REPLACE "\r\n" "\n" output "${output}")
file(READ "${RECORDING}" expected)
if(NOT output STREQUAL expected)
  message(FATAL_ERROR "${EXAMPLE} printed:\n${output}\ninstead of ${RECORDING}:\n${expected}")
endif()
