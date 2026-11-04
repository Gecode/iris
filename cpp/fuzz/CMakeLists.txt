# The fuzz targets iris-fuzz-parse and iris-fuzz-read. Each source defines
# LLVMFuzzerTestOneInput. With IRIS_FUZZ, libFuzzer drives it; otherwise
# replay.cpp runs it over files, so that every build compiles, warns about
# and tests the targets, and can reproduce a libFuzzer artifact.
if(IRIS_FUZZ AND NOT CMAKE_CXX_COMPILER_ID STREQUAL "Clang")
  message(FATAL_ERROR
    "IRIS_FUZZ needs LLVM Clang, which provides libFuzzer, not "
    "${CMAKE_CXX_COMPILER_ID}; Apple Clang has no libFuzzer, so on macOS use "
    "Homebrew LLVM (see README.md)")
endif()

foreach(target IN ITEMS parse read)
  set(executable iris-fuzz-${target})
  # The targets render data and errors with the driver's canonical JSON.
  add_executable(${executable} ${target}.cpp ../driver/json.cpp)
  target_include_directories(${executable} PRIVATE "${PROJECT_SOURCE_DIR}/driver")
  target_link_libraries(${executable} PRIVATE iris::iris)
  set_target_properties(${executable} PROPERTIES CXX_EXTENSIONS OFF)
  iris_set_warnings(${executable})
  if(IRIS_FUZZ)
    target_link_options(${executable} PRIVATE -fsanitize=fuzzer)
  else()
    target_sources(${executable} PRIVATE replay.cpp)
    # A libFuzzer executable given a directory fuzzes it, so only the replay
    # builds get these tests.
    if(IRIS_BUILD_TESTS AND EXISTS "${PROJECT_SOURCE_DIR}/../formats")
      add_test(NAME fuzz-replay-${target}
        COMMAND ${executable} "${PROJECT_SOURCE_DIR}/../formats")
    endif()
  endif()
endforeach()
