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

if(IRIS_CONFORMANCE_TESTS)
  find_program(IRIS_CARGO cargo)
  if(NOT IRIS_CARGO)
    message(FATAL_ERROR
      "IRIS_CONFORMANCE_TESTS needs cargo to run the conformance runner; "
      "install Rust or configure with -DIRIS_CONFORMANCE_TESTS=OFF")
  endif()
  foreach(operation IN ITEMS parse read catalog dzn)
    add_test(NAME ${operation}-conformance
      COMMAND "${CMAKE_COMMAND}"
        "-DDRIVER=$<TARGET_FILE:iris-driver>"
        "-DREPOSITORY=${PROJECT_SOURCE_DIR}/.."
        "-DCARGO=${IRIS_CARGO}"
        -DOPERATION=${operation}
        -P "${CMAKE_CURRENT_SOURCE_DIR}/conformance.cmake")
    # Whichever test runs first builds the runner with cargo.
    set_tests_properties(${operation}-conformance PROPERTIES TIMEOUT 1800)
  endforeach()
endif()
