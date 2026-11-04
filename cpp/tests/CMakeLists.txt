add_executable(iris-spec-test spec_test.cpp)
target_link_libraries(iris-spec-test PRIVATE iris::iris)
set_target_properties(iris-spec-test PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris-spec-test)
add_test(NAME spec-api COMMAND iris-spec-test)

add_executable(iris-data-test data_test.cpp)
target_link_libraries(iris-data-test PRIVATE iris::iris)
set_target_properties(iris-data-test PROPERTIES CXX_EXTENSIONS OFF)
iris_set_warnings(iris-data-test)
add_test(NAME data-api COMMAND iris-data-test)

if(IRIS_CONFORMANCE_TESTS)
  find_program(IRIS_CARGO cargo)
  if(NOT IRIS_CARGO)
    message(FATAL_ERROR
      "IRIS_CONFORMANCE_TESTS needs cargo to run the conformance runner; "
      "install Rust or configure with -DIRIS_CONFORMANCE_TESTS=OFF")
  endif()
  add_test(NAME parse-conformance
    COMMAND "${CMAKE_COMMAND}"
      "-DDRIVER=$<TARGET_FILE:iris-driver>"
      "-DREPOSITORY=${PROJECT_SOURCE_DIR}/.."
      "-DCARGO=${IRIS_CARGO}"
      -DOPERATION=parse
      -P "${CMAKE_CURRENT_SOURCE_DIR}/conformance.cmake")
  # The first run builds the runner with cargo.
  set_tests_properties(parse-conformance PROPERTIES TIMEOUT 900)
  # The cases that need work budgets must fail; see deferred-cases.txt.
  add_test(NAME read-conformance
    COMMAND "${CMAKE_COMMAND}"
      "-DDRIVER=$<TARGET_FILE:iris-driver>"
      "-DREPOSITORY=${PROJECT_SOURCE_DIR}/.."
      "-DCARGO=${IRIS_CARGO}"
      -DOPERATION=read
      "-DDEFERRED=${CMAKE_CURRENT_SOURCE_DIR}/deferred-cases.txt"
      -P "${CMAKE_CURRENT_SOURCE_DIR}/conformance.cmake")
  set_tests_properties(read-conformance PROPERTIES TIMEOUT 1800)
endif()
