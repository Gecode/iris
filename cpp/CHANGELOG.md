# C++ changelog

## Unreleased

- Add the C++23 library `iris`, a native port of Gecode 6.5.0's reader that
  does not depend on Gecode. `iris::Spec` parses and loads reader specs and
  reads and catalogs data files and text; each operation that throws
  `iris::Error` has a `try_` variant that returns `std::expected`.
- Add `iris::Data`, whose accessors return `std::span` rows and
  `std::string_view` strings, `iris::ValueRef` for inspecting values without
  misuse, and `Data::dzn()` for MiniZinc data. Reading enforces the work
  budgets of the Iris Language Specification.
- Give `iris::Error` every part of a diagnostic, from the code and locations
  to the excerpt, context, record path and earlier rejection, as
  `iris::ErrorDetails`, and render the diagnostic of the Specification from
  `what()`.
- Allocate from a `std::pmr::memory_resource` that the caller supplies, so
  that a host such as Gecode can give the library its heap.
- Add typed handles, `iris::Field`, which `data.get` and `params.set` take,
  and print them for any reader spec with `iris gen --lang cpp`.
- Add the spec builder, `<iris/build.hpp>`, which writes reader specs from C++
  statements, checks pattern placeholders at compile time, and names the
  `std::source_location` of the statement in its errors and in the errors of
  reading a built spec.
- Install a CMake package that `find_package(iris)` finds, with the target
  `iris::iris`, and support FetchContent and `add_subdirectory`. Each release
  publishes a source archive, `iris-cpp-VERSION.tar.gz`, with its SHA-256
  checksum.
- Test with GCC 14, Clang 20, Apple Clang and MSVC, under ASan, UBSan and
  TSan, with libFuzzer, and against the whole conformance suite.
